use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Lỗi API: `{ "error": "<code>", "message": "<tiếng Việt>" }`.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into() }
    }
    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", "cần đăng nhập")
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", msg)
    }
    pub fn host_only() -> Self {
        Self::forbidden("chỉ thực hiện được trên cửa sổ ứng dụng desktop")
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", msg)
    }
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "validation", msg)
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "conflict", msg)
    }
    pub fn too_many() -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "thử quá nhiều lần — vui lòng chờ vài phút")
    }
    pub fn internal(e: impl std::fmt::Display) -> Self {
        tracing::error!(error = %e, "lỗi nội bộ");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", "lỗi nội bộ — xem nhật ký ứng dụng")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, axum::Json(json!({ "error": self.code, "message": self.message }))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::internal(e)
    }
}

impl From<doto_media::MediaError> for ApiError {
    fn from(e: doto_media::MediaError) -> Self {
        use doto_media::MediaError::*;
        match e {
            Validation(m) => ApiError::validation(m),
            NotFound(m) => ApiError::not_found(m),
            Unavailable(m) => ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "unavailable", m),
            InsufficientDisk(m) => ApiError::new(StatusCode::INSUFFICIENT_STORAGE, "insufficient_storage", m),
            Other(e) => ApiError::internal(e),
        }
    }
}

impl From<doto_skills::SkillError> for ApiError {
    fn from(e: doto_skills::SkillError) -> Self {
        use doto_skills::SkillError::*;
        match e {
            Validation(m) => ApiError::validation(m),
            NotFound(m) => ApiError::not_found(m),
            Conflict(m) => ApiError::conflict(m),
            Github(g) => match g {
                doto_skills::github::GithubError::NotFound => ApiError::not_found(g.to_string()),
                doto_skills::github::GithubError::Unauthorized => ApiError::new(StatusCode::BAD_GATEWAY, "github_unauthorized", g.to_string()),
                doto_skills::github::GithubError::Network(_) => ApiError::new(StatusCode::BAD_GATEWAY, "github_network", g.to_string()),
                doto_skills::github::GithubError::Invalid(m) => ApiError::validation(m),
            },
            Other(e) => ApiError::internal(e),
        }
    }
}
