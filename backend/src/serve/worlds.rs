//! World management handlers.
use crate::auth::AppState;
use crate::errors::AppError;
use crate::models::User;
use axum::{
    Extension, Json,
    body::Body,
    extract::{Multipart, Path, State},
    response::Response as AxResponse,
};
use chrono::Utc;
use mc_server_manager::world;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

fn validate_world_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::Internal("World name empty".into()));
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.contains('~') {
        return Err(AppError::Internal("Invalid world name".into()));
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::Internal(
            "Only letters, numbers, -, _ allowed".into(),
        ));
    }
    Ok(())
}

pub async fn list_worlds(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = s.get_server(&id)?;
    Ok(Json(
        json!({"id": id, "worlds": server.list_worlds().map_err(|e| AppError::Internal(e.to_string()))?, "server_dir": server.server_dir().to_string_lossy(), "running": server.handle().is_running()}),
    ))
}

#[derive(Deserialize)]
pub struct BackupReq {
    pub world_name: Option<String>,
    pub include_all: Option<bool>,
}
pub async fn backup_worlds(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
    Json(body): Json<BackupReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = s.get_server(&id)?;
    let include_all = body.include_all.unwrap_or(true);
    let worlds: Vec<String> = if include_all {
        let w = server
            .list_worlds()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if w.is_empty() {
            return Err(AppError::Internal("No worlds found".into()));
        }
        w.into_iter().map(|w| w.name).collect()
    } else if let Some(ref name) = body.world_name {
        validate_world_name(name)?;
        vec![name.clone()]
    } else {
        return Err(AppError::Internal(
            "Specify world_name or include_all=true".into(),
        ));
    };
    let backup_path = server.backups_dir().join(format!(
        "{}-worlds-{}.zip",
        id,
        Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    server
        .backup_worlds(&worlds, &backup_path)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(
        json!({"id": id, "backup_file": backup_path.to_string_lossy(), "size_bytes": std::fs::metadata(&backup_path).map(|m| m.len()).unwrap_or(0), "worlds_included": worlds, "created_at": Utc::now().to_rfc3339()}),
    ))
}

pub async fn list_backups(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        json!({"id": id, "backups": s.get_server(&id)?.list_backups().map_err(|e| AppError::Internal(e.to_string()))?}),
    ))
}

pub async fn download_world(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path((id, w)): Path<(String, String)>,
) -> Result<AxResponse, AppError> {
    let server = s.get_server(&id)?;
    let world_path = server.server_dir().join(&w);
    if !world_path.is_dir() || !world::is_minecraft_world(&world_path) {
        return Err(AppError::Internal(format!(
            "World '{w}' not found or invalid"
        )));
    }
    let zip_data = world::create_worlds_zip(&[(&w, &world_path)])
        .map_err(|e| AppError::Internal(e.to_string()))?;
    AxResponse::builder()
        .header("Content-Type", "application/zip")
        .header(
            "Content-Disposition",
            format!("attachment; filename=\"{w}.zip\""),
        )
        .body(Body::from(zip_data))
        .map_err(|e| AppError::Internal(e.to_string()))
}

pub async fn delete_world(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path((id, w)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = s.get_server(&id)?;
    if server.handle().is_running() {
        return Err(AppError::Internal(
            "Cannot delete world while server is running".into(),
        ));
    }
    server
        .delete_world_dir(&w)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({"deleted": true, "world_name": w, "id": id})))
}

pub async fn upload_world(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
    mut mp: Multipart,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = s.get_server(&id)?;
    let mut world_name = None;
    while let Some(field) = mp
        .next_field()
        .await
        .map_err(|e| AppError::Internal(format!("Multipart error: {e}")))?
    {
        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::Internal(format!("Read error: {e}")))?;
        world_name = Some(
            server
                .extract_world_zip(&data)
                .map_err(|e| AppError::Internal(e.to_string()))?,
        );
    }
    Ok(Json(
        json!({"uploaded": true, "world_name": world_name.unwrap_or_else(|| "unknown".into()), "id": id}),
    ))
}

#[derive(Deserialize)]
pub struct ResetWorldReq {
    pub world_name: String,
}
pub async fn reset_world(
    State(s): State<Arc<AppState>>,
    Extension(_u): Extension<User>,
    Path(id): Path<String>,
    Json(body): Json<ResetWorldReq>,
) -> Result<Json<serde_json::Value>, AppError> {
    let server = s.get_server(&id)?;
    let was_running = server.handle().is_running();
    if was_running {
        s.server_registry
            .stop(&id)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }
    server
        .delete_world_dir(&body.world_name)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    if was_running {
        s.server_registry
            .start(&id)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }
    Ok(Json(
        json!({"reset": true, "world_name": body.world_name, "id": id}),
    ))
}
