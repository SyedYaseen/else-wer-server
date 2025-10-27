use axum::response::IntoResponse;

use crate::{api::api_error::ApiError, local_agent::create_jwt};

pub async fn register_agent() -> Result<impl IntoResponse, ApiError> {
    create_jwt();
}
