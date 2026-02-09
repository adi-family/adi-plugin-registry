use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::Router;
use http_body_util::BodyExt;
use plugin_registry_core::RegistryStorage;
use std::sync::Arc;
use tower::ServiceExt;

async fn setup() -> (RegistryStorage, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let storage = RegistryStorage::new(tmp.path().to_path_buf());
    storage.init().await.unwrap();
    storage
        .publish_plugin(
            "adi.tasks",
            "Tasks",
            "Task management",
            "core",
            "1.0.0",
            "darwin-aarch64",
            b"fake binary",
            "ADI Team",
            vec![],
        )
        .await
        .unwrap();
    (storage, tmp)
}

fn build_app(storage: RegistryStorage) -> Router {
    use axum::extract::{Path, State};
    use axum::http::header;
    use axum::routing::{get, post};

    let storage = Arc::new(storage);

    let publish_web = |State(s): State<Arc<RegistryStorage>>,
                       Path((id, version)): Path<(String, String)>,
                       body: axum::body::Bytes| async move {
        if body.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                axum::Json(serde_json::json!({"error": "Empty body"})),
            )
                .into_response();
        }
        s.publish_plugin_web_ui(&id, &version, &body)
            .await
            .unwrap();
        (
            StatusCode::CREATED,
            axum::Json(serde_json::json!({"status": "published"})),
        )
            .into_response()
    };

    let download_web = |State(s): State<Arc<RegistryStorage>>,
                        Path((id, version)): Path<(String, String)>| async move {
        let path = s.get_plugin_web_ui_path(&id, &version);
        if !path.exists() {
            return (
                StatusCode::NOT_FOUND,
                axum::Json(serde_json::json!({"error": "Not found"})),
            )
                .into_response();
        }
        let data = tokio::fs::read(&path).await.unwrap();
        axum::response::Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/javascript")
            .header(
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable",
            )
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(Body::from(data))
            .unwrap()
    };

    let get_plugin_info = |State(s): State<Arc<RegistryStorage>>,
                           Path(id): Path<String>| async move {
        match s.get_plugin_latest(&id).await {
            Ok(info) => axum::Json(serde_json::to_value(&info).unwrap()).into_response(),
            Err(_) => (
                StatusCode::NOT_FOUND,
                axum::Json(serde_json::json!({"error": "Not found"})),
            )
                .into_response(),
        }
    };

    Router::new()
        .route(
            "/v1/publish/plugins/:id/:version/web",
            post(publish_web),
        )
        .route("/v1/plugins/:id/:version/web.js", get(download_web))
        .route("/v1/plugins/:id/latest.json", get(get_plugin_info))
        .with_state(storage)
}

async fn response_bytes(response: axum::response::Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

#[tokio::test]
async fn test_publish_web_ui_endpoint() {
    let (storage, _tmp) = setup().await;
    let app = build_app(storage);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/publish/plugins/adi.tasks/1.0.0/web")
                .body(Body::from("export default class TasksPlugin {}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_download_web_ui_js() {
    let (storage, _tmp) = setup().await;
    let js_content = b"export default class TasksPlugin {}";
    storage
        .publish_plugin_web_ui("adi.tasks", "1.0.0", js_content)
        .await
        .unwrap();

    let app = build_app(storage);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/plugins/adi.tasks/1.0.0/web.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/javascript"
    );
    let body = response_bytes(response).await;
    assert_eq!(body, js_content);
}

#[tokio::test]
async fn test_download_web_ui_not_found() {
    let (storage, _tmp) = setup().await;
    let app = build_app(storage);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/plugins/adi.nonexistent/1.0.0/web.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_publish_web_ui_empty_body() {
    let (storage, _tmp) = setup().await;
    let app = build_app(storage);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/publish/plugins/adi.tasks/1.0.0/web")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_download_web_ui_cors_headers() {
    let (storage, _tmp) = setup().await;
    storage
        .publish_plugin_web_ui("adi.tasks", "1.0.0", b"js")
        .await
        .unwrap();

    let app = build_app(storage);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/plugins/adi.tasks/1.0.0/web.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .unwrap(),
        "*"
    );
}

#[tokio::test]
async fn test_download_web_ui_cache_headers() {
    let (storage, _tmp) = setup().await;
    storage
        .publish_plugin_web_ui("adi.tasks", "1.0.0", b"js")
        .await
        .unwrap();

    let app = build_app(storage);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/plugins/adi.tasks/1.0.0/web.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let cache_control = response
        .headers()
        .get("cache-control")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        cache_control.contains("immutable"),
        "Expected immutable in cache-control, got: {}",
        cache_control
    );
}

#[tokio::test]
async fn test_plugin_info_has_web_ui_url() {
    let (storage, _tmp) = setup().await;
    let js = b"export default class {}";
    storage
        .publish_plugin_web_ui("adi.tasks", "1.0.0", js)
        .await
        .unwrap();

    let app = build_app(storage);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/plugins/adi.tasks/latest.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_bytes(response).await;
    let info: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let web_ui = &info["web_ui"];
    assert!(web_ui.is_object(), "Expected web_ui object, got: {}", info);
    assert_eq!(web_ui["entry_url"], "/v1/plugins/adi.tasks/1.0.0/web.js");
    assert_eq!(web_ui["size_bytes"], js.len() as u64);
}
