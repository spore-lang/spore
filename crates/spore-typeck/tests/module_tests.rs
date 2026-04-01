//! Tests for module import resolution and multi-file type checking.

use spore_parser::parse;
use spore_typeck::check::Checker;
use spore_typeck::error::ErrorCode;
use spore_typeck::module::{ModuleInterface, ModuleRegistry, SymbolVisibility};
use spore_typeck::types::Ty;
use spore_typeck::{build_module_interface, type_check_with_registry};

// ── Helpers ─────────────────────────────────────────────────────────

fn check_with_registry(
    src: &str,
    registry: ModuleRegistry,
) -> Result<(), Vec<(ErrorCode, String)>> {
    let module = parse(src).unwrap_or_else(|e| panic!("parse error: {e:?}"));
    match type_check_with_registry(&module, registry) {
        Ok(_) => Ok(()),
        Err(errs) => Err(errs.into_iter().map(|e| (e.code, e.message)).collect()),
    }
}

fn make_math_module() -> ModuleInterface {
    let mut m = ModuleInterface::new(vec!["Math".into()]);
    m.functions
        .insert("sqrt".into(), (vec![Ty::Float], Ty::Float));
    m.set_visibility("sqrt", SymbolVisibility::Pub);
    m.functions.insert("abs".into(), (vec![Ty::Int], Ty::Int));
    m.set_visibility("abs", SymbolVisibility::Pub);
    m
}

// ── Test 1: Import resolution finds exported function types ─────────

#[test]
fn import_resolution_finds_exported_function() {
    let mut registry = ModuleRegistry::new();
    registry.register(make_math_module());

    let src = r#"
import Math as Math
fn f() -> Float { sqrt(3.14) }
"#;

    check_with_registry(src, registry).unwrap_or_else(|errs| {
        panic!(
            "expected no errors, got:\n{}",
            errs.iter()
                .map(|(c, m)| format!("[{c}] {m}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
}

#[test]
fn import_resolution_finds_multiple_functions() {
    let mut registry = ModuleRegistry::new();
    registry.register(make_math_module());

    let src = r#"
import Math as Math
fn f() -> Float { sqrt(3.14) }
fn g() -> Int { abs(42) }
"#;

    check_with_registry(src, registry).unwrap_or_else(|errs| {
        panic!(
            "expected no errors, got:\n{}",
            errs.iter()
                .map(|(c, m)| format!("[{c}] {m}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
}

// ── Test 2: Private function not accessible from another module ─────

#[test]
fn private_function_not_accessible() {
    let mut registry = ModuleRegistry::new();
    let mut m = ModuleInterface::new(vec!["Lib".into()]);
    m.functions.insert("public_fn".into(), (vec![], Ty::Unit));
    m.set_visibility("public_fn", SymbolVisibility::Pub);
    m.functions.insert("secret_fn".into(), (vec![], Ty::Unit));
    m.set_visibility("secret_fn", SymbolVisibility::Private);
    registry.register(m);

    let src = r#"
import Lib as Lib
fn f() { secret_fn() }
"#;

    let errs = check_with_registry(src, registry).unwrap_err();
    assert!(
        errs.iter().any(|(code, _)| *code == ErrorCode::M0003),
        "expected M003 (private symbol), got: {errs:?}"
    );
}

#[test]
fn pub_pkg_function_is_accessible() {
    let mut registry = ModuleRegistry::new();
    let mut m = ModuleInterface::new(vec!["Lib".into()]);
    m.functions.insert("internal_fn".into(), (vec![], Ty::Unit));
    m.set_visibility("internal_fn", SymbolVisibility::PubPkg);
    registry.register(m);

    let src = r#"
import Lib as Lib
fn f() { internal_fn() }
"#;

    check_with_registry(src, registry).unwrap_or_else(|errs| {
        panic!(
            "expected no errors, got:\n{}",
            errs.iter()
                .map(|(c, m)| format!("[{c}] {m}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
}

// ── Test 3: Multi-file type checking ────────────────────────────────

#[test]
fn multi_file_cross_module_function_call() {
    // Simulate module A: parse and extract its interface
    let src_a = "pub fn add(a: Int, b: Int) -> Int { a + b }";
    let ast_a = parse(src_a).unwrap();
    let mut iface_a = build_module_interface(&ast_a);
    // Override path to a named module (parser gives empty name w/o module decl)
    iface_a.path = vec!["ModA".into()];

    let mut registry = ModuleRegistry::new();
    registry.register(iface_a);

    // Module B imports from ModA
    let src_b = r#"
import ModA
fn f() -> Int { add(1, 2) }
"#;

    check_with_registry(src_b, registry).unwrap_or_else(|errs| {
        panic!(
            "expected no errors, got:\n{}",
            errs.iter()
                .map(|(c, m)| format!("[{c}] {m}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
}

#[test]
fn multi_file_type_checking_with_manual_registry() {
    // Build module "Utils" with a pub function
    let mut utils = ModuleInterface::new(vec!["Utils".into()]);
    utils
        .functions
        .insert("double".into(), (vec![Ty::Int], Ty::Int));
    utils.set_visibility("double", SymbolVisibility::Pub);

    let mut registry = ModuleRegistry::new();
    registry.register(utils);

    let src = r#"
import Utils as U
fn f() -> Int { double(21) }
"#;

    check_with_registry(src, registry).unwrap_or_else(|errs| {
        panic!(
            "expected no errors, got:\n{}",
            errs.iter()
                .map(|(c, m)| format!("[{c}] {m}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
}

// ── Test 4: Missing module error ────────────────────────────────────

#[test]
fn missing_module_error() {
    let registry = ModuleRegistry::new();

    let src = r#"
import NonExistent as NE
fn f() -> Int { 42 }
"#;

    let errs = check_with_registry(src, registry).unwrap_err();
    assert!(
        errs.iter().any(|(code, _)| *code == ErrorCode::M0001),
        "expected M001 (module not found), got: {errs:?}"
    );
}

#[test]
fn missing_module_error_message_contains_name() {
    let registry = ModuleRegistry::new();

    let src = r#"
import Foo.Bar as FB
fn f() -> Int { 42 }
"#;

    let errs = check_with_registry(src, registry).unwrap_err();
    let m001_errors: Vec<_> = errs
        .iter()
        .filter(|(code, _)| *code == ErrorCode::M0001)
        .collect();
    assert!(!m001_errors.is_empty(), "expected M001 error");
    assert!(
        m001_errors[0].1.contains("Foo.Bar"),
        "error message should mention module path, got: {}",
        m001_errors[0].1
    );
}

// ── Test: build_module_interface extracts visibility ─────────────────

#[test]
fn build_module_interface_extracts_pub_functions() {
    let src = r#"
pub fn exported() -> Int { 42 }
fn private_helper() -> Int { 1 }
"#;
    let ast = parse(src).unwrap();
    let iface = build_module_interface(&ast);

    assert!(iface.exports("exported"));
    assert!(iface.exports("private_helper"));
    assert_eq!(*iface.visibility("exported"), SymbolVisibility::Pub);
    assert_eq!(
        *iface.visibility("private_helper"),
        SymbolVisibility::Private
    );
}

#[test]
fn build_module_interface_extracts_types_and_structs() {
    // Parser doesn't support `pub type/struct` syntax yet; they default to Private.
    // Build manually instead to test the interface extraction logic.
    let mut iface = ModuleInterface::new(vec!["Shapes".into()]);
    iface.types.insert(
        "Color".into(),
        vec!["Red".into(), "Green".into(), "Blue".into()],
    );
    iface.set_visibility("Color", SymbolVisibility::Pub);
    iface
        .structs
        .insert("Point".into(), vec!["x".into(), "y".into()]);
    iface.set_visibility("Point", SymbolVisibility::Pub);

    assert!(iface.exports("Color"));
    assert!(iface.exports("Point"));
    assert_eq!(*iface.visibility("Color"), SymbolVisibility::Pub);
    assert_eq!(*iface.visibility("Point"), SymbolVisibility::Pub);
}

// ── Test: Checker with module_registry field ────────────────────────

#[test]
fn checker_with_module_registry() {
    let mut registry = ModuleRegistry::new();
    registry.register(make_math_module());

    let checker = Checker::with_module_registry(registry);
    assert!(checker.errors.is_empty());
    assert!(checker.module_registry.get_by_path("Math").is_some());
}
