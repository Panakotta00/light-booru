#[derive(Clone)]
pub struct Config {
    pub image_files_path: String,
    pub index_on_load: bool,
}

impl Config {
    pub fn load() -> Self {
        Self{
            image_files_path: std::env::var("IMAGE_FILES_PATH").unwrap_or("./images".to_string()),
            index_on_load: std::env::var("INDEX_ON_LOAD").unwrap_or("true".to_string()).parse().unwrap_or(true),
        }
    }
}
