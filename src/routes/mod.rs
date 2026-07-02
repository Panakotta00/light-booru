use std::fs::read_dir;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};
use crate::templates::{GetIndexResponse, ImageListResponse, ImageInfo};
use askama_axum::{IntoResponse, Response};
use axum::body::Body;
use axum::extract::{Form, Path as AxumPath, Query, RawQuery, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::IntoResponse as AxumIntoResponse;
use itertools::Itertools;
use serde::Deserialize;
use tantivy::collector::TopDocs;
use tantivy::{DateTime, Order};
use tantivy::query::{AllQuery, QueryClone};
use tantivy::schema::Value;
use crate::{config, BooruState};
use crate::database::{Database};
use tantivy::query::QueryParser;
use crate::config::Config;

fn is_safe_relative_path(image_path: &str) -> bool {
    let path = Path::new(image_path);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn guess_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn has_preview_query(raw_query: &RawQuery) -> bool {
    raw_query
        .0
        .as_deref()
        .map(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .any(|(key, _)| key == "preview")
        })
        .unwrap_or(false)
}

pub async fn get_image_file(
    AxumPath(image_path): AxumPath<String>,
    raw_query: RawQuery,
    State(BooruState { config, .. }): State<BooruState>,
) -> axum::response::Result<impl AxumIntoResponse> {
    if !is_safe_relative_path(&image_path) {
        return Err(StatusCode::BAD_REQUEST.into());
    }

    let full_path: PathBuf = Path::new(&config.image_files_path).join(&image_path);
    if !full_path.exists() || !full_path.is_file() {
        return Err(StatusCode::NOT_FOUND.into());
    }

    if has_preview_query(&raw_query) {
        let image_bytes = tokio::fs::read(&full_path)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let image = image::load_from_memory(&image_bytes)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let preview = image.resize(600, 600, image::imageops::FilterType::Triangle);

        let mut encoded = Vec::new();
        let mut writer = Cursor::new(&mut encoded);
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 70);
        encoder
            .encode_image(&preview)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        return Ok((
            [(axum::http::header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"))],
            Body::from(encoded),
        ));
    }

    let bytes = tokio::fs::read(&full_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let content_type = guess_content_type(&full_path);

    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static(content_type),
        )],
        Body::from(bytes),
    ))
}

#[derive(Deserialize)]
pub struct Pagination {
    #[serde(default = "default_page")]
    page: usize,
    #[serde(default = "default_page_size")]
    page_size: usize,
}

pub fn default_page() -> usize { 0 }
pub fn default_page_size() -> usize { 10 }

pub async fn search(database: &Database, page: usize, page_size: usize, search: &str) -> Vec<ImageInfo> {
    let search = if search.is_empty() {
        "*"
    } else {
        search
    };

    let reader = database.index.reader().unwrap();
    let searcher = reader.searcher();
    let collector = TopDocs::with_limit(page_size)
        .and_offset(page * page_size)
        .order_by_fast_field("time", Order::Desc);
    let query_parser = QueryParser::for_index(&database.index, vec![]);
    let query = match query_parser.parse_query(&search) {
        Ok(query) => query,
        _ => AllQuery.box_clone(),
    };
    let top_docs = searcher.search(&query, &collector).ok().unwrap_or_default();
    top_docs
        .into_iter()
        .map(|(_, doc_address): (DateTime, _)| -> Option<ImageInfo> {
            let doc: tantivy::TantivyDocument = searcher.doc(doc_address).ok()?;
            let path = doc.get_first(database.image_schema.path)?.as_str()?.to_string();
            let tags: Vec<String> = doc.get_all(database.image_schema.tags)
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect();
            let auto_tags: Vec<String> = doc.get_all(database.image_schema.auto_tags)
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect();
            let filename = Path::new(&path).file_name().unwrap().to_str().unwrap().to_string();
            Some(ImageInfo {
                url: format!("images/{filename}"),
                tags,
                auto_tags,
                id: filename,
            })
        })
        .flatten()
        .collect()
}

pub async fn list_images(config: &Config, page: usize, page_size: usize) -> Option<Vec<String>> {
    Some(read_dir(&config.image_files_path).ok()?
        .skip(page * page_size).take(page_size)
        .flatten()
        .map(|d| d.file_name().into_string())
        .flatten()
        .map(|s| format!("images/{s}"))
        .collect_vec())
}

#[derive(Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub search: String,
}

pub async fn get_index(
    Query(pagination): Query<Pagination>,
    Query(searchQuery): Query<SearchQuery>,
    State(BooruState{database, config}): State<BooruState>,
) -> axum::response::Result<Response> {
    let images = search(&database, pagination.page, pagination.page_size, &searchQuery.search).await;

	Ok(GetIndexResponse {
		images,
        search: &searchQuery.search,
        page_size: pagination.page_size,
		next_page: pagination.page + 1,
	}.into_response())
}

pub async fn get_images(
    Query(pagination): Query<Pagination>,
    Query(searchQuery): Query<SearchQuery>,
    State(BooruState{database, config}): State<BooruState>,
) -> axum::response::Result<Response> {

    //let images = list_images(pagination.page, pagination.page_size).await
    //    .ok_or(Err(axum::response::ErrorResponse::from("Fuck!")))?;
    let images = search(&database, pagination.page, pagination.page_size, &searchQuery.search).await;

    Ok(ImageListResponse {
        images,
        search: &searchQuery.search,
        page_size: pagination.page_size,
        next_page: pagination.page + 1,
    }.into_response())
}

#[derive(Deserialize)]
pub struct ImageViewerQuery {
    pub id: String,
}

pub async fn get_image_viewer(
    Query(query): Query<ImageViewerQuery>,
    State(BooruState{database, config}): State<BooruState>,
) -> axum::response::Result<Response> {
    let reader = database.index.reader().unwrap();
    let searcher = reader.searcher();

    let query_parser = QueryParser::for_index(&database.index, vec![database.image_schema.path]);
    let tantivy_query = query_parser.parse_query(&format!("\"{}\"", query.id)).map_err(|e| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let top_docs = searcher.search(&tantivy_query, &TopDocs::with_limit(1)).ok().unwrap_or_default();

    if let Some((_, doc_address)) = top_docs.first() {
        let doc: tantivy::TantivyDocument = searcher.doc(*doc_address).map_err(|e| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        let path = doc.get_first(database.image_schema.path).and_then(|v| v.as_str()).ok_or(axum::http::StatusCode::NOT_FOUND)?.to_string();
        let tags: Vec<String> = doc.get_all(database.image_schema.tags)
            .map(|v| v.as_str().unwrap_or_default().to_string())
            .collect();
        let auto_tags: Vec<String> = doc.get_all(database.image_schema.auto_tags)
            .map(|t| t.as_str().unwrap_or_default().to_string())
            .collect();
        let filename = Path::new(&path).file_name().unwrap().to_str().unwrap().to_string();

        Ok(crate::templates::ImageViewerResponse {
            image: ImageInfo {
                url: format!("images/{filename}"),
                tags,
                auto_tags,
                id: filename,
            },
        }.into_response())
    } else {
        Err(axum::http::StatusCode::NOT_FOUND.into())
    }
}

#[derive(Deserialize)]
pub struct DeleteTagQuery {
    pub id: String,
    pub tag: String,
}

pub async fn delete_tag(
    Query(query): Query<DeleteTagQuery>,
    State(BooruState{database, config}): State<BooruState>,
) -> axum::response::Result<Response> {
    database.update_tags(&config.image_files_path, &query.id, &[query.tag], &[])
        .map_err(|e| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    get_image_viewer(Query(ImageViewerQuery { id: query.id }), State(BooruState{database,config})).await
}

#[derive(Deserialize)]
pub struct AddTagForm {
    pub tag: String,
}

pub async fn add_tag(
    Query(query): Query<ImageViewerQuery>,
    State(BooruState{database, config}): State<BooruState>,
    Form(form): Form<AddTagForm>,
) -> axum::response::Result<Response> {
    database.update_tags(&config.image_files_path, &query.id, &[], &[form.tag])
        .map_err(|e| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    get_image_viewer(Query(ImageViewerQuery { id: query.id }), State(BooruState{database,config})).await
}

#[derive(Deserialize)]
pub struct RefreshAutoTagsQuery {
    pub id: String,
}

pub async fn refresh_auto_tags(
    Query(query): Query<RefreshAutoTagsQuery>,
    State(BooruState{database, config}): State<BooruState>,
) -> axum::response::Result<Response> {
    let filename = &query.id;
    let images_path = &config.image_files_path;
    let file_path = Path::new(images_path).join(filename);

    if !file_path.exists() {
        return Err(axum::http::StatusCode::NOT_FOUND.into());
    }

    let api_key = "4d32f0a01fbfb2b4bbc3b5a93f26cddad6ae2a2a";

    let part = reqwest::multipart::Part::bytes(std::fs::read(&file_path).unwrap()).file_name(file_path.file_name().unwrap().to_str().unwrap().to_string());
    let form = reqwest::multipart::Form::new().part("file", part);

    let client = reqwest::ClientBuilder::new().build().unwrap();
    let response = client.post("https://saucenao.com/search.php")
        .query(&[("api_key", api_key)])
        .query(&[("output_type", "2")])
        //.query(&[("testmode", "1")])
        .query(&[("numres", "4")])
        .query(&[("db", "999")])
        .multipart(form)
        .send()
        .await
        .unwrap();
    let json = response.json::<serde_json::value::Value>().await.unwrap();
    println!("{:?}", serde_json::to_string_pretty(&json).unwrap());

    /*let handle = rustnao::HandlerBuilder::default()
        .api_key(api_key)
        .num_results(3)
        .build();

    handle.set_min_similarity(70);

    let result: Vec<rustnao::Sauce> = handle.get_sauce(file, None, None).unwrap();

    for sauce in result {
        println!("{:?}", sauce.ext_urls);
    }*/

    get_image_viewer(Query(ImageViewerQuery { id: query.id }), State(BooruState{database,config})).await
}