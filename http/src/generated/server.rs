//! Auto-generated server handlers from TypeSpec.
//! DO NOT EDIT.
//!
//! Implement the handler traits and use the generated router.

#![allow(unused_imports)]

use super::models::*;
use super::enums::*;
use async_trait::async_trait;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, patch, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;


#[derive(Debug, serde::Serialize)]
pub struct ApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self)).into_response()
    }
}


#[async_trait]
pub trait IndexServiceHandler: Send + Sync + 'static {
    async fn get_index(&self) -> Result<RegistryIndex, ApiError>;
}

async fn index_service_get_index<S: IndexServiceHandler>(
    State(state): State<Arc<S>>,
) -> Result<Json<RegistryIndex>, ApiError> {
    let result = state.get_index().await?;
    Ok(Json(result))
}

pub fn index_service_routes<S: IndexServiceHandler>() -> Router<Arc<S>> {
    Router::new()
        .route("/v1/index.json", get(index_service_get_index::<S>))
}

#[async_trait]
pub trait SearchServiceHandler: Send + Sync + 'static {
    async fn search(&self, query: SearchServiceSearchQuery) -> Result<SearchResults, ApiError>;
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchServiceSearchQuery {
    pub q: String,
    pub kind: Option<String>,
}

async fn search_service_search<S: SearchServiceHandler>(
    State(state): State<Arc<S>>,
    Query(query): Query<SearchServiceSearchQuery>,
) -> Result<Json<SearchResults>, ApiError> {
    let result = state.search(query).await?;
    Ok(Json(result))
}

pub fn search_service_routes<S: SearchServiceHandler>() -> Router<Arc<S>> {
    Router::new()
        .route("/v1/search", get(search_service_search::<S>))
}

#[async_trait]
pub trait PackageServiceHandler: Send + Sync + 'static {
    async fn get_latest(&self, id: String) -> Result<PackageInfo, ApiError>;
    async fn get_version(&self, id: String, version: String) -> Result<PackageInfo, ApiError>;
    async fn download(&self, id: String, version: String, platform: String) -> Result<axum::response::Response, ApiError>;
}

async fn package_service_get_latest<S: PackageServiceHandler>(
    State(state): State<Arc<S>>,
    Path(id): Path<String>,
) -> Result<Json<PackageInfo>, ApiError> {
    let result = state.get_latest(id).await?;
    Ok(Json(result))
}

async fn package_service_get_version<S: PackageServiceHandler>(
    State(state): State<Arc<S>>,
    Path((id, version)):  Path<(String, String)>,
) -> Result<Json<PackageInfo>, ApiError> {
    let result = state.get_version(id, version).await?;
    Ok(Json(result))
}

async fn package_service_download<S: PackageServiceHandler>(
    State(state): State<Arc<S>>,
    Path((id, version, platform)):  Path<(String, String, String)>,
) -> Result<axum::response::Response, ApiError> {
    let result = state.download(id, version, platform).await?;
    Ok(result)
}

pub fn package_service_routes<S: PackageServiceHandler>() -> Router<Arc<S>> {
    Router::new()
        .route("/v1/packages/:id/latest.json", get(package_service_get_latest::<S>))
        .route("/v1/packages/:id/{version}.json", get(package_service_get_version::<S>))
        .route("/v1/packages/:id/:version/{platform}.tar.gz", get(package_service_download::<S>))
}

#[async_trait]
pub trait PackagePublishServiceHandler: Send + Sync + 'static {
    async fn publish(&self, id: String, version: String, platform: String, query: PackagePublishServicePublishQuery, body: Vec<u8>) -> Result<PublishResponse, ApiError>;
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagePublishServicePublishQuery {
    pub name: String,
    pub description: Option<String>,
    pub plugin_type: Option<String>,
    pub author: Option<String>,
}

async fn package_publish_service_publish<S: PackagePublishServiceHandler>(
    State(state): State<Arc<S>>,
    Path((id, version, platform)):  Path<(String, String, String)>,
    Query(query): Query<PackagePublishServicePublishQuery>,
    body: axum::body::Bytes,
) -> Result<(StatusCode, Json<PublishResponse>), ApiError> {
    let result = state.publish(id, version, platform, query, body.to_vec()).await?;
    Ok((StatusCode::CREATED, Json(result)))
}

pub fn package_publish_service_routes<S: PackagePublishServiceHandler>() -> Router<Arc<S>> {
    Router::new()
        .route("/v1/publish/packages/:id/:version/:platform", post(package_publish_service_publish::<S>))
}

#[async_trait]
pub trait PluginServiceHandler: Send + Sync + 'static {
    async fn get_latest(&self, id: String) -> Result<PluginInfo, ApiError>;
    async fn get_version(&self, id: String, version: String) -> Result<PluginInfo, ApiError>;
    async fn download(&self, id: String, version: String, platform: String) -> Result<axum::response::Response, ApiError>;
}

async fn plugin_service_get_latest<S: PluginServiceHandler>(
    State(state): State<Arc<S>>,
    Path(id): Path<String>,
) -> Result<Json<PluginInfo>, ApiError> {
    let result = state.get_latest(id).await?;
    Ok(Json(result))
}

async fn plugin_service_get_version<S: PluginServiceHandler>(
    State(state): State<Arc<S>>,
    Path((id, version)):  Path<(String, String)>,
) -> Result<Json<PluginInfo>, ApiError> {
    let result = state.get_version(id, version).await?;
    Ok(Json(result))
}

async fn plugin_service_download<S: PluginServiceHandler>(
    State(state): State<Arc<S>>,
    Path((id, version, platform)):  Path<(String, String, String)>,
) -> Result<axum::response::Response, ApiError> {
    let result = state.download(id, version, platform).await?;
    Ok(result)
}

pub fn plugin_service_routes<S: PluginServiceHandler>() -> Router<Arc<S>> {
    Router::new()
        .route("/v1/plugins/:id/latest.json", get(plugin_service_get_latest::<S>))
        .route("/v1/plugins/:id/{version}.json", get(plugin_service_get_version::<S>))
        .route("/v1/plugins/:id/:version/{platform}.tar.gz", get(plugin_service_download::<S>))
}

#[async_trait]
pub trait PluginPublishServiceHandler: Send + Sync + 'static {
    async fn publish(&self, id: String, version: String, platform: String, query: PluginPublishServicePublishQuery, body: Vec<u8>) -> Result<PublishResponse, ApiError>;
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPublishServicePublishQuery {
    pub name: String,
    pub description: Option<String>,
    pub plugin_type: Option<String>,
    pub author: Option<String>,
}

async fn plugin_publish_service_publish<S: PluginPublishServiceHandler>(
    State(state): State<Arc<S>>,
    Path((id, version, platform)):  Path<(String, String, String)>,
    Query(query): Query<PluginPublishServicePublishQuery>,
    body: axum::body::Bytes,
) -> Result<(StatusCode, Json<PublishResponse>), ApiError> {
    let result = state.publish(id, version, platform, query, body.to_vec()).await?;
    Ok((StatusCode::CREATED, Json(result)))
}

pub fn plugin_publish_service_routes<S: PluginPublishServiceHandler>() -> Router<Arc<S>> {
    Router::new()
        .route("/v1/publish/plugins/:id/:version/:platform", post(plugin_publish_service_publish::<S>))
}

#[async_trait]
pub trait PluginWebUiPublishServiceHandler: Send + Sync + 'static {
    async fn publish(&self, id: String, version: String, body: Vec<u8>) -> Result<PublishResponse, ApiError>;
}

async fn plugin_web_ui_publish_service_publish<S: PluginWebUiPublishServiceHandler>(
    State(state): State<Arc<S>>,
    Path((id, version)): Path<(String, String)>,
    body: axum::body::Bytes,
) -> Result<(StatusCode, Json<PublishResponse>), ApiError> {
    let result = state.publish(id, version, body.to_vec()).await?;
    Ok((StatusCode::CREATED, Json(result)))
}

pub fn plugin_web_ui_publish_service_routes<S: PluginWebUiPublishServiceHandler>() -> Router<Arc<S>> {
    Router::new()
        .route("/v1/publish/plugins/:id/:version/web", post(plugin_web_ui_publish_service_publish::<S>))
}

#[async_trait]
pub trait PluginWebUiServiceHandler: Send + Sync + 'static {
    async fn download(&self, id: String, version: String) -> Result<axum::response::Response, ApiError>;
}

async fn plugin_web_ui_service_download<S: PluginWebUiServiceHandler>(
    State(state): State<Arc<S>>,
    Path((id, version)): Path<(String, String)>,
) -> Result<axum::response::Response, ApiError> {
    let result = state.download(id, version).await?;
    Ok(result)
}

pub fn plugin_web_ui_service_routes<S: PluginWebUiServiceHandler>() -> Router<Arc<S>> {
    Router::new()
        .route("/v1/plugins/:id/:version/web.js", get(plugin_web_ui_service_download::<S>))
}

pub fn create_router<S: IndexServiceHandler + SearchServiceHandler + PackageServiceHandler + PackagePublishServiceHandler + PluginServiceHandler + PluginPublishServiceHandler + PluginWebUiPublishServiceHandler + PluginWebUiServiceHandler>() -> Router<Arc<S>> {
    Router::new()
        .merge(index_service_routes())
        .merge(search_service_routes())
        .merge(package_service_routes())
        .merge(package_publish_service_routes())
        .merge(plugin_service_routes())
        .merge(plugin_web_ui_publish_service_routes())
        .merge(plugin_publish_service_routes())
        .merge(plugin_web_ui_service_routes())
}
