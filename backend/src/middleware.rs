//! Axum middleware — IP ban check, auth, sudo check, rate limiter.
use std::sync::Arc;
use std::time::Instant;
use axum::{extract::{Request, State}, middleware::Next, response::Response};
use crate::auth::{check_ip_whitelist, client_ip, extract_credentials, resolve_user, AppState};
use crate::errors::AppError;
use crate::ip_ban;
use crate::models::User;

pub async fn check_ip_ban(State(s): State<Arc<AppState>>, req: Request, next: Next) -> Result<Response, AppError> {
    let tp = s.settings.read().map(|s| s.trust_proxy_headers).unwrap_or(false);
    if let Some(ip) = client_ip(&req, tp) { if s.ip_ban.read().unwrap().is_banned(&ip) { return Err(AppError::IpBanned); } }
    Ok(next.run(req).await)
}

fn check_rate_limit(state: &Arc<AppState>, req: &Request) -> Result<(), AppError> {
    let ip = client_ip(req, state.settings.read().map(|s| s.trust_proxy_headers).unwrap_or(false)).unwrap_or_else(|| "unknown".into());
    let now = Instant::now();
    let mut limiter = state.rate_limiter.lock().unwrap();
    let entries = limiter.entry(ip).or_insert_with(Vec::new);
    entries.retain(|t| now.duration_since(*t).as_secs() < 60);
    if entries.len() >= 60 { return Err(AppError::Internal("Rate limit exceeded (60 req/min)".into())); }
    entries.push(now);
    Ok(())
}

pub async fn require_auth(State(s): State<Arc<AppState>>, mut req: Request, next: Next) -> Result<Response, AppError> {
    check_rate_limit(&s, &req)?;
    let tp = s.settings.read().map(|s| s.trust_proxy_headers).unwrap_or(false);
    let (username, token) = extract_credentials(req.headers()).map_err(|e| { record(&s, &req, tp); e })?;
    let user = resolve_user(&s.db, &username, &token).await.map_err(|e| { record(&s, &req, tp); e })?;
    if let Some(ip) = client_ip(&req, tp) { s.ip_ban.write().unwrap().clear_failures(&ip); }
    if let Some(ip) = client_ip(&req, tp) { check_ip_whitelist(&s.db, &user.id, &ip, s.settings.read().map(|s| s.ip_whitelist_enabled).unwrap_or(false)).await?; }
    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}

pub async fn require_sudo(req: Request, next: Next) -> Result<Response, AppError> {
    if !req.extensions().get::<User>().ok_or(AppError::ApiKeyNotFound)?.is_sudoer { return Err(AppError::SudoRequired); }
    Ok(next.run(req).await)
}

fn record(s: &Arc<AppState>, req: &Request, trust_proxy: bool) {
    if let Some(ip) = client_ip(req, trust_proxy) { if s.ip_ban.write().unwrap().record_failure(&ip) { log::warn!("IP {ip} blacklisted"); let _ = ip_ban::save_blacklist(&s.blacklist_path, &s.ip_ban.read().unwrap().blacklist().to_vec()); } }
}
