use std::path::Path;

use crate::api::api_error::ApiError;
use crate::api::auth_extractor::AuthUser;
use crate::api::middleware::AdminUser;
use crate::db::user::{self, admin_exists, get_user_by_id, get_user_by_username};
use crate::models::user::{Claims, User};
use crate::{
    AppState,
    models::user::{ChangePasswordDto, DeleteUserDto, LoginDto, UpdateUserPermissionsDto, UserDto, UserSummary},
};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::json;
use sqlx::{Pool, Sqlite};

// Create user
pub async fn create_user(
    AdminUser(_claims): AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<UserDto>,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;

    if payload.username.is_empty() || payload.password.is_empty() {
        return Err(ApiError::BadRequest(
            "Provide both username and password".into(),
        ));
    }

    let user = save_pwd_hash(&payload, db).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "message": format!("User {} created successfully", user.username) })),
    ))
}

// Admin-only: change any user's password (needed since the default admin
// account otherwise has no way to ever change its own password).
pub async fn change_password(
    AdminUser(_claims): AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<ChangePasswordDto>,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;

    if payload.new_password.is_empty() {
        return Err(ApiError::BadRequest("Provide a new password".into()));
    }

    let target = get_user_by_username(db, &payload.username)
        .await?
        .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    let argon2 = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = argon2
        .hash_password(payload.new_password.as_bytes(), &salt)?
        .to_string();

    user::update_user_password(db, target.id, &password_hash).await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "message": format!("Password updated for {}", target.username) })),
    ))
}

// Admin dashboard: list all users (never leaks password_hash/salt).
pub async fn list_users(
    AdminUser(_claims): AdminUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;
    let users: Vec<UserSummary> = user::list_users(db)
        .await?
        .into_iter()
        .map(UserSummary::from)
        .collect();

    Ok((StatusCode::OK, Json(json!({ "users": users }))))
}

// Admin dashboard: delete a user. Can't delete yourself or the last remaining admin.
pub async fn delete_user(
    AdminUser(claims): AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<DeleteUserDto>,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;

    if payload.user_id == claims.sub {
        return Err(ApiError::BadRequest(
            "You can't delete your own account".into(),
        ));
    }

    let target = user::get_user_by_id(db, payload.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("User not found".into()))?;

    if target.is_admin && admin_exists(db).await? <= 1 {
        return Err(ApiError::BadRequest(
            "Can't delete the last remaining admin".into(),
        ));
    }

    user::delete_user(db, payload.user_id).await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "message": format!("User {} deleted", target.username) })),
    ))
}

// Admin dashboard: toggle a user's admin/organize permissions. Can't demote the
// last remaining admin (whether or not it's yourself).
pub async fn update_user_permissions(
    AdminUser(_claims): AdminUser,
    State(state): State<AppState>,
    Json(payload): Json<UpdateUserPermissionsDto>,
) -> Result<impl IntoResponse, ApiError> {
    let db = &state.db_pool;

    let target = user::get_user_by_id(db, payload.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("User not found".into()))?;

    if target.is_admin && !payload.is_admin && admin_exists(db).await? <= 1 {
        return Err(ApiError::BadRequest(
            "Can't remove admin from the last remaining admin".into(),
        ));
    }

    user::update_user_permissions(db, payload.user_id, payload.is_admin, payload.can_organize)
        .await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "message": format!("Permissions updated for {}", target.username) })),
    ))
}

pub async fn save_pwd_hash(user: &UserDto, db: &Pool<Sqlite>) -> Result<User, ApiError> {
    let argon2 = Argon2::default();
    let password_bytes = &user.password.clone().into_bytes();

    let salt = SaltString::generate(&mut OsRng);

    let password_hash = argon2.hash_password(&password_bytes, &salt)?.to_string();

    let user = user::create_user(
        db,
        &user.username,
        &user.is_admin,
        &password_hash,
        &user.can_organize,
    )
    .await?;

    Ok(user)
}

// login
pub async fn login(
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Json(payload): Json<LoginDto>,
) -> Result<impl IntoResponse, ApiError> {
    let db: &Pool<Sqlite> = &state.db_pool;
    let config = &state.config;
    let rate_key = format!("{}:{}", addr.ip(), payload.username);
    state.rate_limiter.check(&rate_key)?;

    let mut token: String = "".to_string();
    if config.self_hosted {
        let jwt = config.jwt_secret.as_bytes();
        match auth_and_issue_jwt(&payload, db, jwt).await {
            Ok(t) => {
                state.rate_limiter.record_success(&rate_key);
                token = t;
            }
            Err(e) => {
                state.rate_limiter.record_failure(&rate_key);
                return Err(e);
            }
        }
    } else {
        token = get_relay_token(state.clone(), &payload).await?;
        let token_path = Path::new(&config.jwt_loc).parent();
        if let Some(path) = token_path {
            tracing::debug!("Parent path is: {:#?}", path);
            tokio::fs::create_dir_all(path).await?;
            tokio::fs::write(&config.jwt_loc, token.clone()).await?;
        }
    }

    Ok((StatusCode::ACCEPTED, Json(json!({"token": token}))))
}

async fn auth_and_issue_jwt(
    user_input: &LoginDto,
    db: &Pool<Sqlite>,
    jwt_secret: &[u8],
) -> Result<String, ApiError> {
    // Same error for "user doesn't exist" and "wrong password" below —
    // distinguishing them lets an attacker enumerate valid usernames.
    let user = get_user_by_username(db, &user_input.username)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("Invalid credentials".to_string()))?;

    let parsed_hash = PasswordHash::new(&user.password_hash)?;
    Argon2::default().verify_password(user_input.password.as_bytes(), &parsed_hash)?;

    issue_jwt(&user, jwt_secret)
}

fn issue_jwt(user: &User, jwt_secret: &[u8]) -> Result<String, ApiError> {
    let now = Utc::now();
    // 30 days rather than hours: the PWA silently re-issues via /refresh_token on
    // every app start/reconnect, so expiry is only ever hit by a device that stayed
    // offline (or logged out) for a full month. Revocation still works through the
    // token_version check on every request.
    let exp = now + Duration::days(30);

    let claims = Claims {
        sub: user.id,
        role: if user.is_admin == true {
            "admin".to_owned()
        } else {
            "user".to_owned()
        },
        username: user.username.clone(),
        iat: now.timestamp() as usize,
        exp: exp.timestamp() as usize,
        can_organize: user.can_organize,
        token_version: user.token_version,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret),
    )?;

    Ok(token)
}

// Silent renewal: exchanges a still-valid token for a fresh 30-day one. Claims are
// rebuilt from the DB row (not copied from the old token) so role/permission changes
// take effect on refresh. AuthUser already enforces signature, expiry and
// token_version, so a revoked or expired token can never be renewed here.
pub async fn refresh_token(
    AuthUser(claims): AuthUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    if !state.config.self_hosted {
        return Err(ApiError::BadRequest(
            "Token refresh is only available in self-hosted mode".into(),
        ));
    }

    let user = get_user_by_id(&state.db_pool, claims.sub)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("Invalid token".into()))?;

    let token = issue_jwt(&user, state.config.jwt_secret.as_bytes())?;
    Ok((StatusCode::OK, Json(json!({ "token": token }))))
}

async fn get_relay_token(state: AppState, payload: &LoginDto) -> Result<String, ApiError> {
    let client = reqwest::Client::new();

    let res = client
        .post(format!("{}/login", state.config.relay_uri))
        .json(&payload)
        .send()
        .await?;

    let relay_token: String = res.json::<serde_json::Value>().await?["token"]
        .as_str()
        .ok_or_else(|| ApiError::Internal("Relay response missing token".into()))?
        .to_string();

    Ok(relay_token)
}
