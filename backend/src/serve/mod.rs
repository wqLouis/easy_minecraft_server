//! HTTP API server — routing and setup.
pub mod admin;
pub mod instances;
pub mod mods;
pub mod ram;
pub mod worlds;

use crate::auth::{self, AppState};
use crate::ip_ban;
use crate::middleware::{check_ip_ban, require_auth, require_sudo};
use crate::settings::{self as settings_mod};
use axum::{
    Router,
    http::HeaderName,
    middleware as mw,
    routing::{delete, get, post, put},
};
use mc_server_manager::registry::ServerRegistry;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

// ── Main serve function ───────────────────────────────────────────

/// Start the HTTP API server. Handles settings loading, optional tmpfs
/// setup, and graceful shutdown.
#[allow(clippy::too_many_arguments)]
pub async fn serve(
    mut pool: SqlitePool,
    settings_path: PathBuf,
    blacklist_path: PathBuf,
    tmpfs: bool,
    _size: Option<String>,
    tmpfs_path: Option<String>,
    database_url: String,
) -> Result<(), Box<dyn std::error::Error>> {
    // ── Load settings and IP ban ──────────────────────────────────
    let app_settings = settings_mod::load_settings(&settings_path);
    let ip_ban_mgr = Arc::new(std::sync::RwLock::new(crate::ip_ban::IpBanManager::new(
        app_settings.fail2ban_max_attempts,
    )));
    let settings = Arc::new(std::sync::RwLock::new(app_settings));
    let loaded = ip_ban::load_blacklist(&blacklist_path)?;
    {
        let mut b = ip_ban_mgr.write().unwrap();
        for ip in &loaded {
            b.add_blacklist(ip);
        }
    }

    // ── Optional tmpfs / RAM disk ────────────────────────────────
    let (ramdisk, active_sp, active_bp, registry) = if tmpfs {
        let app_settings = settings.read().map_err(|e| format!("Settings lock: {e}"))?;
        let servers_dir = PathBuf::from(&app_settings.servers_dir);
        let data_dir = settings_path
            .parent()
            .map_or(PathBuf::from("./data"), |p| p.to_path_buf());
        drop(app_settings);

        // Close DB before copying files so they are in a consistent state.
        pool.close().await;
        let rd = ram::RamDisk::setup(tmpfs_path.as_deref(), &data_dir, &servers_dir)?;

        let new_url = database_url.replacen(
            data_dir.to_string_lossy().as_ref(),
            rd.data_dir.to_string_lossy().as_ref(),
            1,
        );
        pool = crate::db::init_pool(&new_url)
            .await
            .map_err(|e| format!("DB re-init on tmpfs: {e}"))?;

        {
            let mut s = settings
                .write()
                .map_err(|e| format!("Settings lock: {e}"))?;
            s.servers_dir = rd.servers_dir.to_string_lossy().to_string();
        }
        log::info!(
            "Tmpfs ready: data {}, servers {}",
            rd.data_dir.display(),
            rd.servers_dir.display()
        );

        let active_sp = rd.data_dir.join("settings.json");
        let active_bp = rd.data_dir.join("blacklist.json");
        let ip = active_bp
            .parent()
            .map(|p| p.join("instances.json"))
            .unwrap_or_else(|| PathBuf::from("./data/instances.json"));
        let registry = ServerRegistry::new(ip.clone());

        // Remap server_dir paths in existing instances to tmpfs paths.
        for s in registry.list() {
            if let Some((cfg, _)) = registry.get_info(&s.id) {
                if cfg
                    .server_dir
                    .starts_with(rd.original_servers_dir.to_string_lossy().as_ref())
                {
                    let mut u = cfg.clone();
                    u.server_dir = cfg.server_dir.replacen(
                        rd.original_servers_dir.to_string_lossy().as_ref(),
                        rd.servers_dir.to_string_lossy().as_ref(),
                        1,
                    );
                    u.jar_path = format!("{}/server.jar", u.server_dir);
                    let _ = registry.update_config(&s.id, u);
                }
            }
        }

        (Some(rd), active_sp, active_bp, registry)
    } else {
        let ip = blacklist_path
            .parent()
            .map(|p| p.join("instances.json"))
            .unwrap_or_else(|| PathBuf::from("./data/instances.json"));
        let registry = ServerRegistry::new(ip);
        (None, settings_path, blacklist_path, registry)
    };
    let sr = registry.clone();

    // ── Build app state ──────────────────────────────────────────
    let state = Arc::new(AppState {
        db: pool,
        ip_ban: ip_ban_mgr,
        settings: settings.clone(),
        settings_path: active_sp,
        blacklist_path: active_bp,
        server_registry: registry,
        rate_limiter: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        replay_cache: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
    });
    log::info!("Loaded {} blacklisted IP(s)", loaded.len());

    // ── Routes ──────────────────────────────────────────────────
    // Sudo-protected admin routes
    let sudo_routes = Router::new()
        // Auth / users
        .route("/api/auth/register", post(auth::register))
        .route("/api/users", get(auth::list_users))
        .route("/api/users/{id}", delete(auth::delete_user))
        .route("/api/users/{id}", put(auth::update_user))
        // Settings
        .route("/api/settings", get(admin::get_settings))
        .route("/api/settings/schema", get(admin::settings_schema))
        .route("/api/settings", put(admin::update_settings))
        // IP ban
        .route("/api/ipban/status", get(admin::ipban_status))
        .route("/api/ipban/unban", post(admin::ipban_unban))
        .route("/api/ipban/blacklist", post(admin::ipban_add))
        // Providers
        .route("/api/providers", get(instances::list_providers))
        .route("/api/providers/{provider}/versions", get(instances::fetch_versions))
        .route(
            "/api/providers/{provider}/versions/{version}",
            get(instances::fetch_version_info),
        )
        // Instances CRUD
        .route("/api/instances", get(instances::list_instances))
        .route("/api/instances", post(instances::create_instance))
        .route("/api/instances/{id}", get(instances::get_instance))
        .route("/api/instances/{id}", delete(instances::remove_instance))
        .route("/api/instances/{id}/config", put(instances::update_instance_config))
        // Instances control
        .route("/api/instances/{id}/start", post(instances::start_instance))
        .route("/api/instances/{id}/stop", post(instances::stop_instance))
        .route("/api/instances/{id}/command", post(instances::instance_command))
        .route("/api/instances/{id}/logs", get(instances::instance_logs))
        .route("/api/instances/{id}/players", get(instances::instance_players))
        .route("/api/instances/{id}/logs/stream", get(instances::log_stream))
        .route("/api/instances/{id}/properties", get(instances::get_properties))
        .route("/api/instances/{id}/properties", put(instances::update_properties))
        .route("/api/instances/schema", get(instances::instance_config_schema))
        .route("/api/instances/archived", get(instances::list_archived))
        .route("/api/instances/archived/{id}/restore", post(instances::restore_archived))
        .route("/api/instances/{id}/command/history", get(instances::command_history))
        // Worlds
        .route("/api/instances/{id}/worlds", get(worlds::list_worlds))
        .route("/api/instances/{id}/worlds/backup", post(worlds::backup_worlds))
        .route("/api/instances/{id}/backups", get(worlds::list_backups))
        .route("/api/instances/{id}/worlds/upload", post(worlds::upload_world))
        .route("/api/instances/{id}/worlds/{world_name}", delete(worlds::delete_world))
        .route(
            "/api/instances/{id}/worlds/{world_name}/download",
            get(worlds::download_world),
        )
        .route("/api/instances/{id}/worlds/reset", post(worlds::reset_world))
        // Mods / Modrinth
        .route("/api/modrinth/search", get(mods::modrinth_search))
        .route("/api/modrinth/project/{slug}", get(mods::modrinth_project))
        .route(
            "/api/modrinth/project/{slug}/versions",
            get(mods::modrinth_project_versions),
        )
        .route(
            "/api/modrinth/project/{slug}/download-url",
            get(mods::modrinth_download_url),
        )
        .route(
            "/api/modrinth/project/{slug}/dependencies",
            get(mods::modrinth_dependencies),
        )
        .route("/api/instances/{id}/mods", get(mods::list_mods))
        .route("/api/instances/{id}/mods/install", post(mods::install_mod))
        .route("/api/instances/{id}/mods/{filename}", delete(mods::delete_mod))
        .route(
            "/api/instances/{id}/mods/{filename}/toggle",
            put(mods::toggle_mod),
        )
        .route("/api/instances/{id}/mods/modpack", post(mods::generate_modpack))
        .route(
            "/api/instances/{id}/mods/modpack/download",
            get(mods::download_modpack),
        )
        .layer(mw::from_fn(require_sudo));

    // General auth routes (no sudo requirement)
    let app = Router::new()
        .route("/api/health", get(admin::health))
        .route("/api/auth/me", get(auth::me))
        .route("/api/ipwhitelist", get(admin::whitelist_status))
        .merge(sudo_routes)
        .layer(mw::from_fn_with_state(state.clone(), require_auth))
        .layer(mw::from_fn_with_state(state.clone(), check_ip_ban))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers([
                    HeaderName::from_static("content-type"),
                    HeaderName::from_static("authorization"),
                    HeaderName::from_static("x-timestamp"),
                    HeaderName::from_static("x-nonce"),
                ]),
        )
        .with_state(state);

    // ── Listen and serve ────────────────────────────────────────
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind address");
    log::info!("Backend API listening on http://0.0.0.0:3000");

    let sig = {
        let rd = ramdisk.clone();
        async move {
            let ctrl_c = tokio::signal::ctrl_c();
            #[cfg(unix)]
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("SIGTERM handler");
            #[cfg(unix)]
            tokio::select! { _ = ctrl_c => {} _ = term.recv() => {} }
            #[cfg(not(unix))]
            ctrl_c.await.expect("Ctrl+C");
            log::info!("Shutting down...");
            if let Some(rd) = rd {
                log::info!("Syncing tmpfs...");
                let _ = rd.sync_back();
                rd.cleanup();
            }
        }
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(sig)
        .await
        .expect("Server failed");
    log::info!("Killed {} server instance(s)", sr.kill_all().await);
    Ok(())
}
