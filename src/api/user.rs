use crate::api::api_error::ApiError;
use crate::api::middleware::AdminUser;
use crate::db::user::{self, get_user_by_username};
use crate::models::user::{Claims, User};
use crate::{
    AppState,
    models::user::{LoginDto, UserDto},
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

    let jwt = config.jwt_secret.as_ref().unwrap().as_bytes(); // TODO: Handle unwrap properly

    let token = auth_and_issue_jwt(&payload, db, jwt).await?;
    Ok((StatusCode::ACCEPTED, Json(json!({"token": token}))))
}

async fn auth_and_issue_jwt(
    user_input: &LoginDto,
    db: &Pool<Sqlite>,
    jwt_secret: &[u8],
) -> Result<String, ApiError> {
    let user = get_user_by_username(db, &user_input.username)
        .await?
        .ok_or_else(|| ApiError::BadRequest("User not found".to_string()))?;

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
