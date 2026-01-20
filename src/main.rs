mod storage;

use anyhow::Result;
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use lib_http_common::version_header_layer;
use lib_plugin_registry::SearchResults;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use storage::RegistryStorage;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

struct AppState {
    storage: RegistryStorage,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_kind")]
    kind: String,
}

fn default_kind() -> String {
    "all".to_string()
}

#[derive(Deserialize)]
struct PublishParams {
    name: String,
    description: Option<String>,
    #[serde(default)]
    plugin_type: Option<String>,
    author: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
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
        // Health
        .route("/", get(health))
        .route("/health", get(health))
        // Index
        .route("/v1/index.json", get(get_index))
        // Search
        .route("/v1/search", get(search))
        // Packages
        .route("/v1/packages/:id/latest.json", get(get_package_latest))
        .route("/v1/packages/:id/:version.json", get(get_package_version))
        .route(
            "/v1/packages/:id/:version/:platform.tar.gz",
            get(download_package),
        )
        .route(
            "/v1/publish/packages/:id/:version/:platform",
            post(publish_package),
        )
        // Plugins
        .route("/v1/plugins/:id/latest.json", get(get_plugin_latest))
        .route("/v1/plugins/:id/:version.json", get(get_plugin_version))
        .route(
            "/v1/plugins/:id/:version/:platform.tar.gz",
            get(download_plugin),
        )
        .route(
            "/v1/publish/plugins/:id/:version/:platform",
            post(publish_plugin),
        )
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024)) // 100MB limit for plugin uploads
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

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "adi-plugin-registry",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn get_index(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.storage.load_index().await {
        Ok(index) => (StatusCode::OK, Json(index)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let index = match state.storage.load_index().await {
        Ok(idx) => idx,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let query_lower = query.q.to_lowercase();
    let mut results = SearchResults::default();

    let include_packages = query.kind == "all" || query.kind == "package";
    let include_plugins = query.kind == "all" || query.kind == "plugin";

    if include_packages {
        results.packages = index
            .packages
            .into_iter()
            .filter(|p| {
                p.id.to_lowercase().contains(&query_lower)
                    || p.name.to_lowercase().contains(&query_lower)
                    || p.description.to_lowercase().contains(&query_lower)
                    || p.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect();
    }

    if include_plugins {
        results.plugins = index
            .plugins
            .into_iter()
            .filter(|p| {
                p.id.to_lowercase().contains(&query_lower)
                    || p.name.to_lowercase().contains(&query_lower)
                    || p.description.to_lowercase().contains(&query_lower)
                    || p.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect();
    }

    (StatusCode::OK, Json(results)).into_response()
}

// === Package Endpoints ===

async fn get_package_latest(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.storage.get_package_latest(&id).await {
        Ok(info) => (StatusCode::OK, Json(info)).into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Package not found" })),
        )
            .into_response(),
    }
}

async fn get_package_version(
    State(state): State<Arc<AppState>>,
    Path((id, version)): Path<(String, String)>,
) -> impl IntoResponse {
    let version = version.trim_end_matches(".json");
    match state.storage.get_package_info(&id, version).await {
        Ok(info) => (StatusCode::OK, Json(info)).into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Package version not found" })),
        )
            .into_response(),
    }
}

async fn download_package(
    State(state): State<Arc<AppState>>,
    Path((id, version, platform)): Path<(String, String, String)>,
) -> impl IntoResponse {
    let platform = platform.trim_end_matches(".tar.gz");
    let path = state.storage.package_artifact_path(&id, &version, platform);

    if !path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Package artifact not found" })),
        )
            .into_response();
    }

    // Increment download counter (fire and forget)
    let storage_root = state.storage.root().to_path_buf();
    let id_clone = id.clone();
    tokio::spawn(async move {
        let storage = RegistryStorage::new(storage_root);
        let _ = storage.increment_downloads("packages", &id_clone).await;
    });

    serve_file(path).await
}

async fn publish_package(
    State(state): State<Arc<AppState>>,
    Path((id, version, platform)): Path<(String, String, String)>,
    Query(params): Query<PublishParams>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut data: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            match field.bytes().await {
                Ok(bytes) => data = Some(bytes.to_vec()),
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "error": format!("Failed to read file: {}", e) })),
                    )
                        .into_response();
                }
            }
        }
    }

    let data = match data {
        Some(d) => d,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "No file uploaded" })),
            )
                .into_response();
        }
    };

    match state
        .storage
        .publish_package(
            &id,
            &params.name,
            params.description.as_deref().unwrap_or(""),
            &version,
            &platform,
            &data,
            params.author.as_deref().unwrap_or("unknown"),
            params.tags,
        )
        .await
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "status": "published",
                "id": id,
                "version": version,
                "platform": platform
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// === Plugin Endpoints ===

async fn get_plugin_latest(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.storage.get_plugin_latest(&id).await {
        Ok(info) => (StatusCode::OK, Json(info)).into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Plugin not found" })),
        )
            .into_response(),
    }
}

async fn get_plugin_version(
    State(state): State<Arc<AppState>>,
    Path((id, version)): Path<(String, String)>,
) -> impl IntoResponse {
    let version = version.trim_end_matches(".json");
    match state.storage.get_plugin_info(&id, version).await {
        Ok(info) => (StatusCode::OK, Json(info)).into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Plugin version not found" })),
        )
            .into_response(),
    }
}

async fn download_plugin(
    State(state): State<Arc<AppState>>,
    Path((id, version, platform)): Path<(String, String, String)>,
) -> impl IntoResponse {
    let platform = platform.trim_end_matches(".tar.gz");
    let path = state.storage.plugin_artifact_path(&id, &version, platform);

    if !path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Plugin artifact not found" })),
        )
            .into_response();
    }

    // Increment download counter
    let storage_root = state.storage.root().to_path_buf();
    let id_clone = id.clone();
    tokio::spawn(async move {
        let storage = RegistryStorage::new(storage_root);
        let _ = storage.increment_downloads("plugins", &id_clone).await;
    });

    serve_file(path).await
}

async fn publish_plugin(
    State(state): State<Arc<AppState>>,
    Path((id, version, platform)): Path<(String, String, String)>,
    Query(params): Query<PublishParams>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    use tokio::io::AsyncWriteExt;

    // Stream to temp file to handle large uploads
    let temp_path = std::env::temp_dir().join(format!("plugin-upload-{}-{}.tmp", id, platform));

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let mut file = match tokio::fs::File::create(&temp_path).await {
                Ok(f) => f,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({ "error": format!("Failed to create temp file: {}", e) })),
                    )
                        .into_response()
                }
            };

            // Stream chunks to file
            let mut stream = field;
            while let Some(chunk) = stream.chunk().await.transpose() {
                match chunk {
                    Ok(bytes) => {
                        if let Err(e) = file.write_all(&bytes).await {
                            let _ = tokio::fs::remove_file(&temp_path).await;
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({ "error": format!("Failed to write chunk: {}", e) })),
                            )
                                .into_response();
                        }
                    }
                    Err(e) => {
                        let _ = tokio::fs::remove_file(&temp_path).await;
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({ "error": format!("Failed to read file: {}", e) })),
                        )
                            .into_response();
                    }
                }
            }

            if let Err(e) = file.flush().await {
                let _ = tokio::fs::remove_file(&temp_path).await;
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": format!("Failed to flush file: {}", e) })),
                )
                    .into_response();
            }
        }
    }

    // Read the temp file
    let data = match tokio::fs::read(&temp_path).await {
        Ok(d) => d,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "No file uploaded" })),
            )
                .into_response();
        }
    };

    // Clean up temp file
    let _ = tokio::fs::remove_file(&temp_path).await;

    let plugin_type = params.plugin_type.as_deref().unwrap_or("extension");

    match state
        .storage
        .publish_plugin(
            &id,
            &params.name,
            params.description.as_deref().unwrap_or(""),
            plugin_type,
            &version,
            &platform,
            &data,
            params.author.as_deref().unwrap_or("unknown"),
            params.tags,
        )
        .await
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "status": "published",
                "id": id,
                "version": version,
                "platform": platform
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn serve_file(path: PathBuf) -> axum::response::Response {
    match File::open(&path).await {
        Ok(file) => {
            let stream = ReaderStream::new(file);
            let body = Body::from_stream(stream);

            axum::response::Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/gzip")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!(
                        "attachment; filename=\"{}\"",
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("download.tar.gz")
                    ),
                )
                .body(body)
                .unwrap()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
