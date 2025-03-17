#![feature(async_closure)]

mod routes;
mod templates;
mod database;

use std::fs;
use std::fs::read_dir;
use axum::routing::get;
use axum::Router;
use filetime::FileTime;
use tantivy::{doc, DateTime, Index, IndexWriter, TantivyDocument};
use tantivy::directory::MmapDirectory;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use crate::database::{build_schema, load_database, ImageSchema};

pub async fn app() -> Result<Router, anyhow::Error> {
	let database = load_database();
	
	Ok(Router::new()
		.nest_service(
			"/script",
			tower_http::services::ServeDir::new("static/script"),
		)
		.nest_service(
			"/styles",
			tower_http::services::ServeDir::new("static/styles"),
		)
		.nest_service(
			"/images",
			tower_http::services::ServeDir::new("."))
		.route("/", get(routes::get_index))
		.route("/imageList", get(routes::get_images))
		.layer(TraceLayer::new_for_http())
		.layer(CompressionLayer::new())
		.layer(CorsLayer::new().allow_origin(Any))
		.with_state(database))
}

#[tokio::main]
async fn main() {
	tracing_subscriber::registry()
		.with(
			tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
				"LightBooru=debug,tower_http=debug".into()
			}),
		)
		.with(tracing_subscriber::fmt::layer())
		.init();

	let port = std::env::var("PORT").unwrap_or(String::from("3000"));

	let addr = format!("0.0.0.0:{}", port);
	info!("listening on {}", addr);

	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	axum::serve(listener, app().await.unwrap()).await.unwrap();
}
