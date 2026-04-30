use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use sqlx::error::Error as SqlxError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{message}")]
    BadRequest { code: &'static str, message: String },
    #[error("{message}")]
    NotFound { code: &'static str, message: String },
    #[error("{message}")]
    Conflict { code: &'static str, message: String },
    #[error("{message}")]
    RateLimited { code: &'static str, message: String },
    #[error("{message}")]
    Upstream { code: &'static str, message: String },
    #[error("{message}")]
    Database { code: &'static str, message: String },
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: ErrorPayload<'a>,
}

#[derive(Serialize)]
struct ErrorPayload<'a> {
    code: &'a str,
    message: &'a str,
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest {
            code: "BAD_REQUEST",
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound {
            code: "NOT_FOUND",
            message: message.into(),
        }
    }

    pub fn already_voted() -> Self {
        Self::Conflict {
            code: "ALREADY_VOTED",
            message: "You have already voted for this machine today".to_string(),
        }
    }

    pub fn rate_limited() -> Self {
        Self::RateLimited {
            code: "RATE_LIMITED",
            message: "Too many votes from this IP range for the same machine".to_string(),
        }
    }

    pub fn upstream(message: impl Into<String>) -> Self {
        Self::Upstream {
            code: "UPSTREAM_ERROR",
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Database {
            code: "INTERNAL_ERROR",
            message: message.into(),
        }
    }

    pub fn database(message: impl Into<String>) -> Self {
        Self::Database {
            code: "DATABASE_ERROR",
            message: message.into(),
        }
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::Conflict { .. } => StatusCode::CONFLICT,
            Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::Upstream { .. } => StatusCode::BAD_GATEWAY,
            Self::Database { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::BadRequest { code, .. }
            | Self::NotFound { code, .. }
            | Self::Conflict { code, .. }
            | Self::RateLimited { code, .. }
            | Self::Upstream { code, .. }
            | Self::Database { code, .. } => code,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let message = self.to_string();
        let status = self.status_code();
        let body = ErrorBody {
            error: ErrorPayload {
                code: self.code(),
                message: &message,
            },
        };
        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        match value {
            SqlxError::RowNotFound => AppError::not_found("Resource not found"),
            SqlxError::Database(db_err) => {
                if db_err.code().as_deref() == Some("23505") {
                    return AppError::already_voted();
                }
                AppError::database(db_err.message().to_string())
            }
            other => AppError::database(other.to_string()),
        }
    }
}
