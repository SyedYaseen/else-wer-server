use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};

use crate::models::user::Claims;

pub struct AuthUser(pub Claims);

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "Missing Authorization header".into(),
            ))?;

        if !auth_header.starts_with("Bearer ") {
            return Err((StatusCode::UNAUTHORIZED, "Invalid auth header".into()));
        }

        let token = &auth_header[7..];

        let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        )
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token".into()))?;

        Ok(AuthUser(token_data.claims))
    }
}
