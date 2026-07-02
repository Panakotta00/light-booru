use crate::preview;
use crate::BooruState;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

pub fn start_watcher(state: BooruState) -> anyhow::Result<()> {
	let images_path_str = state.config.image_files_path.clone();
	let images_path = Path::new(&images_path_str)
		.canonicalize()
		.unwrap_or_else(|_| Path::new(&images_path_str).to_path_buf());
	let (tx, mut rx) = mpsc::channel(100);

	let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| match res {
		Ok(event) => {
			let _ = tx.blocking_send(event);
		}
		Err(e) => error!("watch error: {:?}", e),
	})?;

	watcher.watch(&images_path, RecursiveMode::Recursive)?;

	tokio::spawn(async move {
		// Keep the watcher alive by moving it into this task
		let _watcher = watcher;

		while let Some(event) = rx.recv().await {
			debug!("FS event: {:?}", event);
			for path in event.paths {
				// Try to strip prefix from both canonicalized and original path just in case
				let relative_path = path.strip_prefix(&images_path).or_else(|_| {
					let absolute_images_path =
						std::env::current_dir().unwrap().join(&images_path_str);
					path.strip_prefix(absolute_images_path)
				});

				if let Ok(rel_path) = relative_path {
					if let Some(filename) = rel_path.to_str() {
						if filename.is_empty() {
							continue;
						}

						// Ignore directories
						if path.is_dir() {
							continue;
						}

						// We only care about image-like files.
						// rexiv2 will handle the actual check, but we can do a quick extension check to avoid noise.
						let ext = path
							.extension()
							.and_then(|e| e.to_str())
							.unwrap_or_default()
							.to_lowercase();
						if !ext.is_empty()
							&& !["jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif"]
								.contains(&ext.as_str())
						{
							continue;
						}

						info!("Updating index for: {}", filename);
						if let Err(e) = state
							.database
							.update_file_in_index(&images_path_str, filename)
						{
							error!("Failed to update index for {}: {}", filename, e);
						}

						let cfg = state.config.clone();
						let rel = filename.to_string();
						tokio::spawn(async move {
							if let Err(e) = preview::ensure_preview(&cfg, &rel).await {
								debug!("Failed to build preview for {}: {}", rel, e);
							}
						});
					}
				}
			}
		}
	});

	Ok(())
}
