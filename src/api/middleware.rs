use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};

use crate::{
    AppState,
    api::{api_error::ApiError, auth_extractor::AuthUser},
    models::user::Claims,
};

pub struct AdminUser(pub Claims);

impl<S> FromRequestParts<S> for AdminUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;

        if claims.role != "admin" {
            return Err(ApiError::Unauthorized("Admin access required".into()));
        }

        Ok(AdminUser(claims))
    }
}

/// Gates the organize/scan/upload endpoints: admins always pass, everyone else
/// needs the per-user `can_organize` permission (set via the admin dashboard).
pub struct OrganizeUser(pub Claims);

impl<S> FromRequestParts<S> for OrganizeUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;

        if claims.role != "admin" && !claims.can_organize {
            return Err(ApiError::Unauthorized("Organize access required".into()));
        }

        Ok(OrganizeUser(claims))
    }
}
