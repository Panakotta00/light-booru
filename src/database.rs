use crate::config;
use crate::util::sanitize_tag;
use anyhow;
use filetime::FileTime;
use itertools::Itertools;
use std::collections::HashSet;
use std::fs;
use std::fs::{read_dir, DirEntry};
use std::path::Path;
use std::sync::Arc;
use tantivy::directory::MmapDirectory;
use tantivy::schema::*;
use tantivy::{doc, DateTime, DocAddress, Index, IndexWriter, TantivyDocument, Term};

#[derive(Clone)]
pub struct ImageSchema {
	pub id: Field,
	pub path: Field,
	pub time: Field,
	pub tags: Field,
	pub auto_tags: Field,
	pub aspect_ratio: Field,
}

#[derive(Clone)]
pub struct Database {
	pub image_schema: Arc<ImageSchema>,
	pub index: Arc<Index>,
}

pub fn load_database(images_path: Option<&str>) -> Database {
	let (schema, image_schema) = build_schema();

	let index_path = "./index";
	fs::create_dir_all(index_path).unwrap();
	let index = Index::open_or_create(MmapDirectory::open(index_path).unwrap(), schema).unwrap();

	if let Some(images_path) = images_path {
		write_index(images_path, &image_schema, &index);
	}

	Database {
		image_schema: Arc::new(image_schema),
		index: Arc::new(index),
	}
}

pub fn build_schema() -> (Schema, ImageSchema) {
	let mut builder = Schema::builder();

	let package = ImageSchema {
		id: builder.add_text_field("id", STRING | STORED | FAST),
		path: builder.add_text_field("path", STRING | STORED | FAST),
		time: builder.add_date_field("time", STORED | FAST),
		tags: builder.add_text_field("tags", TEXT | STORED),
		auto_tags: builder.add_text_field("auto_tags", TEXT | STORED),
		aspect_ratio: builder.add_f64_field("aspect_ratio", STORED),
	};

	(builder.build(), package)
}

pub fn image_file_to_index_doc(
	image_schema: &ImageSchema,
	full_path: &Path,
	database_path: &str,
) -> TantivyDocument {
	let metadata = fs::metadata(full_path).unwrap();

	let mtime = FileTime::from_last_modification_time(&metadata);

	let mut doc: TantivyDocument = doc!(
		image_schema.path => database_path,
		image_schema.time => DateTime::from_timestamp_secs(mtime.unix_seconds()),
	);

	if let Ok(meta) = rexiv2::Metadata::new_from_path(full_path) {
		let tags = meta
			.get_tag_multiple_strings("Xmp.dc.subject")
			.or_else(|_| meta.get_tag_multiple_strings("Iptc.Application2.Keywords"))
			.unwrap_or_default();
		for tag in &tags {
			doc.add_text(image_schema.tags, tag);
		}

		let width = meta.get_pixel_width();
		let height = meta.get_pixel_height();
		if width > 0 && height > 0 {
			let aspect_ratio = width as f64 / height as f64;
			doc.add_f64(image_schema.aspect_ratio, aspect_ratio);
		};
	};

	doc
}

pub fn write_index(images_path: &str, image_schema: &ImageSchema, index: &Index) {
	let reader = index.reader().unwrap();
	let mut writer: IndexWriter = index.writer(50_000_000).unwrap();

	for entry in read_dir(images_path)
		.unwrap()
		.flatten()
		.filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
	{
		let path = entry.path();
		let filename = path.strip_prefix(images_path).unwrap().to_str().unwrap();
		let doc = image_file_to_index_doc(image_schema, &path, filename);

		let term = Term::from_field_text(image_schema.path, filename);
		writer.delete_term(term);
		writer.add_document(doc).unwrap();
	}

	writer.commit().unwrap();
}

impl Database {
	pub fn update_file_in_index(
		&self,
		images_path: &str,
		filename: &str,
	) -> Result<(), anyhow::Error> {
		let mut writer: IndexWriter = self.index.writer(50_000_000)?;
		let file_path = Path::new(images_path).join(filename);

		if file_path.exists() && file_path.is_file() {
			let doc = image_file_to_index_doc(&self.image_schema, &file_path, filename);
			let term = Term::from_field_text(self.image_schema.path, filename);
			writer.delete_term(term);
			writer.add_document(doc)?;
		} else {
			let term = Term::from_field_text(self.image_schema.path, filename);
			writer.delete_term(term);
		}

		writer.commit()?;
		Ok(())
	}

	pub fn update_tags(
		&self,
		images_path: &str,
		filename: &str,
		remove_tags: &[String],
		add_tags: &[String],
	) -> Result<(), String> {
		let file_path = Path::new(images_path).join(filename);

		if !file_path.exists() {
			return Err(format!("File not found: {}", filename));
		}

		let times = fs::metadata(&file_path)
			.map(|m| {
				(
					FileTime::from_last_access_time(&m),
					FileTime::from_last_modification_time(&m),
				)
			})
			.ok();

		let remove_tags: HashSet<_> = remove_tags.iter().map(|t| sanitize_tag(t)).collect();

		match rexiv2::Metadata::new_from_path(&file_path) {
			Ok(meta) => {
				let tags: Vec<_> =
					if let Ok(tags) = meta.get_tag_multiple_strings("Xmp.dc.subject") {
						tags.into_iter()
					} else {
						vec![].into_iter()
					}
					.filter(|t| !remove_tags.contains(t.as_str()))
					.chain(add_tags.iter().cloned())
					.map(|t| sanitize_tag(&t))
					.unique()
					.collect();
				let tags: Vec<_> = tags.iter().map(|t| t.as_str()).collect();

				meta.set_tag_multiple_strings("Xmp.dc.subject", &tags)
					.map_err(|e| format!("Failed to set XMP tags: {}", e))?;

				meta.save_to_file(&file_path)
					.map_err(|e| format!("Failed to save metadata to file: {}", e))?;

				if let Some((atime, mtime)) = times {
					let _ = filetime::set_file_times(&file_path, atime, mtime);
				}

				println!("Updated tags for {} to: {:?}", filename, tags);
			}
			Err(e) => return Err(format!("Failed to open metadata: {}", e)),
		}

		self.update_file_in_index(images_path, filename)
			.map_err(|e| e.to_string())?;

		Ok(())
	}
}
