use serde::{Deserialize, Serialize};
use std::fmt;

/// RPC status codes (inspired by gRPC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum RpcStatus {
    Ok = 0,
    Cancelled = 1,
    InvalidArgument = 2,
    NotFound = 3,
    AlreadyExists = 4,
    PermissionDenied = 5,
    Unauthenticated = 6,
    ResourceExhausted = 7,
    Internal = 8,
    Unavailable = 9,
    DeadlineExceeded = 10,
}

impl fmt::Display for RpcStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RpcStatus::Ok => write!(f, "OK"),
            RpcStatus::Cancelled => write!(f, "CANCELLED"),
            RpcStatus::InvalidArgument => write!(f, "INVALID_ARGUMENT"),
            RpcStatus::NotFound => write!(f, "NOT_FOUND"),
            RpcStatus::AlreadyExists => write!(f, "ALREADY_EXISTS"),
            RpcStatus::PermissionDenied => write!(f, "PERMISSION_DENIED"),
            RpcStatus::Unauthenticated => write!(f, "UNAUTHENTICATED"),
            RpcStatus::ResourceExhausted => write!(f, "RESOURCE_EXHAUSTED"),
            RpcStatus::Internal => write!(f, "INTERNAL"),
            RpcStatus::Unavailable => write!(f, "UNAVAILABLE"),
            RpcStatus::DeadlineExceeded => write!(f, "DEADLINE_EXCEEDED"),
        }
    }
}

/// Structured RPC error with status code and message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub status: RpcStatus,
    pub message: String,
}

impl RpcError {
    pub fn new(status: RpcStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(RpcStatus::Internal, message)
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(RpcStatus::InvalidArgument, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(RpcStatus::NotFound, message)
    }

    pub fn method_not_found(method_id: u16) -> Self {
        Self::new(
            RpcStatus::NotFound,
            format!("Method not found: {}", method_id),
        )
    }

    pub fn service_not_found(service_id: u16) -> Self {
        Self::new(
            RpcStatus::NotFound,
            format!("Service not found: {}", service_id),
        )
    }

    pub fn codec_error(message: impl Into<String>) -> Self {
        Self::new(RpcStatus::Internal, message)
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(RpcStatus::Unavailable, message)
    }

    pub fn deadline_exceeded(message: impl Into<String>) -> Self {
        Self::new(RpcStatus::DeadlineExceeded, message)
    }
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.status, self.message)
    }
}

impl std::error::Error for RpcError {}

impl From<std::io::Error> for RpcError {
    fn from(err: std::io::Error) -> Self {
        RpcError::internal(err.to_string())
    }
}

impl From<bincode::Error> for RpcError {
    fn from(err: bincode::Error) -> Self {
        RpcError::codec_error(format!("bincode: {}", err))
    }
}
