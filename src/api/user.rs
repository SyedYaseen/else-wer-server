use std::path::Path;

use crate::api::api_error::ApiError;
use crate::api::middleware::AdminUser;
use crate::db::user::{self, get_user_by_username};
use crate::models::user::{Claims, User};
use crate::{
    AppState,
    models::user::{ChangePasswordDto, LoginDto, UserDto},
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

    user::update_user_password(db, target.id, &password_hash, &salt.to_string()).await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "message": format!("Password updated for {}", target.username) })),
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
        &salt.to_string(),
    )
    .await?;

    Ok(user)
}

// login
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginDto>,
) -> Result<impl IntoResponse, ApiError> {
    let db: &Pool<Sqlite> = &state.db_pool;
    let config = &state.config;
    let mut token: String = "".to_string();
    if config.self_hosted {
        let jwt = config.jwt_secret.as_bytes();
        token = auth_and_issue_jwt(&payload, db, jwt).await?;
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

    let now = Utc::now();
    let exp = now + Duration::hours(24); // token valid for 24 hours

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
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret),
    )?;

    Ok(token)
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
