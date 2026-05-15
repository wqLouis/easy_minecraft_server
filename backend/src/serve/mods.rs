//! Mod/plugin management handlers, Modrinth API proxy, and modpack generation.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    response::Response as AxResponse,
    Extension, Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::auth::AppState;
use crate::errors::AppError;
use crate::models::User;
use mc_server_manager::mc_server_installer::modrinth;

// ═══════════════════════════════════════════════════════════════════
// Modrinth API (external API proxy — data layer is in mc-server-installer)
// ═══════════════════════════════════════════════════════════════════

#[derive(serde::Deserialize)]
pub struct ModrinthSearchQuery {
    pub query: String,
    #[serde(rename = "type")]
    pub project_type: Option<String>,
    pub loaders: Option<String>,
    pub limit: Option<u32>,
}

/// GET /api/modrinth/search — search Modrinth for mods/plugins.
pub async fn modrinth_search(
    Extension(_user): Extension<User>,
    Query(query): Query<ModrinthSearchQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let limit = query.limit.unwrap_or(10).min(50);

    // Parse loaders: "paper,fabric" → ["paper", "fabric"]
    let loaders: Option<Vec<&str>> = query.loaders.as_ref().map(|s| {
        s.split(',').map(|s| s.trim()).collect()
    });
    let loaders_ref: Option<&[&str]> = loaders.as_deref();

    let results = modrinth::search(
        &query.query,
        query.project_type.as_deref(),
        loaders_ref,
        limit,
    )
    .await
    .map_err(|e| AppError::Internal(format!("Modrinth search failed: {e}")))?;

    let total_hits = results.len();
    Ok(Json(json!({
        "results": results,
        "total_hits": total_hits,
    })))
}

#[derive(serde::Deserialize)]
pub struct ProjectVersionsQuery {
    pub mc_version: Option<String>,
    pub loader: Option<String>,
}

/// GET /api/modrinth/project/{slug}/versions — fetch versions for a project.
pub async fn modrinth_project_versions(
    Extension(_user): Extension<User>,
    Path(slug): Path<String>,
    Query(query): Query<ProjectVersionsQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let versions = modrinth::fetch_versions(
        &slug,
        query.mc_version.as_deref(),
        query.loader.as_deref(),
    )
    .await
    .map_err(|e| AppError::Internal(format!("Failed to fetch versions: {e}")))?;

    Ok(Json(json!({
        "slug": slug,
        "versions": versions,
    })))
}

#[derive(serde::Deserialize)]
pub struct DownloadUrlQuery {
    pub mc_version: String,
    pub loader: String,
}

/// GET /api/modrinth/project/{slug}/download-url — get primary download URL.
pub async fn modrinth_download_url(
    Extension(_user): Extension<User>,
    Path(slug): Path<String>,
    Query(query): Query<DownloadUrlQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let download_url = modrinth::get_download_url(
        &slug,
        &query.mc_version,
        &query.loader,
    )
    .await
    .map_err(|e| AppError::Internal(format!("Failed to get download URL: {e}")))?;

    // Extract filename from URL
    let filename = download_url
        .rsplit('/')
        .next()
        .unwrap_or("unknown.jar")
        .to_string();

    Ok(Json(json!({
        "slug": slug,
        "download_url": download_url,
        "filename": filename,
        "mc_version": query.mc_version,
        "loader": query.loader,
    })))
}

// ═══════════════════════════════════════════════════════════════════
// Instance mod/plugin management
// ═══════════════════════════════════════════════════════════════════



/// GET /api/instances/{id}/mods — list installed mods/plugins.
pub async fn list_mods(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state
        .server_registry
        .get_server(&id)
        .map_err(|e| AppError::Internal(format!("Instance not found: {e}")))?;

    let items = server
        .list_mods()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let mod_type = server.mod_type();
    let dir_name = if mod_type == "mod" { "mods" } else { "plugins" };

    Ok(Json(json!({
        "id": id,
        "type": mod_type,
        "directory": dir_name,
        "items": items,
    })))
}

#[derive(Deserialize)]
pub struct InstallModRequest {
    pub download_url: String,
    pub filename: String,
}

/// POST /api/instances/{id}/mods/install — download and install a mod/plugin.
pub async fn install_mod(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path(id): Path<String>,
    Json(body): Json<InstallModRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state
        .server_registry
        .get_server(&id)
        .map_err(|e| AppError::Internal(format!("Instance not found: {e}")))?;

    let mod_info = server
        .install_mod(&body.download_url, &body.filename)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mods_dir = server.mods_dir();
    let file_path = mods_dir.join(&mod_info.filename);

    Ok(Json(json!({
        "installed": true,
        "id": id,
        "filename": mod_info.filename,
        "path": file_path.to_string_lossy(),
        "size_bytes": mod_info.size_bytes,
    })))
}

/// DELETE /api/instances/{id}/mods/{filename} — remove a mod/plugin.
pub async fn delete_mod(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path((id, filename)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state
        .server_registry
        .get_server(&id)
        .map_err(|e| AppError::Internal(format!("Instance not found: {e}")))?;

    if server.handle().is_running() {
        return Err(AppError::Internal(
            "Cannot delete mod/plugin while server is running. Stop the server first.".into(),
        ));
    }

    server
        .delete_mod(&filename)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "removed": true,
        "id": id,
        "filename": filename,
    })))
}

#[derive(Deserialize)]
pub struct ToggleModRequest {
    pub enabled: bool,
}

/// PUT /api/instances/{id}/mods/{filename}/toggle — enable or disable a mod/plugin.
pub async fn toggle_mod(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path((id, filename)): Path<(String, String)>,
    Json(body): Json<ToggleModRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state
        .server_registry
        .get_server(&id)
        .map_err(|e| AppError::Internal(format!("Instance not found: {e}")))?;

    let info = server
        .toggle_mod(&filename, body.enabled)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "id": id,
        "filename": info.filename,
        "enabled": info.enabled,
    })))
}

#[derive(Deserialize)]
pub struct GenerateModpackRequest {
    pub name: String,
    pub version: String,
    pub include: Option<Vec<String>>,
}

/// POST /api/instances/{id}/mods/modpack — generate a Modrinth modpack.
pub async fn generate_modpack(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path(id): Path<String>,
    Json(body): Json<GenerateModpackRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state
        .server_registry
        .get_server(&id)
        .map_err(|e| AppError::Internal(format!("Instance not found: {e}")))?;

    let include = body.include.unwrap_or_default();
    let modpack = server
        .generate_modpack(&body.name, &body.version, &include)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "generated": true,
        "id": id,
        "name": modpack.name,
        "version": modpack.version,
        "modpack_file": modpack.file_path,
        "size_bytes": modpack.size_bytes,
        "include_count": modpack.include_count,
    })))
}

/// GET /api/instances/{id}/mods/modpack/download — download the generated modpack.
pub async fn download_modpack(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<User>,
    Path(id): Path<String>,
) -> Result<AxResponse, AppError> {
    let server = state
        .server_registry
        .get_server(&id)
        .map_err(|e| AppError::Internal(format!("Instance not found: {e}")))?;

    let modpack_path = server
        .modpack_path()
        .ok_or_else(|| AppError::Internal("No modpack has been generated yet".into()))?;

    if !modpack_path.is_file() {
        return Err(AppError::Internal("Modpack file not found on disk".into()));
    }

    let data = tokio::fs::read(&modpack_path)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to read modpack: {e}")))?;

    let filename = modpack_path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "modpack.mrpack".to_string());

    let body = Body::from(data);
    let response = AxResponse::builder()
        .header("Content-Type", "application/zip")
        .header(
            "Content-Disposition",
            format!(r##"attachment; filename="{filename}""##),
        )
        .body(body)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(response)
}
