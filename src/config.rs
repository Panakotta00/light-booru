#[derive(Clone)]
pub struct Config {
	pub image_files_path: String,
	pub preview_cache_path: String,
	pub preview_max_size: u32,
	pub preview_jpeg_quality: u8,
	pub index_on_load: bool,
}

impl Config {
	pub fn load() -> Self {
		Self {
			image_files_path: std::env::var("IMAGE_FILES_PATH").unwrap_or("./images".to_string()),
			preview_cache_path: std::env::var("PREVIEW_CACHE_PATH")
				.unwrap_or("./preview-cache".to_string()),
			preview_max_size: std::env::var("PREVIEW_MAX_SIZE")
				.ok().map(|v| v.parse().ok()).flatten()
				.unwrap_or(320)
				.clamp(120, 320),
			preview_jpeg_quality: std::env::var("PREVIEW_JPEG_QUALITY")
				.ok().map(|v| v.parse().ok()).flatten()
				.unwrap_or(50)
				.clamp(30, 90),
			index_on_load: std::env::var("INDEX_ON_LOAD")
				.ok().map(|v| v.parse().ok()).flatten()
				.unwrap_or(true),
		}
	}
}
