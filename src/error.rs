use std::string::FromUtf8Error;

use axum::{http::StatusCode, response::IntoResponse, Json};
use borderless_hash::Hash256;
use serde_json::json;
use thiserror::Error;

use crate::models;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Bincode error - {0}")]
    Bincode(#[from] bincode::Error),
    #[error("Storage error - {0}")]
    Storage(#[from] borderless_kv_store::Error),
    #[error("Duplicated Key - {0}")]
    Dublicated(Hash256),
    #[error("No entry in storage for key - {0}")]
    NoPkg(Hash256),
    #[error("Database error - {0}")]
    Database(#[from] sea_orm::error::DbErr),
    #[error("Invalid source type")]
    InvalidSource,
    #[error("Url encoding error")]
    UrlEncoding,
    #[error("Invalid path!")]
    InvalidPath,
    #[error("Oci path error - {0} ")]
    Oci(#[from] models::Error),
    #[error("UTF-8 error - {0}")]
    UTF8(#[from] FromUtf8Error),
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match &self {
            Error::Bincode(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::Dublicated(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::NoPkg(_) => (StatusCode::NOT_FOUND, self.to_string()),
            Error::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::InvalidSource => (StatusCode::NO_CONTENT, self.to_string()),
            Error::UrlEncoding => (StatusCode::BAD_REQUEST, self.to_string()),
            Error::InvalidPath => (StatusCode::BAD_GATEWAY, self.to_string()),
            Error::Oci(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::UTF8(_) => (StatusCode::BAD_REQUEST, self.to_string()),
        };

        let body = Json(json!({
            "error": {
                "message": error_message,
                "status": status.as_u16()
            }
        }));

        (status, body).into_response()
    }
}
