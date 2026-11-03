use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Validation,
    NotFound,
    Database,
    Credential,
    Internal,
}

/// Error returned to the frontend. Messages are user-safe and never contain secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    /// Input field a validation error refers to, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl AppError {
    pub fn validation(field: &str, message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Validation,
            message: message.into(),
            field: Some(field.to_string()),
        }
    }

    pub fn not_found(what: &str) -> Self {
        Self {
            code: ErrorCode::NotFound,
            message: format!("{what} was not found."),
            field: None,
        }
    }

    pub fn database(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Database,
            message: message.into(),
            field: None,
        }
    }

    pub fn credential(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Credential,
            message: message.into(),
            field: None,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Internal,
            message: message.into(),
            field: None,
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        // Detailed SQLite errors go to stderr only; the UI gets a generic message.
        eprintln!("database error: {err}");
        AppError::database("A database operation failed.")
    }
}
