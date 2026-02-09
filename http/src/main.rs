mod generated;

use anyhow::Result;
use async_trait::async_trait;
use axum::{
    body::Body,
    http::{header, StatusCode},
    routing::get,
    Json, Router,
};
use generated::models::*;
use generated::server::*;
use lib_http_common::version_header_layer;
use plugin_registry_core::RegistryStorage;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

struct AppState {
    storage: RegistryStorage,
}

fn internal_error(e: impl std::fmt::Display) -> ApiError {
    ApiError {
        status: 500,
        code: "internal_error".to_string(),
        message: e.to_string(),
    }
}

fn not_found(msg: &str) -> ApiError {
    ApiError {
        status: 404,
        code: "not_found".to_string(),
        message: msg.to_string(),
    }
}

fn bad_request(msg: &str) -> ApiError {
    ApiError {
        status: 400,
        code: "bad_request".to_string(),
        message: msg.to_string(),
    }
}

/// Serve a file as a streaming gzip response.
async fn serve_file_response(path: PathBuf) -> Result<axum::response::Response, ApiError> {
    let file = File::open(&path).await.map_err(internal_error)?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download.tar.gz");

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/gzip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(body)
        .map_err(internal_error)
}

#[async_trait]
impl IndexServiceHandler for AppState {
    async fn get_index(&self) -> Result<RegistryIndex, ApiError> {
        let index = self.storage.load_index().await.map_err(internal_error)?;
        json_convert(&index)
    }
}

#[async_trait]
impl SearchServiceHandler for AppState {
    async fn search(&self, query: SearchServiceSearchQuery) -> Result<SearchResults, ApiError> {
        let index = self.storage.load_index().await.map_err(internal_error)?;
        let query_lower = query.q.to_lowercase();
        let kind = query.kind.as_deref().unwrap_or("all");

        let packages = if kind == "all" || kind == "package" {
            json_convert(
                &index
                    .packages
                    .iter()
                    .filter(|p| {
                        p.id.to_lowercase().contains(&query_lower)
                            || p.name.to_lowercase().contains(&query_lower)
                            || p.description.to_lowercase().contains(&query_lower)
                            || p.tags
                                .iter()
                                .any(|t| t.to_lowercase().contains(&query_lower))
                    })
                    .collect::<Vec<_>>(),
            )?
        } else {
            vec![]
        };

        let plugins = if kind == "all" || kind == "plugin" {
            json_convert(
                &index
                    .plugins
                    .iter()
                    .filter(|p| {
                        p.id.to_lowercase().contains(&query_lower)
                            || p.name.to_lowercase().contains(&query_lower)
                            || p.description.to_lowercase().contains(&query_lower)
                            || p.tags
                                .iter()
                                .any(|t| t.to_lowercase().contains(&query_lower))
                    })
                    .collect::<Vec<_>>(),
            )?
        } else {
            vec![]
        };

        Ok(SearchResults { packages, plugins })
    }
}

#[async_trait]
impl PackageServiceHandler for AppState {
    async fn get_latest(&self, id: String) -> Result<PackageInfo, ApiError> {
        let info = self
            .storage
            .get_package_latest(&id)
            .await
            .map_err(|_| not_found("Package not found"))?;
        json_convert(&info)
    }

    async fn get_version(&self, id: String, version: String) -> Result<PackageInfo, ApiError> {
        let version = version.trim_end_matches(".json");
        let info = self
            .storage
            .get_package_info(&id, version)
            .await
            .map_err(|_| not_found("Package version not found"))?;
        json_convert(&info)
    }

    async fn download(
        &self,
        id: String,
        version: String,
        platform: String,
    ) -> Result<axum::response::Response, ApiError> {
        let platform = platform.trim_end_matches(".tar.gz");
        let path = self.storage.package_artifact_path(&id, &version, platform);

        if !path.exists() {
            return Err(not_found("Package artifact not found"));
        }

        // Increment download counter (fire and forget)
        let storage_root = self.storage.root().to_path_buf();
        let id_clone = id.clone();
        tokio::spawn(async move {
            let storage = RegistryStorage::new(storage_root);
            let _ = storage.increment_downloads("packages", &id_clone).await;
        });

        serve_file_response(path).await
    }
}

#[async_trait]
impl PackagePublishServiceHandler for AppState {
    async fn publish(
        &self,
        id: String,
        version: String,
        platform: String,
        query: PackagePublishServicePublishQuery,
        body: Vec<u8>,
    ) -> Result<PublishResponse, ApiError> {
        if body.is_empty() {
            return Err(bad_request("No file uploaded"));
        }

        self.storage
            .publish_package(
                &id,
                &query.name,
                query.description.as_deref().unwrap_or(""),
                &version,
                &platform,
                &body,
                query.author.as_deref().unwrap_or("unknown"),
                vec![],
            )
            .await
            .map_err(internal_error)?;

        Ok(PublishResponse {
            status: "published".to_string(),
            id,
            version,
            platform,
        })
    }
}

#[async_trait]
impl PluginServiceHandler for AppState {
    async fn get_latest(&self, id: String) -> Result<PluginInfo, ApiError> {
        let info = self
            .storage
            .get_plugin_latest(&id)
            .await
            .map_err(|_| not_found("Plugin not found"))?;
        json_convert(&info)
    }

    async fn get_version(&self, id: String, version: String) -> Result<PluginInfo, ApiError> {
        let version = version.trim_end_matches(".json");
        let info = self
            .storage
            .get_plugin_info(&id, version)
            .await
            .map_err(|_| not_found("Plugin version not found"))?;
        json_convert(&info)
    }

    async fn download(
        &self,
        id: String,
        version: String,
        platform: String,
    ) -> Result<axum::response::Response, ApiError> {
        let platform = platform.trim_end_matches(".tar.gz");
        let path = self.storage.plugin_artifact_path(&id, &version, platform);

        if !path.exists() {
            return Err(not_found("Plugin artifact not found"));
        }

        // Increment download counter
        let storage_root = self.storage.root().to_path_buf();
        let id_clone = id.clone();
        tokio::spawn(async move {
            let storage = RegistryStorage::new(storage_root);
            let _ = storage.increment_downloads("plugins", &id_clone).await;
        });

        serve_file_response(path).await
    }
}

#[async_trait]
impl PluginPublishServiceHandler for AppState {
    async fn publish(
        &self,
        id: String,
        version: String,
        platform: String,
        query: PluginPublishServicePublishQuery,
        body: Vec<u8>,
    ) -> Result<PublishResponse, ApiError> {
        if body.is_empty() {
            return Err(bad_request("No file uploaded"));
        }

        let plugin_type = query.plugin_type.as_deref().unwrap_or("extension");

        self.storage
            .publish_plugin(
                &id,
                &query.name,
                query.description.as_deref().unwrap_or(""),
                plugin_type,
                &version,
                &platform,
                &body,
                query.author.as_deref().unwrap_or("unknown"),
                vec![],
            )
            .await
            .map_err(internal_error)?;

        Ok(PublishResponse {
            status: "published".to_string(),
            id,
            version,
            platform,
        })
    }
}

#[async_trait]
impl PluginWebUiPublishServiceHandler for AppState {
    async fn publish(
        &self,
        id: String,
        version: String,
        body: Vec<u8>,
    ) -> Result<PublishResponse, ApiError> {
        if body.is_empty() {
            return Err(bad_request("Empty body — expected JavaScript content"));
        }

        self.storage
            .publish_plugin_web_ui(&id, &version, &body)
            .await
            .map_err(internal_error)?;

        Ok(PublishResponse {
            status: "published".to_string(),
            id,
            version,
            platform: "web".to_string(),
        })
    }
}

#[async_trait]
impl PluginWebUiServiceHandler for AppState {
    async fn download(
        &self,
        id: String,
        version: String,
    ) -> Result<axum::response::Response, ApiError> {
        let path = self.storage.get_plugin_web_ui_path(&id, &version);
        if !path.exists() {
            return Err(not_found("Plugin web UI not found"));
        }

        let file = File::open(&path).await.map_err(internal_error)?;
        let stream = ReaderStream::new(file);
        let body = Body::from_stream(stream);

        axum::response::Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/javascript")
            .header(
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable",
            )
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(body)
            .map_err(internal_error)
    }
}

/// Convert core types to generated models via serde Value
fn json_convert<T: serde::Serialize, U: serde::de::DeserializeOwned>(
    val: &T,
) -> Result<U, ApiError> {
    serde_json::to_value(val)
        .and_then(|v| serde_json::from_value(v))
        .map_err(internal_error)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "adi-plugin-registry",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let data_dir = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        std::env::var("REGISTRY_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/data"))
    };

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    info!("Starting Plugin Registry HTTP server");
    info!("Data directory: {}", data_dir.display());

    let storage = RegistryStorage::new(data_dir);
    storage.init().await?;

    let state = Arc::new(AppState { storage });

    let app = Router::new()
        .route("/", get(health))
        .route("/health", get(health))
        .merge(create_router::<AppState>())
        .layer(axum::extract::DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(version_header_layer(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
        ))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
