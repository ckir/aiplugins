//! Stable error codes and the tool error envelope.

use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    OpencodeNotFound,
    ServiceDown,
    AuthFailed,
    SessionNotFound,
    NoSession,
    PairNotFound,
    InvalidArgument,
    InvalidRequest,
    RefusedOnHost,
    ConfirmRequired,
    PermissionsLocked,
    MutationBlocked,
    BlockedOperation,
    NonJsonResponse,
    UnknownOperation,
    BodyTooLarge,
    Conflict,
    Deadline,
    IoError,
    LauncherConflict,
    ControllerNotFound,
    CliError,
}

impl ErrorCode {
    pub const ALL: [ErrorCode; 22] = [
        ErrorCode::OpencodeNotFound,
        ErrorCode::ServiceDown,
        ErrorCode::AuthFailed,
        ErrorCode::SessionNotFound,
        ErrorCode::NoSession,
        ErrorCode::PairNotFound,
        ErrorCode::InvalidArgument,
        ErrorCode::InvalidRequest,
        ErrorCode::RefusedOnHost,
        ErrorCode::ConfirmRequired,
        ErrorCode::PermissionsLocked,
        ErrorCode::MutationBlocked,
        ErrorCode::BlockedOperation,
        ErrorCode::NonJsonResponse,
        ErrorCode::UnknownOperation,
        ErrorCode::BodyTooLarge,
        ErrorCode::Conflict,
        ErrorCode::Deadline,
        ErrorCode::IoError,
        ErrorCode::LauncherConflict,
        ErrorCode::ControllerNotFound,
        ErrorCode::CliError,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::OpencodeNotFound => "OPENCODE_NOT_FOUND",
            ErrorCode::ServiceDown => "SERVICE_DOWN",
            ErrorCode::AuthFailed => "AUTH_FAILED",
            ErrorCode::SessionNotFound => "SESSION_NOT_FOUND",
            ErrorCode::NoSession => "NO_SESSION",
            ErrorCode::PairNotFound => "PAIR_NOT_FOUND",
            ErrorCode::InvalidArgument => "INVALID_ARGUMENT",
            ErrorCode::InvalidRequest => "INVALID_REQUEST",
            ErrorCode::RefusedOnHost => "REFUSED_ON_HOST",
            ErrorCode::ConfirmRequired => "CONFIRM_REQUIRED",
            ErrorCode::PermissionsLocked => "PERMISSIONS_LOCKED",
            ErrorCode::MutationBlocked => "MUTATION_BLOCKED",
            ErrorCode::BlockedOperation => "BLOCKED_OPERATION",
            ErrorCode::NonJsonResponse => "NON_JSON_RESPONSE",
            ErrorCode::UnknownOperation => "UNKNOWN_OPERATION",
            ErrorCode::BodyTooLarge => "BODY_TOO_LARGE",
            ErrorCode::Conflict => "CONFLICT",
            ErrorCode::Deadline => "DEADLINE",
            ErrorCode::IoError => "IO_ERROR",
            ErrorCode::LauncherConflict => "LAUNCHER_CONFLICT",
            ErrorCode::ControllerNotFound => "CONTROLLER_NOT_FOUND",
            ErrorCode::CliError => "CLI_ERROR",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolError {
    pub code: ErrorCode,
    pub message: String,
    pub hint: Option<String>,
    pub details: Option<Value>,
}

impl ToolError {
    pub fn new(code: ErrorCode, msg: impl Into<String>) -> Self {
        Self {
            code,
            message: msg.into(),
            hint: None,
            details: None,
        }
    }

    pub fn hint(mut self, hint: &str) -> Self {
        self.hint = Some(hint.to_string());
        self
    }

    pub fn details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn to_json(&self) -> Value {
        let mut error = json!({ "code": self.code.as_str(), "message": self.message });
        if let Some(h) = &self.hint {
            error["hint"] = json!(h);
        }
        if let Some(d) = &self.details {
            error["details"] = d.clone();
        }
        json!({ "error": error })
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for ToolError {}

impl From<std::io::Error> for ToolError {
    fn from(e: std::io::Error) -> Self {
        ToolError::new(ErrorCode::IoError, e.to_string())
    }
}

pub type ToolResult = Result<Value, ToolError>;
