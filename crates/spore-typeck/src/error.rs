//! Type checker errors.

use std::fmt;

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
        }
    }
}

/// Error codes for the Spore type checker (SEP-0006 scheme).
///
/// Prefixes:
///   E0xxx — Type errors        (33 codes)
///   W0xxx — Warnings           (10 codes)
///   C0xxx — Capability errors  ( 7 codes)
///   K0xxx — Cost errors        ( 6 codes)
///   R0xxx — Refinement errors  ( 1 code)
///   H0xxx — Hole diagnostics   ( 8 codes)
///   M0xxx — Module errors      (11 codes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    // ── E01xx: Struct errors ────────────────────────────────────────
    E0101, // missing required struct field
    E0102, // extra field in struct literal
    E0103, // duplicate field in struct literal
    E0104, // struct field type mismatch
    E0105, // recursive struct without indirection

    // ── E02xx: Argument / call errors ───────────────────────────────
    E0201, // wrong number of arguments
    E0202, // argument type mismatch
    E0203, // cannot call non-function
    E0204, // pipe target not a function
    E0205, // pipe target wrong arity
    E0206, // missing error types in throws

    // ── E03xx: Type context / mismatch ──────────────────────────────
    E0301, // type mismatch (general)
    E0302, // cannot apply operator to type
    E0303, // if condition must be Bool
    E0304, // match guard must be Bool
    E0305, // non-exhaustive match
    E0306, // pattern type mismatch
    E0307, // infinite type (occurs check)
    E0308, // expected type, found different
    E0309, // await expects Task[T]

    // ── E04xx: Generics / type parameters ───────────────────────────
    E0401, // wrong number of type arguments
    E0402, // type parameter out of scope
    E0403, // constraint not satisfied
    E0404, // cannot infer type parameter

    // ── E05xx: Patterns ─────────────────────────────────────────────
    E0501, // unknown variant in pattern
    E0502, // wrong number of variant fields
    E0503, // unreachable pattern
    E0504, // or-pattern binding mismatch

    // ── E0xxx: Legacy / general (kept for backward compat) ──────────
    E0001, // type mismatch (alias for E0301)
    E0002, // cannot apply operator to type (alias for E0302)
    E0003, // infinite type (alias for E0307)
    E0004, // undefined variable
    E0005, // undefined struct
    E0006, // unknown variant
    E0007, // wrong number of arguments (alias for E0201)
    E0008, // cannot call non-function (alias for E0203)
    E0009, // pipe target not a function (alias for E0204)
    E0010, // non-exhaustive match (alias for E0305)
    E0011, // pattern type mismatch (alias for E0306)
    E0012, // missing error types in throws (alias for E0206)
    E0013, // impl missing method
    E0014, // extra method in impl
    E0015, // no such field
    E0016, // type has no fields
    E0017, // match guard must be Bool (alias for E0304)

    // ── W01xx: Unused ───────────────────────────────────────────────
    W0101, // unused variable
    W0102, // unused function
    W0103, // unused import
    W0104, // unused field

    // ── W02xx: Deprecated ───────────────────────────────────────────
    W0201, // deprecated function
    W0202, // deprecated type

    // ── W03xx: Shadowing ────────────────────────────────────────────
    W0301, // variable shadows outer binding
    W0302, // import shadows local name

    // ── W04xx: Annotations ──────────────────────────────────────────
    W0401, // redundant type annotation
    W0402, // unnecessary wildcard pattern

    // ── Refinement violations (R0xxx) ───────────────────────────────
    R0001, // refinement predicate violated

    // ── C01xx: Undeclared capabilities ──────────────────────────────
    C0101, // missing required capability
    C0102, // unknown capability name
    C0103, // capability not in scope

    // ── C02xx: Platform capabilities ────────────────────────────────
    C0201, // platform-specific capability unavailable
    C0202, // capability requires higher platform version

    // ── C03xx: Purity ───────────────────────────────────────────────
    C0301, // impure call in pure context
    C0302, // capability leak across module boundary

    // ── Legacy capability codes ─────────────────────────────────────
    C0001, // missing capabilities (alias for C0101)
    C0002, // unknown capability (alias for C0102)

    // ── K01xx: Budget ───────────────────────────────────────────────
    K0101, // cost budget exceeded
    K0102, // cost annotation mismatch

    // ── K02xx: Unbounded ────────────────────────────────────────────
    K0201, // unbounded recursion detected
    K0202, // loop without bounded iteration

    // ── K03xx: Declaration ──────────────────────────────────────────
    K0301, // missing cost annotation on recursive function
    K0302, // invalid cost expression syntax

    // ── Legacy cost codes ───────────────────────────────────────────
    K0001, // cost budget exceeded (alias for K0101)

    // ── H01xx: Hole reports ─────────────────────────────────────────
    H0101, // typed hole found
    H0102, // hole with inferred type
    H0103, // hole in return position

    // ── H02xx: Candidates ───────────────────────────────────────────
    H0201, // hole candidates available
    H0202, // no candidates for hole
    H0203, // ambiguous candidates for hole

    // ── H03xx: Dependency ───────────────────────────────────────────
    H0301, // hole depends on another hole
    H0302, // circular hole dependency

    // ── M01xx: Circular ─────────────────────────────────────────────
    M0101, // circular module dependency
    M0102, // circular type dependency

    // ── M02xx: Visibility ───────────────────────────────────────────
    M0201, // private symbol not accessible
    M0202, // internal symbol used outside package
    M0203, // visibility mismatch in re-export

    // ── M03xx: Import ───────────────────────────────────────────────
    M0301, // module not found
    M0302, // symbol not found in module
    M0303, // ambiguous import (multiple modules export same name)

    // ── M04xx: Snapshot / versioning ────────────────────────────────
    M0401, // snapshot version mismatch
    M0402, // missing snapshot for dependency

    // ── Legacy module codes ─────────────────────────────────────────
    M0001, // module not found (alias for M0301)
    M0002, // symbol not found in module (alias for M0302)
    M0003, // private symbol not accessible (alias for M0201)
}

impl ErrorCode {
    /// Return the diagnostic severity for this code.
    pub fn severity(&self) -> Severity {
        use ErrorCode::*;
        match self {
            // Warnings
            W0101 | W0102 | W0103 | W0104 | W0201 | W0202 | W0301 | W0302 | W0401 | W0402 => {
                Severity::Warning
            }
            // Cost budget diagnostics are warnings (SEP-0004)
            K0101 | K0102 | K0001 => Severity::Warning,
            // Hole diagnostics are informational
            H0101 | H0102 | H0103 | H0201 | H0202 | H0203 | H0301 | H0302 => Severity::Info,
            // Everything else is an error
            _ => Severity::Error,
        }
    }

    /// Return a brief human-readable explanation of this error code.
    pub fn explain(&self) -> &'static str {
        use ErrorCode::*;
        match self {
            // E01xx — Struct errors
            E0101 => "Missing required struct field",
            E0102 => "Extra field in struct literal",
            E0103 => "Duplicate field in struct literal",
            E0104 => "Struct field type mismatch",
            E0105 => "Recursive struct without indirection",

            // E02xx — Argument / call errors
            E0201 => "Wrong number of arguments in function call",
            E0202 => "Argument type mismatch",
            E0203 => "Cannot call a non-function type",
            E0204 => "Pipe target is not a function",
            E0205 => "Pipe target expects wrong number of arguments",
            E0206 => "Missing error types in throws declaration",

            // E03xx — Type context / mismatch
            E0301 => "Type mismatch between expected and actual types",
            E0302 => "Cannot apply operator to this type",
            E0303 => "If condition must be Bool",
            E0304 => "Match guard must be Bool",
            E0305 => "Non-exhaustive match expression",
            E0306 => "Pattern type does not match scrutinee type",
            E0307 => "Infinite type detected (occurs check failure)",
            E0308 => "Expected one type, found a different type",
            E0309 => "Await requires a Task[T] type",

            // E04xx — Generics / type parameters
            E0401 => "Wrong number of type arguments",
            E0402 => "Type parameter is out of scope",
            E0403 => "Type constraint not satisfied",
            E0404 => "Cannot infer type parameter",

            // E05xx — Patterns
            E0501 => "Unknown variant in pattern",
            E0502 => "Wrong number of fields in variant pattern",
            E0503 => "Unreachable pattern",
            E0504 => "Or-pattern branches bind different variables",

            // Legacy E0xxx
            E0001 => "Type mismatch",
            E0002 => "Cannot apply operator to type",
            E0003 => "Infinite type (occurs check)",
            E0004 => "Undefined variable",
            E0005 => "Undefined struct",
            E0006 => "Unknown variant",
            E0007 => "Wrong number of arguments",
            E0008 => "Cannot call non-function",
            E0009 => "Pipe target not a function",
            E0010 => "Non-exhaustive match",
            E0011 => "Pattern type mismatch",
            E0012 => "Missing error types in throws",
            E0013 => "Impl missing required method",
            E0014 => "Extra method not in capability",
            E0015 => "No such field on struct",
            E0016 => "Type has no fields",
            E0017 => "Match guard must be Bool",

            // W01xx — Unused
            W0101 => "Unused variable",
            W0102 => "Unused function",
            W0103 => "Unused import",
            W0104 => "Unused field",

            // W02xx — Deprecated
            W0201 => "Use of deprecated function",
            W0202 => "Use of deprecated type",

            // W03xx — Shadowing
            W0301 => "Variable shadows an outer binding",
            W0302 => "Import shadows a local name",

            // W04xx — Annotations
            W0401 => "Redundant type annotation",
            W0402 => "Unnecessary wildcard pattern",

            // R0xxx — Refinement
            R0001 => "Refinement predicate violated",

            // C01xx — Undeclared capabilities
            C0101 => "Missing required capability",
            C0102 => "Unknown capability name",
            C0103 => "Capability not in scope",

            // C02xx — Platform capabilities
            C0201 => "Platform-specific capability unavailable on target",
            C0202 => "Capability requires a higher platform version",

            // C03xx — Purity
            C0301 => "Impure call in a pure context",
            C0302 => "Capability leaks across module boundary",

            // Legacy capability codes
            C0001 => "Missing capabilities",
            C0002 => "Unknown capability",

            // K01xx — Budget
            K0101 => "Cost budget exceeded",
            K0102 => "Cost annotation does not match inferred cost",

            // K02xx — Unbounded
            K0201 => "Unbounded recursion detected",
            K0202 => "Loop without bounded iteration",

            // K03xx — Declaration
            K0301 => "Missing cost annotation on recursive function",
            K0302 => "Invalid cost expression syntax",

            // Legacy cost code
            K0001 => "Cost budget exceeded",

            // H01xx — Hole reports
            H0101 => "Typed hole found",
            H0102 => "Hole with inferred type",
            H0103 => "Hole in return position",

            // H02xx — Candidates
            H0201 => "Hole candidates available",
            H0202 => "No candidates found for hole",
            H0203 => "Ambiguous candidates for hole",

            // H03xx — Dependency
            H0301 => "Hole depends on another hole",
            H0302 => "Circular hole dependency detected",

            // M01xx — Circular
            M0101 => "Circular module dependency",
            M0102 => "Circular type dependency",

            // M02xx — Visibility
            M0201 => "Private symbol not accessible",
            M0202 => "Internal symbol used outside its package",
            M0203 => "Visibility mismatch in re-export",

            // M03xx — Import
            M0301 => "Module not found",
            M0302 => "Symbol not found in module",
            M0303 => "Ambiguous import from multiple modules",

            // M04xx — Snapshot / versioning
            M0401 => "Snapshot version mismatch",
            M0402 => "Missing snapshot for dependency",

            // Legacy module codes
            M0001 => "Module not found",
            M0002 => "Symbol not found in module",
            M0003 => "Private symbol not accessible",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ErrorCode::*;
        let code = match self {
            // E01xx
            E0101 => "E0101",
            E0102 => "E0102",
            E0103 => "E0103",
            E0104 => "E0104",
            E0105 => "E0105",
            // E02xx
            E0201 => "E0201",
            E0202 => "E0202",
            E0203 => "E0203",
            E0204 => "E0204",
            E0205 => "E0205",
            E0206 => "E0206",
            // E03xx
            E0301 => "E0301",
            E0302 => "E0302",
            E0303 => "E0303",
            E0304 => "E0304",
            E0305 => "E0305",
            E0306 => "E0306",
            E0307 => "E0307",
            E0308 => "E0308",
            E0309 => "E0309",
            // E04xx
            E0401 => "E0401",
            E0402 => "E0402",
            E0403 => "E0403",
            E0404 => "E0404",
            // E05xx
            E0501 => "E0501",
            E0502 => "E0502",
            E0503 => "E0503",
            E0504 => "E0504",
            // Legacy E0xxx
            E0001 => "E0001",
            E0002 => "E0002",
            E0003 => "E0003",
            E0004 => "E0004",
            E0005 => "E0005",
            E0006 => "E0006",
            E0007 => "E0007",
            E0008 => "E0008",
            E0009 => "E0009",
            E0010 => "E0010",
            E0011 => "E0011",
            E0012 => "E0012",
            E0013 => "E0013",
            E0014 => "E0014",
            E0015 => "E0015",
            E0016 => "E0016",
            E0017 => "E0017",
            // W01xx
            W0101 => "W0101",
            W0102 => "W0102",
            W0103 => "W0103",
            W0104 => "W0104",
            // W02xx
            W0201 => "W0201",
            W0202 => "W0202",
            // W03xx
            W0301 => "W0301",
            W0302 => "W0302",
            // W04xx
            W0401 => "W0401",
            W0402 => "W0402",
            // R0xxx
            R0001 => "R0001",
            // C01xx
            C0101 => "C0101",
            C0102 => "C0102",
            C0103 => "C0103",
            // C02xx
            C0201 => "C0201",
            C0202 => "C0202",
            // C03xx
            C0301 => "C0301",
            C0302 => "C0302",
            // Legacy C0xxx
            C0001 => "C0001",
            C0002 => "C0002",
            // K01xx
            K0101 => "K0101",
            K0102 => "K0102",
            // K02xx
            K0201 => "K0201",
            K0202 => "K0202",
            // K03xx
            K0301 => "K0301",
            K0302 => "K0302",
            // Legacy K0xxx
            K0001 => "K0001",
            // H01xx
            H0101 => "H0101",
            H0102 => "H0102",
            H0103 => "H0103",
            // H02xx
            H0201 => "H0201",
            H0202 => "H0202",
            H0203 => "H0203",
            // H03xx
            H0301 => "H0301",
            H0302 => "H0302",
            // M01xx
            M0101 => "M0101",
            M0102 => "M0102",
            // M02xx
            M0201 => "M0201",
            M0202 => "M0202",
            M0203 => "M0203",
            // M03xx
            M0301 => "M0301",
            M0302 => "M0302",
            M0303 => "M0303",
            // M04xx
            M0401 => "M0401",
            M0402 => "M0402",
            // Legacy M0xxx
            M0001 => "M0001",
            M0002 => "M0002",
            M0003 => "M0003",
        };
        write!(f, "{code}")
    }
}

/// Return a slice of all `ErrorCode` variants (for exhaustive testing).
pub fn all_error_codes() -> &'static [ErrorCode] {
    use ErrorCode::*;
    &[
        E0101, E0102, E0103, E0104, E0105, E0201, E0202, E0203, E0204, E0205, E0206, E0301, E0302,
        E0303, E0304, E0305, E0306, E0307, E0308, E0309, E0401, E0402, E0403, E0404, E0501, E0502,
        E0503, E0504, E0001, E0002, E0003, E0004, E0005, E0006, E0007, E0008, E0009, E0010, E0011,
        E0012, E0013, E0014, E0015, E0016, E0017, W0101, W0102, W0103, W0104, W0201, W0202, W0301,
        W0302, W0401, W0402, R0001, C0101, C0102, C0103, C0201, C0202, C0301, C0302, C0001, C0002,
        K0101, K0102, K0201, K0202, K0301, K0302, K0001, H0101, H0102, H0103, H0201, H0202, H0203,
        H0301, H0302, M0101, M0102, M0201, M0202, M0203, M0301, M0302, M0303, M0401, M0402, M0001,
        M0002, M0003,
    ]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_codes_have_unique_display_strings() {
        let codes = all_error_codes();
        let mut seen = HashSet::new();
        for code in codes {
            let s = code.to_string();
            assert!(
                seen.insert(s.clone()),
                "duplicate display string: {s} (variant {code:?})"
            );
        }
    }

    #[test]
    fn severity_correct_for_each_category() {
        // K0101, K0102, K0001 are warnings per SEP-0004
        let cost_warning_codes: std::collections::HashSet<&str> =
            ["K0101", "K0102", "K0001"].into_iter().collect();
        for code in all_error_codes() {
            let s = code.to_string();
            let expected = if s.starts_with('W') || cost_warning_codes.contains(s.as_str()) {
                Severity::Warning
            } else if s.starts_with('H') {
                Severity::Info
            } else {
                Severity::Error
            };
            assert_eq!(code.severity(), expected, "severity mismatch for {code:?}");
        }
    }

    #[test]
    fn explain_returns_non_empty_for_all_codes() {
        for code in all_error_codes() {
            let explanation = code.explain();
            assert!(!explanation.is_empty(), "empty explanation for {code:?}");
        }
    }

    #[test]
    fn display_roundtrip_format() {
        // Verify the display format matches the expected pattern (letter + 4 digits).
        let re_pattern = |s: &str| -> bool {
            s.len() == 5
                && s.as_bytes()[0].is_ascii_uppercase()
                && s[1..].chars().all(|c| c.is_ascii_digit())
        };
        for code in all_error_codes() {
            let s = code.to_string();
            assert!(
                re_pattern(&s),
                "display string `{s}` does not match X0000 format for {code:?}"
            );
        }
    }

    #[test]
    fn total_code_count_is_at_least_75() {
        // 75 SEP-specified + legacy aliases
        assert!(
            all_error_codes().len() >= 75,
            "expected at least 75 codes, got {}",
            all_error_codes().len()
        );
    }
}
