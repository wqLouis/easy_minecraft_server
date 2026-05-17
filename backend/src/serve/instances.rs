//! Instance CRUD handlers.
use crate::auth::AppState;
use crate::errors::AppError;
use crate::models::User;
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use chrono::Utc;
use futures::{StreamExt, stream::Stream};
use mc_server_manager::registry::InstanceConfig;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;

pub async fn list_providers() -> Json<serde_json::Value> {
    Json(json!(mc_server_manager::list_providers()))
}

pub async fn fetch_versions(Path(p): Path<String>) -> Result<Json<serde_json::Value>, AppError> {
    mc_server_manager::fetch_versions(&p)
        .await
        .map(|v| Json(json!({"provider": p, "versions": v})))
        .map_err(|e| AppError::Internal(e.to_string()))
}

pub async fn fetch_version_info(
    Path((p, v)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    #[derive(serde::Serialize)]
    struct S {
        name: String,
        build: Option<String>,
        java_version: Option<u32>,
    }
    mc_server_manager::fetch_latest(&p, &v)
        .await
        .map(|i| {
            Json(json!(S {
                name: i.name,
                build: i.build,
                java_version: i.java_version
            }))
        })
        .map_err(|e| AppError::Internal(e.to_string()))
}

pub async fn list_instances(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
) -> Json<serde_json::Value> {
    Json(json!(s.server_registry.list()))
}

pub async fn create_instance(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Json(mut c): Json<InstanceConfig>,
) -> Result<Json<serde_json::Value>, AppError> {
    validate_id(&c.id)?;
    let settings = s
        .settings
        .read()
        .map_err(|e| AppError::Internal(format!("Settings lock: {e}")))?;
    c.server_dir = format!("{}/{}", settings.servers_dir.trim_end_matches('/'), c.id);
    c.jar_path = format!("{}/server.jar", c.server_dir);
    if c.java_path.is_empty() {
        c.java_path = settings.java_path.clone();
    }
    drop(settings);
    s.server_registry
        .create(c.clone())
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({"created": true, "instance": c})))
}

pub async fn get_instance(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (config, handle) = s
        .server_registry
        .get_info(&id)
        .ok_or_else(|| AppError::Internal(format!("Instance '{id}' not found")))?;
    Ok(Json(
        json!({"config": config, "status": handle.status().await}),
    ))
}

pub async fn remove_instance(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    if let Ok(server) = s.server_registry.get_server(&id) {
        if server.handle().is_running() {
            s.server_registry
                .stop(&id)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
        }
    }
    s.server_registry
        .remove(&id)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({"removed": true, "id": id, "archived": true})))
}

pub async fn update_instance_config(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
    Json(mut c): Json<InstanceConfig>,
) -> Result<Json<serde_json::Value>, AppError> {
    validate_id(&id)?;
    let settings = s
        .settings
        .read()
        .map_err(|e| AppError::Internal(format!("Settings lock: {e}")))?;
    c.server_dir = format!("{}/{}", settings.servers_dir.trim_end_matches('/'), &id);
    c.jar_path = format!("{}/server.jar", c.server_dir);
    if c.java_path.is_empty() {
        c.java_path = settings.java_path.clone();
    }
    drop(settings);
    validate_instance_paths(&c)?;
    s.server_registry
        .update_config(&id, c)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({"updated": true, "id": id})))
}

pub async fn start_instance(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    s.server_registry.start(&id).await.map_err(|e| {
        log::error!("Failed to start '{id}': {e}");
        AppError::Internal(e.to_string())
    })?;
    Ok(Json(json!({"started": true, "id": id})))
}

pub async fn stop_instance(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    s.server_registry
        .stop(&id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({"stopped": true, "id": id})))
}

#[derive(Deserialize)]
pub struct CmdReq {
    pub command: String,
}
pub async fn instance_command(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
    Json(body): Json<CmdReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    s.server_registry
        .send_command(&id, &body.command)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    if let Ok(server) = s.server_registry.get_server(&id) {
        server.record_command(&body.command);
    }
    Ok(Json(
        json!({"sent": true, "id": id, "command": body.command}),
    ))
}

#[derive(Deserialize)]
pub struct LogsQ {
    pub tail: Option<usize>,
}
pub async fn instance_logs(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
    Query(q): Query<LogsQ>,
) -> Result<Json<serde_json::Value>, AppError> {
    let logs = s
        .get_server(&id)?
        .handle()
        .logs_tail(q.tail.unwrap_or(100))
        .await;
    Ok(Json(json!({"id": id, "logs": logs, "count": logs.len()})))
}

pub async fn instance_players(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let players = s.get_server(&id)?.handle().online_players().await;
    Ok(Json(
        json!({"id": id, "players": players, "count": players.len()}),
    ))
}

pub async fn get_properties(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = s.get_server(&id)?;
    let h = server.handle();
    Ok(Json(
        json!({"id": id, "properties": h.all_properties(), "file_path": h.properties_path().to_string_lossy()}),
    ))
}

#[derive(Deserialize)]
pub struct PropsReq {
    #[serde(flatten)]
    pub properties: std::collections::HashMap<String, String>,
}
pub async fn update_properties(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
    Json(b): Json<PropsReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = s.get_server(&id)?;
    let h = server.handle();
    let restart = h
        .update_properties(b.properties)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(
        json!({"updated": true, "id": id, "properties": h.all_properties(), "requires_restart": restart}),
    ))
}

pub async fn instance_config_schema() -> Json<serde_json::Value> {
    Json(mc_server_manager::instance_config_schema())
}

pub async fn list_archived(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(json!(s.server_registry.list_archived())))
}

pub async fn restore_archived(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    s.server_registry
        .restore_archived(&id)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({"restored": true, "id": id})))
}

fn sanitize_sse(s: &str) -> String {
    s.replace("\n", "\\n")
        .replace("\r", "\\r")
        .chars()
        .filter(|&c| c.is_ascii_graphic() || c == ' ' || c == '\t' || c == '\n' || c == '\r')
        .collect()
}

pub async fn log_stream(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, AppError> {
    let rx = s.get_server(&id)?.handle().subscribe_logs();
    let stream = BroadcastStream::new(rx).filter_map(|r| async {
        r.ok().map(|line| {
            Ok(Event::default().data(
                json!({"line": sanitize_sse(&line), "timestamp": Utc::now().to_rfc3339()})
                    .to_string(),
            ))
        })
    });
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

pub async fn command_history(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        json!({"id": id, "history": s.get_server(&id)?.command_history()}),
    ))
}

fn validate_id(id: &str) -> Result<(), AppError> {
    if id.is_empty() {
        return Err(AppError::Internal("ID cannot be empty".into()));
    }
    if id.contains('/') || id.contains('\\') || id.contains("..") || id.contains('~') {
        return Err(AppError::Internal(
            "ID cannot contain path separators, '..', or '~'".into(),
        ));
    }
    if !id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::Internal(
            "ID can only contain letters, numbers, hyphens, underscores".into(),
        ));
    }
    Ok(())
}

pub fn validate_instance_paths(config: &InstanceConfig) -> Result<(), AppError> {
    let forbidden = ["..", "~"];
    let schemes = ["http://", "https://", "ftp://", "file://"];
    for (name, val) in [
        ("jar_path", &config.jar_path),
        ("java_path", &config.java_path),
    ] {
        let v = val.trim();
        if v.is_empty() {
            return Err(AppError::InvalidPath(format!("{name} must not be empty")));
        }
        for c in &forbidden {
            if v.contains(c) {
                return Err(AppError::InvalidPath(format!("{name} contains '{c}'")));
            }
        }
        for s in &schemes {
            if v.to_lowercase().starts_with(s) {
                return Err(AppError::InvalidPath(format!(
                    "{name} must be a local path"
                )));
            }
        }
    }
    if !config.jar_path.trim().to_lowercase().ends_with(".jar") {
        return Err(AppError::InvalidPath("jar_path must end with .jar".into()));
    }
    let jv_fn = std::path::Path::new(config.java_path.trim())
        .file_name()
        .map(|f| f.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !jv_fn.contains("java") {
        return Err(AppError::InvalidPath(
            "java_path must point to a Java executable".into(),
        ));
    }
    Ok(())
}
