//! `POST /api/register` - create a tenant. Persists a workspace, a user (with
//! a real argon2-hashed password, issue #100), and the membership joining
//! them, then returns `{ workspace_id, key_token, refresh_token }`.
//!
//! An existing email is now a real 409 Conflict (log in instead via
//! `POST /api/login`) rather than the old soft "re-issue a token for
//! whoever owns this email, no password required" behavior - that was the
//! actual security gap issue #100 closes: anyone who knew (or guessed) a
//! registered email could mint themselves a valid token for that tenant.

use crate::auth::{hash_password, issue_token};
use crate::error::{ApiError, ApiResult};
use crate::ids::{new_id, now_secs};
use crate::routes::login::create_session;
use crate::state::AppState;
use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub struct RegisterRequest {
    email: String,
    name: String,
    password: String,
    workspace_name: String,
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> ApiResult<Json<Value>> {
    let email = req.email.trim();
    if email.is_empty() || req.name.trim().is_empty() || req.workspace_name.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "email, name and workspace_name are required".into(),
        ));
    }
    if req.password.len() < 8 {
        return Err(ApiError::BadRequest("password must be at least 8 characters".into()));
    }

    if email_exists(&state, email).await? {
        return Err(ApiError::BadRequest(format!(
            "an account for '{email}' already exists - log in via POST /api/login instead"
        )));
    }

    let password_hash =
        hash_password(&req.password).map_err(|e| ApiError::Internal(format!("password hashing failed: {e}")))?;

    let now = now_secs();
    let user_id = new_id("usr");
    let workspace_id = new_id("ws");
    let membership_id = new_id("memb");

    state
        .conn
        .execute(
            "INSERT INTO workspaces (id, name, plan, limits, created_at, updated_at) \
             VALUES (?1, ?2, 'free', '{}', ?3, ?4)",
            libsql::params![workspace_id.clone(), req.workspace_name, now, now],
        )
        .await?;
    state
        .conn
        .execute(
            "INSERT INTO users (id, email, name, password_hash, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            libsql::params![user_id.clone(), email, req.name, password_hash, now, now],
        )
        .await?;
    state
        .conn
        .execute(
            "INSERT INTO memberships (id, user_id, workspace_id, role, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'owner', ?4, ?5)",
            libsql::params![membership_id, user_id.clone(), workspace_id.clone(), now, now],
        )
        .await?;

    let key_token = issue_token(&state.config.jwt_secret, &user_id, &workspace_id, email);
    let refresh_token = create_session(&state, &user_id, &workspace_id).await?;
    tracing::info!(%user_id, %workspace_id, "registered new tenant");

    Ok(Json(json!({
        "user_id": user_id,
        "workspace_id": workspace_id,
        "key_token": key_token,
        "refresh_token": refresh_token,
        "status": "registered",
    })))
}

async fn email_exists(state: &AppState, email: &str) -> ApiResult<bool> {
    let mut rows = state
        .conn
        .query("SELECT 1 FROM users WHERE email = ?1", libsql::params![email])
        .await?;
    Ok(rows.next().await?.is_some())
}
