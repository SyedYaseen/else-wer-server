use axum::{extract::FromRequestParts, http::request::Parts};

use crate::{
    api::{api_error::ApiError, auth_extractor::AuthUser},
    models::user::Claims,
};

pub struct AdminUser(pub Claims);

impl<S> FromRequestParts<S> for AdminUser
where
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
