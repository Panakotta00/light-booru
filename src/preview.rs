use crate::config::Config;
use crate::util;
use image::GenericImageView;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tokio::sync::Semaphore;
use tracing::{debug, warn};

fn is_supported_image(path: &Path) -> bool {
	util::is_supported_image_file(path)
}

fn preview_file_path(cache_root: &Path, relative_path: &str) -> PathBuf {
	let mut out = cache_root.join(relative_path);
	out.set_extension("jpg");
	out
}

fn encode_preview_bytes(
	image_path: &Path,
	max_size: u32,
	jpeg_quality: u8,
) -> anyhow::Result<Vec<u8>> {
	let image = image::ImageReader::open(image_path)?
		.with_guessed_format()?
		.decode()?;
	let (width, height) = image.dimensions();
	if width == 0 || height == 0 {
		anyhow::bail!("image has zero dimension")
	}

	let resized = if width > max_size || height > max_size {
		image.resize(max_size, max_size, image::imageops::FilterType::Triangle)
	} else {
		image
	};

	let mut encoded = Vec::new();
	let mut writer = Cursor::new(&mut encoded);
	let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, jpeg_quality);
	encoder.encode_image(&resized)?;
	Ok(encoded)
}

pub async fn ensure_preview(config: &Config, relative_path: &str) -> anyhow::Result<PathBuf> {
	let image_path = Path::new(&config.image_files_path).join(relative_path);
	if !image_path.exists() || !image_path.is_file() {
		anyhow::bail!("source image not found")
	}

	let cache_root = Path::new(&config.preview_cache_path);
	let preview_path = preview_file_path(cache_root, relative_path);
	if preview_path.exists() {
		return Ok(preview_path);
	}

	let Some(parent) = preview_path.parent() else {
		anyhow::bail!("failed to resolve preview parent path")
	};
	tokio::fs::create_dir_all(parent).await?;

	let image_path_for_encode = image_path.clone();
	let max_size = config.preview_max_size;
	let quality = config.preview_jpeg_quality;
	let encoded = tokio::task::spawn_blocking(move || {
		encode_preview_bytes(&image_path_for_encode, max_size, quality)
	})
	.await??;

	tokio::fs::write(&preview_path, encoded).await?;
	Ok(preview_path)
}

fn collect_image_files_recursive(base: &Path, path: &Path, out: &mut Vec<String>) {
	let Ok(entries) = std::fs::read_dir(path) else {
		return;
	};

	for entry in entries.flatten() {
		let entry_path = entry.path();
		if entry_path.is_dir() {
			collect_image_files_recursive(base, &entry_path, out);
			continue;
		}

		if !is_supported_image(&entry_path) {
			continue;
		}

		if let Ok(rel) = entry_path.strip_prefix(base) {
			if let Some(rel) = rel.to_str() {
				out.push(rel.to_string());
			}
		}
	}
}

pub fn spawn_background_cache_warmup(config: Config) {
	tokio::spawn(async move {
		let image_root = Path::new(&config.image_files_path).to_path_buf();
		let mut files = Vec::new();
		collect_image_files_recursive(&image_root, &image_root, &mut files);

		let semaphore = std::sync::Arc::new(Semaphore::new(2));
		let mut handles = Vec::new();

		for relative_path in files {
			let preview_target =
				preview_file_path(Path::new(&config.preview_cache_path), &relative_path);
			if preview_target.exists() {
				continue;
			}

			let permit = match semaphore.clone().acquire_owned().await {
				Ok(permit) => permit,
				Err(_) => break,
			};

			let cfg = config.clone();
			handles.push(tokio::spawn(async move {
				let _permit = permit;
				if let Err(err) = ensure_preview(&cfg, &relative_path).await {
					debug!("preview warmup failed for {}: {}", relative_path, err);
				}
			}));
		}

		for handle in handles {
			if let Err(err) = handle.await {
				warn!("preview warmup task join error: {}", err);
			}
		}
	});
}
