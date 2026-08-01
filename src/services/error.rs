use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    NotFound,
    Forbidden,
    Validation,
    Conflict,
    Precondition,
    Internal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainError {
    pub kind: ErrorKind,
    pub code: &'static str,
    pub message: String,
}

pub type DomainResult<T> = Result<T, DomainError>;

impl DomainError {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::NotFound,
            code: "not_found",
            message: message.into(),
        }
    }
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Forbidden,
            code: "forbidden",
            message: message.into(),
        }
    }
    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Validation,
            code: "invalid_request",
            message: message.into(),
        }
    }
    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Conflict,
            code,
            message: message.into(),
        }
    }
    pub fn precondition(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Precondition,
            code: "precondition_failed",
            message: message.into(),
        }
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Internal,
            code: "internal_error",
            message: message.into(),
        }
    }
}

impl Display for DomainError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DomainError {}
impl From<String> for DomainError {
    fn from(message: String) -> Self {
        Self::internal(message)
    }
}
impl From<&str> for DomainError {
    fn from(message: &str) -> Self {
        Self::internal(message)
    }
}
impl From<sqlx::Error> for DomainError {
    fn from(error: sqlx::Error) -> Self {
        Self::internal(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{DomainError, ErrorKind};

    #[test]
    fn constructors_assign_stable_kinds() {
        assert_eq!(
            DomainError::not_found("Paste not found").kind,
            ErrorKind::NotFound
        );
        assert_eq!(
            DomainError::precondition("changed").kind,
            ErrorKind::Precondition
        );
        assert_eq!(DomainError::forbidden("denied").kind, ErrorKind::Forbidden);
        assert_eq!(
            DomainError::validation("invalid").kind,
            ErrorKind::Validation
        );
    }
}
