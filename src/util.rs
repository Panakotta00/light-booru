use image::ImageFormat;
use std::path::Path;

pub fn sanitize_tag(tag: &str) -> String {
	tag.trim()
		.replace(" ", "_")
		.replace("\t", "_")
		.to_lowercase()
}

pub fn detect_image_format(path: &Path) -> Option<ImageFormat> {
	let bytes = std::fs::read(path).ok()?;
	image::guess_format(&bytes).ok()
}

pub fn is_supported_image_file(path: &Path) -> bool {
	matches!(
		detect_image_format(path),
		Some(
			ImageFormat::Jpeg
				| ImageFormat::Png
				| ImageFormat::Gif
				| ImageFormat::WebP
				| ImageFormat::Bmp
				| ImageFormat::Tiff
		)
	)
}

pub fn detect_content_type(path: &Path) -> &'static str {
	match detect_image_format(path) {
		Some(ImageFormat::Jpeg) => "image/jpeg",
		Some(ImageFormat::Png) => "image/png",
		Some(ImageFormat::Gif) => "image/gif",
		Some(ImageFormat::WebP) => "image/webp",
		Some(ImageFormat::Bmp) => "image/bmp",
		Some(ImageFormat::Tiff) => "image/tiff",
		Some(_) => "application/octet-stream",
		None => match path
			.extension()
			.and_then(|e| e.to_str())
			.map(|e| e.to_ascii_lowercase())
			.as_deref()
		{
			Some("svg") => "image/svg+xml",
			_ => "application/octet-stream",
		},
	}
}
