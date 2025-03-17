use askama::Template;

#[derive(Template)]
#[template(source = "", ext = "")]
pub struct EmptyResponse {}

#[derive(Template)]
#[template(path = "index.html")]
pub struct GetIndexResponse {
	pub images: Vec<String>,
	pub next_page: usize,
}

#[derive(Template)]
#[template(path = "imageList.html")]
pub struct ImageListResponse {
	pub images: Vec<String>,
	pub next_page: usize,
}
