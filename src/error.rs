use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    #[error("Authorization failed: {0}")]
    AuthorizationError(String),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Application not found: {0}")]
    ApplicationNotFound(String),

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Invalid input: {0}")]
    BadRequest(String),

    #[error("Internal server error: {0}")]
    InternalServerError(String),

    #[error("JSON serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Error: {0}")]
    NotFound(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::AuthenticationError(_) => (StatusCode::UNAUTHORIZED, "Authentication failed"),
            AppError::AuthorizationError(_) => (StatusCode::FORBIDDEN, "Authorization failed"),
            AppError::ChannelError(_) => (StatusCode::BAD_REQUEST, "Channel error"),
            AppError::ConnectionError(_) => (StatusCode::BAD_REQUEST, "Connection error"),
            AppError::ApplicationNotFound(_) => (StatusCode::NOT_FOUND, "Application not found"),
            AppError::ChannelNotFound(_) => (StatusCode::NOT_FOUND, "Channel not found"),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "Invalid input"),
            AppError::InternalServerError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
            AppError::SerializationError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Serialization error")
            }
            AppError::IoError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "I/O error"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        };

        let body = Json(json!({
            "error": error_message,
            "message": self.to_string(),
        }));

        (status, body).into_response()
    }
}

// Utility function to convert any error to AppError
pub fn to_app_error<E>(err: E) -> AppError
where
    E: std::error::Error + Send + Sync + 'static,
{
    AppError::InternalServerError(err.to_string())
}
