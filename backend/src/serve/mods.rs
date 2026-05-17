//! Mod/plugin handlers and Modrinth API proxy.
use crate::auth::AppState;
use crate::errors::AppError;
use crate::models::User;
use axum::{
    Extension, Json,
    body::Body,
    extract::{Path, Query, State},
    response::Response as AxResponse,
};
use mc_server_manager::mc_server_installer::modrinth;
use percent_encoding::percent_decode_str;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct SearchQ {
    pub query: String,
    #[serde(rename = "type")]
    pub project_type: Option<String>,
    pub loaders: Option<String>,
    pub versions: Option<String>,
    pub client_side: Option<String>,
    pub server_side: Option<String>,
    pub open_source: Option<bool>,
    pub index: Option<String>,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}
pub async fn modrinth_search(
    Extension(_u): Extension<User>,
    Query(q): Query<SearchQ>,
) -> Result<Json<serde_json::Value>, AppError> {
    let limit = q.limit.unwrap_or(10).min(100);
    let offset = q.offset.unwrap_or(0);
    let loaders: Option<Vec<&str>> = q
        .loaders
        .as_ref()
        .map(|s| s.split(',').map(|s| s.trim()).collect());
    let versions: Option<Vec<&str>> = q
        .versions
        .as_ref()
        .map(|s| s.split(',').map(|s| s.trim()).collect());
    let results = modrinth::search(
        &q.query,
        q.project_type.as_deref(),
        loaders.as_deref(),
        versions.as_deref(),
        q.client_side.as_deref(),
        q.server_side.as_deref(),
        q.open_source,
        q.index.as_deref(),
        offset,
        limit,
    )
    .await
    .map_err(|e| AppError::Internal(format!("Modrinth search failed: {e}")))?;
    Ok(Json(
        json!({"results": results, "total_hits": results.len()}),
    ))
}

pub async fn modrinth_project(
    Extension(_u): Extension<User>,
    Path(slug): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let project = modrinth::get_project(&slug)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch project: {e}")))?;
    Ok(Json(json!(project)))
}

#[derive(Deserialize)]
pub struct VerQ {
    pub mc_version: Option<String>,
    pub loader: Option<String>,
}
pub async fn modrinth_project_versions(
    Extension(_u): Extension<User>,
    Path(slug): Path<String>,
    Query(q): Query<VerQ>,
) -> Result<Json<serde_json::Value>, AppError> {
    let versions = modrinth::fetch_versions(&slug, q.mc_version.as_deref(), q.loader.as_deref())
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch versions: {e}")))?;
    Ok(Json(json!({"slug": slug, "versions": versions})))
}

#[derive(Deserialize)]
pub struct DlQ {
    pub mc_version: String,
    pub loader: String,
}
pub async fn modrinth_download_url(
    Extension(_u): Extension<User>,
    Path(slug): Path<String>,
    Query(q): Query<DlQ>,
) -> Result<Json<serde_json::Value>, AppError> {
    let url = modrinth::get_download_url(&slug, &q.mc_version, &q.loader)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to get download URL: {e}")))?;
    // URL-decode the filename from the download URL (Modrinth CDN may use
    // percent-encoded characters like %2B for +)
    let raw = url.rsplit('/').next().unwrap_or("unknown.jar");
    let filename = percent_decode_str(raw)
        .decode_utf8()
        .unwrap_or(std::borrow::Cow::Borrowed(raw))
        .to_string();
    Ok(Json(
        json!({"slug": slug, "download_url": url, "filename": filename, "mc_version": q.mc_version, "loader": q.loader}),
    ))
}

pub async fn list_mods(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = s
        .server_registry
        .get_server(&id)
        .map_err(|e| AppError::Internal(format!("Instance not found: {e}")))?;
    let items = server
        .list_mods()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let dir = if server.mod_type() == "mod" {
        "mods"
    } else {
        "plugins"
    };
    Ok(Json(
        json!({"id": id, "type": server.mod_type(), "directory": dir, "items": items}),
    ))
}

#[derive(Deserialize)]
pub struct InstallReq {
    pub download_url: String,
    pub filename: String,
}
pub async fn install_mod(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
    Json(body): Json<InstallReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = s
        .server_registry
        .get_server(&id)
        .map_err(|e| AppError::Internal(format!("Instance not found: {e}")))?;
    // URL-decode the filename so percent-encoded chars (e.g. %2B → +)
    // are stored as the actual character on disk.
    let decoded = percent_decode_str(&body.filename)
        .decode_utf8()
        .map(|c| c.to_string())
        .unwrap_or(body.filename.clone());
    let m = server
        .install_mod(&body.download_url, &decoded)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(
        json!({"installed": true, "id": id, "filename": m.filename, "path": server.mods_dir().join(&m.filename).to_string_lossy(), "size_bytes": m.size_bytes}),
    ))
}

pub async fn delete_mod(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path((id, filename)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = s
        .server_registry
        .get_server(&id)
        .map_err(|e| AppError::Internal(format!("Instance not found: {e}")))?;
    if server.handle().is_running() {
        return Err(AppError::Internal(
            "Cannot delete mod while server is running".into(),
        ));
    }
    server
        .delete_mod(&filename)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(
        json!({"removed": true, "id": id, "filename": filename}),
    ))
}

#[derive(Deserialize)]
pub struct ToggleReq {
    pub enabled: bool,
}
pub async fn toggle_mod(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path((id, filename)): Path<(String, String)>,
    Json(body): Json<ToggleReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = s
        .server_registry
        .get_server(&id)
        .map_err(|e| AppError::Internal(format!("Instance not found: {e}")))?;
    let info = server
        .toggle_mod(&filename, body.enabled)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(
        json!({"id": id, "filename": info.filename, "enabled": info.enabled}),
    ))
}

#[derive(Deserialize)]
pub struct ModpackReq {
    pub name: String,
    pub version: String,
    pub include: Option<Vec<String>>,
}
pub async fn generate_modpack(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
    Json(body): Json<ModpackReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = s
        .server_registry
        .get_server(&id)
        .map_err(|e| AppError::Internal(format!("Instance not found: {e}")))?;
    let m = server
        .generate_modpack(&body.name, &body.version, &body.include.unwrap_or_default())
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(
        json!({"generated": true, "id": id, "name": m.name, "version": m.version, "modpack_file": m.file_path, "size_bytes": m.size_bytes, "include_count": m.include_count}),
    ))
}

pub async fn download_modpack(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
) -> Result<AxResponse, AppError> {
    let server = s
        .server_registry
        .get_server(&id)
        .map_err(|e| AppError::Internal(format!("Instance not found: {e}")))?;
    let mp = server
        .modpack_path()
        .ok_or_else(|| AppError::Internal("No modpack generated yet".into()))?;
    if !mp.is_file() {
        return Err(AppError::Internal("Modpack file not found".into()));
    }
    let data = tokio::fs::read(&mp)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to read modpack: {e}")))?;
    let filename = mp
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "modpack.mrpack".to_string());
    AxResponse::builder()
        .header("Content-Type", "application/zip")
        .header(
            "Content-Disposition",
            format!(r##"attachment; filename="{filename}"##),
        )
        .body(Body::from(data))
        .map_err(|e| AppError::Internal(e.to_string()))
}
