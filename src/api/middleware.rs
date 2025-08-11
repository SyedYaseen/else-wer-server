use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};

use crate::{api::auth_extractor::AuthUser, models::user::Claims};

pub struct AdminUser(pub Claims);

impl<S> FromRequestParts<S> for AdminUser
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;

        if claims.role != "admin" {
            return Err((StatusCode::FORBIDDEN, "Admin access required".into()));
        }

        Ok(AdminUser(claims))
    }
}
