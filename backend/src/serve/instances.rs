//! Server instance + world management handlers.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    response::sse::{Event, Sse},
    response::Response as AxResponse,
    Extension, Json,
};
use chrono::Utc;
use futures::stream::Stream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_stream::wrappers::BroadcastStream;

use crate::auth::AppState;
use crate::errors::AppError;
use crate::models::User;
use mc_server_manager::registry::InstanceConfig;
use mc_server_manager::world;

// ═══════════════════════════════════════════════════════════════════
// Providers
// ═══════════════════════════════════════════════════════════════════

pub async fn list_providers() -> Json<serde_json::Value> {
    Json(json!(mc_server_manager::list_providers()))
}

pub async fn fetch_versions_handler(
    Path(provider): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    mc_server_manager::fetch_versions(&provider).await
        .map(|v| Json(json!({ "provider": provider, "versions": v })))
        .map_err(|e| AppError::Internal(e.to_string()))
}

pub async fn fetch_version_info(
    Path((provider, version)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    #[derive(Serialize)]
    struct SafeVersionInfo { name: String, build: Option<String>, java_version: Option<u32> }
    mc_server_manager::fetch_latest(&provider, &version).await
        .map(|i| Json(json!(SafeVersionInfo { name: i.name, build: i.build, java_version: i.java_version })))
        .map_err(|e| AppError::Internal(e.to_string()))
}

// ═══════════════════════════════════════════════════════════════════
// Instance CRUD
// ═══════════════════════════════════════════════════════════════════

pub async fn list_instances(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
) -> Json<serde_json::Value> {
    Json(json!(state.server_registry.list()))
}

pub async fn create_instance(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
    Json(mut config): Json<InstanceConfig>,
) -> Result<Json<serde_json::Value>, AppError> {
    let settings = state.settings.read().map_err(|e| AppError::Internal(format!("Settings lock: {e}")))?;
    config.server_dir = format!("{}/{}", settings.servers_dir.trim_end_matches('/'), config.id);
    config.jar_path = format!("{}/server.jar", config.server_dir);
    if config.java_path.is_empty() { config.java_path = settings.java_path.clone(); }
    drop(settings);
    state.server_registry.create(config.clone()).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({ "created": true, "instance": config })))
}

pub async fn get_instance(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>, Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (config, handle) = state.server_registry.get_info(&id)
        .ok_or_else(|| AppError::Internal(format!("Instance '{id}' not found")))?;
    Ok(Json(json!({ "config": config, "status": handle.status().await })))
}

pub async fn remove_instance(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>, Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    if let Ok(server) = state.server_registry.get_server(&id) {
        if server.handle().is_running() {
            state.server_registry.stop(&id).await.map_err(|e| AppError::Internal(e.to_string()))?;
        }
    }
    state.server_registry.remove(&id).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({ "removed": true, "id": id, "archived": true })))
}

pub async fn update_instance_config(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
    Path(id): Path<String>, Json(mut config): Json<InstanceConfig>,
) -> Result<Json<serde_json::Value>, AppError> {
    let settings = state.settings.read().map_err(|e| AppError::Internal(format!("Settings lock: {e}")))?;
    config.server_dir = format!("{}/{}", settings.servers_dir.trim_end_matches('/'), &id);
    config.jar_path = format!("{}/server.jar", config.server_dir);
    if config.java_path.is_empty() { config.java_path = settings.java_path.clone(); }
    drop(settings);
    validate_instance_paths(&config)?;
    state.server_registry.update_config(&id, config).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({ "updated": true, "id": id })))
}

pub async fn start_instance(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>, Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    state.server_registry.start(&id).await.map_err(|e| {
        log::error!("Failed to start '{id}': {e}");
        AppError::Internal(e.to_string())
    })?;
    Ok(Json(json!({ "started": true, "id": id })))
}

pub async fn stop_instance(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>, Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    state.server_registry.stop(&id).await.map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({ "stopped": true, "id": id })))
}

pub async fn instance_command(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
    Path(id): Path<String>, Json(body): Json<CommandRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    state.server_registry.send_command(&id, &body.command).await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    if let Ok(server) = state.server_registry.get_server(&id) {
        server.record_command(&body.command);
    }
    Ok(Json(json!({ "sent": true, "id": id, "command": body.command })))
}

#[derive(Deserialize)]
pub struct CommandRequest { pub command: String }

pub async fn instance_logs(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
    Path(id): Path<String>, Query(query): Query<LogsQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state.get_server(&id)?;
    let logs = server.handle().logs_tail(query.tail.unwrap_or(100)).await;
    Ok(Json(json!({ "id": id, "logs": logs, "count": logs.len() })))
}

#[derive(Deserialize)]
pub struct LogsQuery { pub tail: Option<usize> }

pub async fn instance_players(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>, Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state.get_server(&id)?;
    let players = server.handle().online_players().await;
    Ok(Json(json!({ "id": id, "players": players, "count": players.len() })))
}

// ═══════════════════════════════════════════════════════════════════
// Server Properties
// ═══════════════════════════════════════════════════════════════════

pub async fn get_properties(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>, Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state.get_server(&id)?;
    Ok(Json(json!({ "id": id, "properties": server.handle().all_properties(),
        "file_path": server.handle().properties_path().to_string_lossy() })))
}

#[derive(Deserialize)]
pub struct UpdatePropertiesRequest {
    #[serde(flatten)] pub properties: std::collections::HashMap<String, String>,
}

pub async fn update_properties(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
    Path(id): Path<String>, Json(body): Json<UpdatePropertiesRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state.get_server(&id)?;
    let requires_restart = server.handle().update_properties(body.properties)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({ "updated": true, "id": id, "properties": server.handle().all_properties(),
        "requires_restart": requires_restart })))
}

// ═══════════════════════════════════════════════════════════════════
// Schema, Archive, Logs, History
// ═══════════════════════════════════════════════════════════════════

pub async fn instance_config_schema() -> Json<serde_json::Value> {
    Json(mc_server_manager::instance_config_schema())
}

pub async fn list_archived_instances(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(json!(state.server_registry.list_archived())))
}

pub async fn restore_archived_instance(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>, Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    state.server_registry.restore_archived(&id).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({"restored": true, "id": id})))
}

pub async fn log_stream(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>, Path(id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, AppError> {
    let server = state.get_server(&id)?;
    let rx = server.handle().subscribe_logs();
    let stream = BroadcastStream::new(rx).filter_map(|r| async {
        r.ok().map(|line| Ok(Event::default().data(json!({"line": line,
            "timestamp": Utc::now().to_rfc3339()}).to_string())))
    });
    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)).text("keep-alive"),
    ))
}

pub async fn command_history(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>, Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state.get_server(&id)?;
    Ok(Json(json!({ "id": id, "history": server.command_history() })))
}

// ═══════════════════════════════════════════════════════════════════
// World Management
// ═══════════════════════════════════════════════════════════════════

pub async fn list_worlds(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>, Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state.get_server(&id)?;
    Ok(Json(json!({ "id": id, "worlds": server.list_worlds().map_err(|e| AppError::Internal(e.to_string()))?,
        "server_dir": server.server_dir().to_string_lossy(), "running": server.handle().is_running() })))
}

#[derive(Deserialize)]
pub struct BackupRequest { pub world_name: Option<String>, pub include_all: Option<bool> }

pub async fn backup_worlds(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
    Path(id): Path<String>, Json(body): Json<BackupRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state.get_server(&id)?;
    let include_all = body.include_all.unwrap_or(true);
    let worlds_to_backup: Vec<String> = if include_all {
        let worlds = server.list_worlds().map_err(|e| AppError::Internal(e.to_string()))?;
        if worlds.is_empty() { return Err(AppError::Internal("No worlds found".into())); }
        worlds.into_iter().map(|w| w.name).collect()
    } else if let Some(ref name) = body.world_name {
        vec![name.clone()]
    } else {
        return Err(AppError::Internal("Specify world_name or include_all=true".into()));
    };
    let backup_path = server.backups_dir().join(format!("{}-worlds-{}.zip", id, Utc::now().format("%Y%m%dT%H%M%SZ")));
    server.backup_worlds(&worlds_to_backup, &backup_path).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({ "id": id, "backup_file": backup_path.to_string_lossy(),
        "size_bytes": std::fs::metadata(&backup_path).map(|m| m.len()).unwrap_or(0),
        "worlds_included": worlds_to_backup, "created_at": Utc::now().to_rfc3339() })))
}

pub async fn list_backups(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>, Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state.get_server(&id)?;
    Ok(Json(json!({ "id": id, "backups": server.list_backups().map_err(|e| AppError::Internal(e.to_string()))? })))
}

pub async fn download_world(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
    Path((id, world_name)): Path<(String, String)>,
) -> Result<AxResponse, AppError> {
    let server = state.get_server(&id)?;
    let world_path = server.server_dir().join(&world_name);
    if !world_path.is_dir() || !world::is_minecraft_world(&world_path) {
        return Err(AppError::Internal(format!("World '{world_name}' not found or invalid")));
    }
    let zip_data = world::create_worlds_zip(&[(&world_name, &world_path)])
        .map_err(|e| AppError::Internal(e.to_string()))?;
    AxResponse::builder().header("Content-Type", "application/zip")
        .header("Content-Disposition", format!("attachment; filename=\"{world_name}.zip\""))
        .body(Body::from(zip_data)).map_err(|e| AppError::Internal(e.to_string()))
}

pub async fn delete_world(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
    Path((id, world_name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state.get_server(&id)?;
    if server.handle().is_running() {
        return Err(AppError::Internal("Cannot delete world while server is running".into()));
    }
    server.delete_world_dir(&world_name).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({"deleted": true, "world_name": world_name, "id": id})))
}

pub async fn upload_world(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
    Path(id): Path<String>, mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state.get_server(&id)?;
    let mut world_name = None;
    while let Some(field) = multipart.next_field().await
        .map_err(|e| AppError::Internal(format!("Multipart error: {e}")))? {
        let data = field.bytes().await.map_err(|e| AppError::Internal(format!("Read error: {e}")))?;
        world_name = Some(server.extract_world_zip(&data).map_err(|e| AppError::Internal(e.to_string()))?);
    }
    Ok(Json(json!({"uploaded": true, "world_name": world_name.unwrap_or_else(|| "unknown".into()),
        "id": id })))
}

#[derive(Deserialize)]
pub struct ResetWorldRequest { pub world_name: String }

pub async fn reset_world(
    State(state): State<Arc<AppState>>, Extension(_user): Extension<User>,
    Path(id): Path<String>, Json(body): Json<ResetWorldRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = state.get_server(&id)?;
    let was_running = server.handle().is_running();
    if was_running { state.server_registry.stop(&id).await.map_err(|e| AppError::Internal(e.to_string()))?; }
    server.delete_world_dir(&body.world_name).map_err(|e| AppError::Internal(e.to_string()))?;
    if was_running { state.server_registry.start(&id).await.map_err(|e| AppError::Internal(e.to_string()))?; }
    Ok(Json(json!({"reset": true, "world_name": body.world_name, "id": id})))
}

// ═══════════════════════════════════════════════════════════════════
// Path validation helper
// ═══════════════════════════════════════════════════════════════════

pub fn validate_instance_paths(config: &InstanceConfig) -> Result<(), AppError> {
    let forbidden = ["..", "~"];
    let schemes = ["http://", "https://", "ftp://", "file://"];
    for (name, val) in [("jar_path", &config.jar_path), ("java_path", &config.java_path)] {
        let v = val.trim();
        if v.is_empty() { return Err(AppError::InvalidPath(format!("{name} must not be empty"))); }
        for c in &forbidden { if v.contains(c) { return Err(AppError::InvalidPath(format!("{name} contains '{c}'"))); } }
        for s in &schemes { if v.to_lowercase().starts_with(s) { return Err(AppError::InvalidPath(format!("{name} must be a local path"))); } }
    }
    if !config.jar_path.trim().to_lowercase().ends_with(".jar") {
        return Err(AppError::InvalidPath("jar_path must end with .jar".into()));
    }
    let jv_fn = std::path::Path::new(config.java_path.trim()).file_name().map(|f| f.to_string_lossy().to_lowercase()).unwrap_or_default();
    if !jv_fn.contains("java") {
        return Err(AppError::InvalidPath("java_path must point to a Java executable".into()));
    }
    Ok(())
}
