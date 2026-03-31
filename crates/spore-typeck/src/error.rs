//! Type checker errors.

use std::fmt;

/// Error codes for the Spore type checker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // Type mismatch errors (E0xx)
    E001, // type mismatch
    E002, // cannot apply operator to type
    E003, // infinite type (occurs check)

    // Definition errors (E1xx)
    E101, // undefined variable
    E102, // undefined struct
    E103, // unknown variant
    E104, // unknown capability

    // Function/call errors (E2xx)
    E201, // wrong number of arguments
    E202, // cannot call non-function
    E203, // pipe target not a function

    // Pattern errors (E3xx)
    E301, // non-exhaustive match
    E302, // pattern type mismatch

    // Capability/effect errors (E4xx)
    E401, // missing capabilities
    E402, // missing error types in throws
    E403, // impl missing method
    E404, // extra method in impl

    // Field errors (E5xx)
    E501, // no such field
    E502, // type has no fields

    // Guard errors (E6xx)
    E601, // match guard must be Bool
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            ErrorCode::E001 => "E001",
            ErrorCode::E002 => "E002",
            ErrorCode::E003 => "E003",
            ErrorCode::E101 => "E101",
            ErrorCode::E102 => "E102",
            ErrorCode::E103 => "E103",
            ErrorCode::E104 => "E104",
            ErrorCode::E201 => "E201",
            ErrorCode::E202 => "E202",
            ErrorCode::E203 => "E203",
            ErrorCode::E301 => "E301",
            ErrorCode::E302 => "E302",
            ErrorCode::E401 => "E401",
            ErrorCode::E402 => "E402",
            ErrorCode::E403 => "E403",
            ErrorCode::E404 => "E404",
            ErrorCode::E501 => "E501",
            ErrorCode::E502 => "E502",
            ErrorCode::E601 => "E601",
        };
        write!(f, "{code}")
    }
}

#[derive(Debug, Clone)]
pub struct TypeError {
    pub code: ErrorCode,
    pub message: String,
}

impl TypeError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for TypeError {}
