use askama::Template;

pub struct ImageInfo {
    pub url: String,
    pub tags: Vec<String>,
    pub auto_tags: Vec<String>,
    pub id: String,
}

#[derive(Template)]
#[template(source = "", ext = "")]
pub struct EmptyResponse {}

#[derive(Template)]
#[template(path = "index.html")]
pub struct GetIndexResponse<'a> {
	pub images: Vec<ImageInfo>,
    pub search: &'a str,
    pub page_size: usize,
	pub next_page: usize,
}

#[derive(Template)]
#[template(path = "imageList.html")]
pub struct ImageListResponse<'a> {
	pub images: Vec<ImageInfo>,
    pub search: &'a str,
    pub page_size: usize,
	pub next_page: usize,
}

#[derive(Template)]
#[template(path = "imageViewer.html")]
pub struct ImageViewerResponse {
	pub image: ImageInfo,
}