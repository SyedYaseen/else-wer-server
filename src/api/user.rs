use crate::db::user::{self, get_user_by_username};
use crate::models::user::{Claims, User};
use crate::{
    AppState,
    models::user::{LoginDto, UserDto},
};
use anyhow::Result;
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
    State(state): State<AppState>,
    Json(payload): Json<UserDto>,
) -> impl IntoResponse {
    let db = &state.db_pool;

    if payload.username.is_empty() || payload.password.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Provide both username and password" })),
        );
    }

    match save_pwd_hash(&payload, db).await {
        Result::Ok(user) => {
            let message = format!("User {} created successfully", user.username);

            (StatusCode::CREATED, Json(json!({ "message": message })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Auth failed: {}", e) })),
        ),
    }
}

async fn save_pwd_hash(user: &UserDto, db: &Pool<Sqlite>) -> Result<User> {
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
) -> impl IntoResponse {
    let db: &Pool<Sqlite> = &state.db_pool;
    let config = &state.config;

    let jwt = config.jwt_secret.as_ref().unwrap(); // TODO: Handle unwrap properly
    let jwt_bytes = jwt.as_bytes();

    match auth_and_issue_jwt(&payload, db, &jwt_bytes).await {
        Ok(token) => (StatusCode::ACCEPTED, Json(json!({ "token": token }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Login failed: {}", e) })),
        ),
    }
}

async fn auth_and_issue_jwt(
    user_input: &LoginDto,
    db: &Pool<Sqlite>,
    jwt_secret: &[u8],
) -> Result<String> {
    if let Some(user) = get_user_by_username(db, &user_input.username).await? {
        let parsed_hash = PasswordHash::new(&user.password_hash)?;
        Argon2::default()
            .verify_password(user_input.password.as_bytes(), &parsed_hash)
            .map_err(|_| anyhow::anyhow!("Invalid username or password"))?;

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
    } else {
        Err(anyhow::anyhow!("User not found"))
    }
}
