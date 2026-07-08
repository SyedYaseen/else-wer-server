use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};

use crate::{AppState, api::api_error::ApiError, models::user::Claims};

pub struct AuthUser(pub Claims);

impl<S> FromRequestParts<S> for AuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| ApiError::BadRequest("Missing Authorization header".into()))?;

        if !auth_header.starts_with("Bearer ") {
            return Err(ApiError::BadRequest("Invalid auth header".into()));
        }

        let token = &auth_header[7..];

        let app_state = AppState::from_ref(state);

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(app_state.config.jwt_secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        )
        .map_err(|_| ApiError::Unauthorized("Invalid token".into()))?;

        Ok(AuthUser(token_data.claims))
    }
}

/// Auth for the streaming endpoint only: HTML5 `<audio>`/`<video>` elements can't set a custom
/// Authorization header, so this also accepts `?token=<jwt>` as a query param, falling back to
/// the header. Not used on any other route to avoid tokens showing up in access logs/referrers
/// more broadly than necessary.
pub struct StreamAuth(pub Claims);

impl<S> FromRequestParts<S> for StreamAuth
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let header_token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "));

        // JWTs are base64url (A-Z a-z 0-9 - _) joined by '.', which never need percent-decoding,
        // so a plain split is enough here.
        let query_token = parts.uri.query().and_then(|query| {
            query.split('&').find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                (k == "token").then_some(v)
            })
        });

        let token = header_token
            .or(query_token)
            .ok_or_else(|| ApiError::Unauthorized("Missing token".into()))?;

        let app_state = AppState::from_ref(state);

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(app_state.config.jwt_secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        )
        .map_err(|_| ApiError::Unauthorized("Invalid token".into()))?;

        Ok(StreamAuth(token_data.claims))
    }
}
