mod config;
mod database;
mod preview;
mod routes;
mod templates;
mod util;
mod watcher;

use crate::database::{build_schema, load_database, ImageSchema};
use axum::routing::{get, post};
use axum::Router;
use filetime::FileTime;
use std::fs;
use std::fs::read_dir;
use tantivy::directory::MmapDirectory;
use tantivy::{doc, DateTime, Index, IndexWriter, TantivyDocument, Term};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Clone)]
pub struct BooruState {
	database: database::Database,
	config: config::Config,
}

pub async fn app() -> Result<Router, anyhow::Error> {
	let config = config::Config::load();
	let database = load_database(if config.index_on_load {
		Some(&config.image_files_path)
	} else {
		None
	});

	println!(
		"Database loaded with {} images!",
		database.index.reader().unwrap().searcher().num_docs()
	);

	let state = BooruState { database, config };

	watcher::start_watcher(state.clone())?;
	preview::spawn_background_cache_warmup(state.config.clone());

	Ok(Router::new()
		.nest_service(
			"/script",
			tower_http::services::ServeDir::new("static/script"),
		)
		.nest_service(
			"/styles",
			tower_http::services::ServeDir::new("static/styles"),
		)
		.route("/images/{*path}", get(routes::get_image_file))
		.route("/", get(routes::get_index))
		.route("/imageList", get(routes::get_images))
		.route("/imageViewer", get(routes::get_image_viewer))
		.route("/imageViewer/addTag", post(routes::add_tag))
		.route("/imageViewer/deleteTag", post(routes::delete_tag))
		.route(
			"/imageViewer/refreshAutoTags",
			get(routes::refresh_auto_tags),
		)
		.layer(TraceLayer::new_for_http())
		.layer(CompressionLayer::new())
		.layer(CorsLayer::new().allow_origin(Any))
		.with_state(state))
}

#[tokio::main]
async fn main() {
	tracing_subscriber::registry()
		.with(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| "LightBooru=debug,tower_http=debug".into()),
		)
		.with(tracing_subscriber::fmt::layer())
		.init();

	let port = std::env::var("PORT").unwrap_or(String::from("3000"));

	let addr = format!("0.0.0.0:{}", port);
	info!("listening on {}", addr);

	let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
	axum::serve(listener, app().await.unwrap()).await.unwrap();
}
