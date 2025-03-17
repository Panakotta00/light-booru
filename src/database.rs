use std::fs;
use std::fs::read_dir;
use std::path::Path;
use std::sync::Arc;
use filetime::FileTime;
use tantivy::{doc, DateTime, Index, IndexWriter};
use tantivy::directory::MmapDirectory;
use tantivy::schema::*;

#[derive(Clone)]
pub struct ImageSchema {
    pub id: Field,
    pub path: Field,
    pub time: Field,
    pub tags: Field,
    pub auto_tags: Field,
}

#[derive(Clone)]
pub struct Database {
    pub image_schema: Arc<ImageSchema>,
    pub index: Arc<Index>,
}

pub fn load_database() -> Database {
    let (schema, image_schema) = build_schema();

    let index_path = "./index";
    fs::create_dir_all(index_path).unwrap();
    let index = Index::open_or_create(MmapDirectory::open(index_path).unwrap(), schema).unwrap();

    write_index(&image_schema, &index);
    
    Database {
        image_schema: Arc::new(image_schema),
        index: Arc::new(index),
    }
}

pub fn build_schema() -> (Schema, ImageSchema) {
    let mut builder = Schema::builder();

    let package = ImageSchema {
        id: builder.add_text_field("id", STRING | STORED | FAST),
        path: builder.add_text_field("path", TEXT | STORED),
        time: builder.add_date_field("time", STORED | FAST),
        tags: builder.add_text_field("tags", TEXT | STORED),
        auto_tags: builder.add_text_field("auto_tags", TEXT | STORED),
    };

    (builder.build(), package)
}

pub fn write_index(image_schema: &ImageSchema, index: &Index) {
    let reader = index.reader().unwrap();
    let mut writer: IndexWriter = index.writer(50_000_000).unwrap();
    
    let images_path = ".";

    for entry in read_dir(images_path).unwrap().flatten().filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false)) {
        let metadata = fs::metadata(entry.path()).unwrap();

        let mtime = FileTime::from_last_modification_time(&metadata);

        let doc: TantivyDocument = doc!(
			image_schema.path => entry.path().strip_prefix(images_path).unwrap().to_str().unwrap(),
			image_schema.time => DateTime::from_timestamp_secs(mtime.unix_seconds()),
		);

        /*let file = std::fs::File::open(entry.path()).unwrap();
        let mut bufreader = std::io::BufReader::new(&file);
        let exifreader = exif::Reader::new();
        let exif = exifreader.read_from_container(&mut bufreader).unwrap();
        exif.get_field(exif::Tag::)
        for f in exif.fields() {
            println!("{} {} {}",
                     f.tag, f.ifd_num, f.display_value().with_unit(&exif));
        }*/

        writer.add_document(doc).unwrap();
    };
    
    writer.commit().unwrap();
}
