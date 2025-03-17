use std::fs::read_dir;
use std::path::Path;
use crate::templates::{GetIndexResponse, ImageListResponse};
use askama_axum::{IntoResponse, Response};
use axum::extract::{Query, State};
use itertools::Itertools;
use serde::Deserialize;
use tantivy::collector::TopDocs;
use tantivy::{DateTime, Order};
use tantivy::query::{AllQuery, EnableScoring};
use tantivy::schema::Value;
use crate::database::Database;

#[derive(Deserialize)]
pub struct Pagination {
    #[serde(default = "default_page")]
    page: usize,
    #[serde(default = "default_page_size")]
    page_size: usize,
}

pub fn default_page() -> usize { 0 }
pub fn default_page_size() -> usize { 10 }

pub async fn search(database: &Database, page: usize, page_size: usize) -> Vec<String> {
    let reader = database.index.reader().unwrap();
    let searcher = reader.searcher();
    let collector = TopDocs::with_limit(page_size).and_offset(page * page_size).order_by_fast_field("time", Order::Desc);
    let top_docs = searcher.search(&AllQuery, &collector).ok().unwrap_or_default();
    top_docs
        .into_iter()
        .map(|(_, doc_address): (DateTime, _)| -> Option<String> {
            let doc: tantivy::TantivyDocument = searcher.doc(doc_address).ok()?;
            let path = doc.get_first(database.image_schema.path)?.as_str()?.to_string();
            Some(Path::new(&path).file_name().unwrap().to_str().unwrap().to_string())
        })
        .flatten()
        .map(|s| format!("images/{s}"))
        .collect()
}

pub async fn list_images(page: usize, page_size: usize) -> Option<Vec<String>> {
    Some(read_dir(".").ok()?
        .skip(page * page_size).take(page_size)
        .flatten()
        .map(|d| d.file_name().into_string())
        .flatten()
        .map(|s| format!("images/{s}"))
        .collect_vec())
}

pub async fn get_index(
    Query(pagination): Query<Pagination>,
    State(database): State<Database>,
) -> axum::response::Result<Response> {

    //let images = list_images(pagination.page, pagination.page_size).await
    //    .ok_or(Err(axum::response::ErrorResponse::from("Fuck!")))?;
    let images = search(&database, pagination.page, pagination.page_size).await;

	Ok(GetIndexResponse {
		images,
		next_page: pagination.page + 1,
	}.into_response())
}

pub async fn get_images(
    Query(pagination): Query<Pagination>,
    State(database): State<Database>,
) -> axum::response::Result<Response> {
    
    //let images = list_images(pagination.page, pagination.page_size).await
    //    .ok_or(Err(axum::response::ErrorResponse::from("Fuck!")))?;
    let images = search(&database, pagination.page, pagination.page_size).await;

    Ok(ImageListResponse {
        images,
        next_page: pagination.page + 1,
    }.into_response())
}
