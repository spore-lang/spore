//! Type checker errors.

use std::fmt;

/// Error codes for the Spore type checker (SEP-0006 scheme).
///
/// Prefixes:
///   E0xxx — Type errors
///   W0xxx — Warnings
///   C0xxx — Capability violations
///   K0xxx — Cost violations
///   H0xxx — Hole diagnostics
///   M0xxx — Module errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // ── Type errors (E0xxx) ─────────────────────────────────────────
    E0001, // type mismatch
    E0002, // cannot apply operator to type
    E0003, // infinite type (occurs check)
    E0004, // undefined variable
    E0005, // undefined struct
    E0006, // unknown variant
    E0007, // wrong number of arguments
    E0008, // cannot call non-function
    E0009, // pipe target not a function
    E0010, // non-exhaustive match
    E0011, // pattern type mismatch
    E0012, // missing error types in throws
    E0013, // impl missing method
    E0014, // extra method in impl
    E0015, // no such field
    E0016, // type has no fields
    E0017, // match guard must be Bool

    // ── Capability violations (C0xxx) ───────────────────────────────
    C0001, // missing capabilities
    C0002, // unknown capability

    // ── Warnings (W0xxx) ────────────────────────────────────────────
    // (reserved for future use)

    // ── Cost violations (K0xxx) ─────────────────────────────────────
    // (reserved for future use)

    // ── Hole diagnostics (H0xxx) ────────────────────────────────────
    // (reserved for future use)

    // ── Module errors (M0xxx) ───────────────────────────────────────
    M0001, // module not found
    M0002, // symbol not found in module
    M0003, // private symbol not accessible
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            ErrorCode::E0001 => "E0001",
            ErrorCode::E0002 => "E0002",
            ErrorCode::E0003 => "E0003",
            ErrorCode::E0004 => "E0004",
            ErrorCode::E0005 => "E0005",
            ErrorCode::E0006 => "E0006",
            ErrorCode::E0007 => "E0007",
            ErrorCode::E0008 => "E0008",
            ErrorCode::E0009 => "E0009",
            ErrorCode::E0010 => "E0010",
            ErrorCode::E0011 => "E0011",
            ErrorCode::E0012 => "E0012",
            ErrorCode::E0013 => "E0013",
            ErrorCode::E0014 => "E0014",
            ErrorCode::E0015 => "E0015",
            ErrorCode::E0016 => "E0016",
            ErrorCode::E0017 => "E0017",
            ErrorCode::C0001 => "C0001",
            ErrorCode::C0002 => "C0002",
            ErrorCode::M0001 => "M0001",
            ErrorCode::M0002 => "M0002",
            ErrorCode::M0003 => "M0003",
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
