use serde::Serialize;
use ts_rs::TS;

/// Error yang dikirim ke frontend sebagai `{ code, message }`.
/// Pesan untuk pengguna ditulis dalam bahasa Indonesia.
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)] // Forbidden/NotFound dipakai modul domain berikutnya
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Unauthenticated(String),
    #[error("Anda tidak memiliki hak untuk melakukan aksi ini")]
    Forbidden,
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("Terjadi kesalahan database")]
    Database(#[from] rusqlite::Error),
    #[error("Terjadi kesalahan internal: {0}")]
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Serialize, TS)]
#[ts(export, rename = "AppError")]
pub struct ErrorPayload {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Serialize, TS, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum ErrorCode {
    Validation,
    Unauthenticated,
    Forbidden,
    NotFound,
    Conflict,
    Database,
    Internal,
}

impl AppError {
    pub fn code(&self) -> ErrorCode {
        match self {
            AppError::Validation(_) => ErrorCode::Validation,
            AppError::Unauthenticated(_) => ErrorCode::Unauthenticated,
            AppError::Forbidden => ErrorCode::Forbidden,
            AppError::NotFound(_) => ErrorCode::NotFound,
            AppError::Conflict(_) => ErrorCode::Conflict,
            AppError::Database(_) => ErrorCode::Database,
            AppError::Internal(_) => ErrorCode::Internal,
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if let AppError::Database(e) = self {
            tracing::error!(error = %e, "database error");
        }
        ErrorPayload {
            code: self.code(),
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}
