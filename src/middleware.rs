use std::sync::Arc;

use axum::{
    extract::Request,
    extract::State,
    middleware::Next,
    response::Response,
};

use crate::auth::{client_ip, extract_bearer_token, resolve_user_from_api_key, AppState};
use crate::blacklist;
use crate::errors::AppError;

// ---------------------------------------------------------------------------
// check_ip_ban — reject banned IPs early (runs before auth)
// ---------------------------------------------------------------------------

pub async fn check_ip_ban(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    if let Some(ip) = client_ip(&req) {
        if state.ip_ban.read().unwrap().is_banned(&ip) {
            return Err(AppError::IpBanned);
        }
    }
    Ok(next.run(req).await)
}

// ---------------------------------------------------------------------------
// require_auth — rejects requests with missing/invalid API key
// ---------------------------------------------------------------------------

pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let api_key = match extract_bearer_token(req.headers()) {
        Ok(k) => k,
        Err(e) => {
            record_failure(&state, &req);
            return Err(e);
        }
    };

    let user = match resolve_user_from_api_key(&state.db, &api_key).await {
        Ok(u) => u,
        Err(e) => {
            record_failure(&state, &req);
            return Err(e);
        }
    };

    // Successful auth — clear failure history for this IP
    if let Some(ip) = client_ip(&req) {
        state.ip_ban.write().unwrap().clear_failures(&ip);
    }

    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}

// ---------------------------------------------------------------------------
// require_sudo — rejects if not authenticated or not a sudoer
// ---------------------------------------------------------------------------

pub async fn require_sudo(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let api_key = match extract_bearer_token(req.headers()) {
        Ok(k) => k,
        Err(e) => {
            record_failure(&state, &req);
            return Err(e);
        }
    };

    let user = match resolve_user_from_api_key(&state.db, &api_key).await {
        Ok(u) => u,
        Err(e) => {
            record_failure(&state, &req);
            return Err(e);
        }
    };

    if !user.is_sudoer {
        record_failure(&state, &req);
        return Err(AppError::SudoRequired);
    }

    // Successful auth — clear failure history for this IP
    if let Some(ip) = client_ip(&req) {
        state.ip_ban.write().unwrap().clear_failures(&ip);
    }

    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn record_failure(state: &Arc<AppState>, req: &Request) {
    let Some(ip) = client_ip(req) else { return };

    let newly_banned = state.ip_ban.write().unwrap().record_failure(&ip);
    if newly_banned {
        log::warn!("IP {} blacklisted after too many failed auth attempts", ip);
        // Persist to blacklist.json immediately
        let ips: Vec<String> = state.ip_ban.read().unwrap().blacklist().to_vec();
        let _ = blacklist::save_blacklist(&state.blacklist_path, &ips);
    }
}
