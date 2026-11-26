use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

/// An API error: an HTTP status, a stable snake_case code and a message written for people.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub details: Option<Value>,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        ApiError { status, code, message: message.into(), details: None }
    }
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", "Sign in to continue.")
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", msg)
    }
    pub fn not_found(what: &str) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", format!("{what} was not found."))
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "conflict", msg)
    }
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid", msg)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut err = json!({ "code": self.code, "message": self.message });
        if let Some(d) = self.details {
            err["details"] = d;
        }
        (self.status, Json(json!({ "error": err }))).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        if let sqlx::Error::RowNotFound = e {
            return ApiError::not_found("That item");
        }
        if let Some(db) = e.as_database_error() {
            if db.is_unique_violation() {
                return ApiError::conflict("Something with that name already exists.");
            }
        }
        tracing::error!(error = %e, "database error");
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", "The server hit a database error. Try again; if it keeps happening, check the server log.")
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(error = %format!("{e:#}"), "internal error");
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", format!("The server hit an error: {e}"))
    }
}

impl From<object_store::Error> for ApiError {
    fn from(e: object_store::Error) -> Self {
        if let object_store::Error::NotFound { .. } = e {
            return ApiError::not_found("That stored data");
        }
        tracing::error!(error = %e, "storage error");
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "storage", "The server could not read or write its storage. Check that the storage point is reachable.")
    }
}
