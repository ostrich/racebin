use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    NotFound,
    Unauthorized,
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
    fn new(kind: ErrorKind, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            code,
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, "not_found", message)
    }
    pub fn unauthorized(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unauthorized, code, message)
    }
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Forbidden, "forbidden", message)
    }
    pub fn forbidden_code(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Forbidden, code, message)
    }
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Validation, "invalid_request", message)
    }
    pub fn validation_code(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Validation, code, message)
    }
    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Conflict, code, message)
    }
    pub fn precondition(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Precondition, "precondition_failed", message)
    }
    pub fn internal(error: impl Display) -> Self {
        Self::new(ErrorKind::Internal, "internal_error", error.to_string())
    }
}

impl Display for DomainError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DomainError {}
impl From<sqlx::Error> for DomainError {
    fn from(error: sqlx::Error) -> Self {
        Self::internal(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{DomainError, ErrorKind};

    #[test]
    fn constructors_assign_stable_kinds_and_codes() {
        assert_eq!(DomainError::not_found("missing").kind, ErrorKind::NotFound);
        assert_eq!(
            DomainError::unauthorized("invalid_token", "bad").kind,
            ErrorKind::Unauthorized
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
        assert_eq!(
            DomainError::validation_code("invalid_password", "bad").code,
            "invalid_password"
        );
    }
}
