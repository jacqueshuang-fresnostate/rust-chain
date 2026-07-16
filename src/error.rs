use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use thiserror::Error;
use tracing::error;
use utoipa::ToSchema;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(#[from] config::ConfigError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("mongo error: {0}")]
    Mongo(#[from] mongodb::error::Error),
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("rabbitmq error: {0}")]
    RabbitMq(#[from] lapin::Error),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("validation error: {0}")]
    Validation(String),
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal error: {0}")]
    Internal(String),
    #[error("{message}")]
    Api {
        status: StatusCode,
        code: &'static str,
        message: String,
    },
}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Api { status, .. } => *status,
            Self::Config(_)
            | Self::Database(_)
            | Self::Mongo(_)
            | Self::Redis(_)
            | Self::RabbitMq(_)
            | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::Config(_) => "CONFIG_ERROR",
            Self::Database(_) => "DATABASE_ERROR",
            Self::Mongo(_) => "MONGO_ERROR",
            Self::Redis(_) => "REDIS_ERROR",
            Self::RabbitMq(_) => "RABBITMQ_ERROR",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Forbidden => "FORBIDDEN",
            Self::Validation(_) => "VALIDATION_ERROR",
            Self::NotFound => "NOT_FOUND",
            Self::Conflict(_) => "CONFLICT",
            Self::Internal(_) => "INTERNAL_ERROR",
            Self::Api { code, .. } => code,
        }
    }

    pub fn security_validation(code: &'static str, message: impl Into<String>) -> Self {
        Self::Api {
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
        }
    }

    pub fn security_forbidden(code: &'static str, message: impl Into<String>) -> Self {
        Self::Api {
            status: StatusCode::FORBIDDEN,
            code,
            message: message.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status_code();

        if status.is_server_error() {
            error!(error = %self, "请求处理失败");
        }

        let body = ErrorResponse {
            code: self.code(),
            message: self.to_string(),
        };

        (status, Json(body)).into_response()
    }
}
