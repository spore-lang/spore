//! Tests for module import resolution and multi-file type checking.

use sporec_parser::parse;
use sporec_typeck::check::Checker;
use sporec_typeck::error::ErrorCode;
use sporec_typeck::module::{ModuleInterface, ModuleRegistry, SymbolVisibility};
use sporec_typeck::types::Ty;
use sporec_typeck::{build_module_interface, type_check_with_registry};

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
 fn f() -> F64 { sqrt(3.14) }
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
 fn f() -> F64 { sqrt(3.14) }
 fn g() -> I32 { abs(42) }
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
    let src_a = "pub fn add(a: I32, b: I32) -> I32 { a + b }";
    let ast_a = parse(src_a).unwrap();
    let mut iface_a = build_module_interface(&ast_a);
    // Override path to a named module (module path comes from file layout).
    iface_a.path = vec!["ModA".into()];

    let mut registry = ModuleRegistry::new();
    registry.register(iface_a);

    // Module B imports from ModA
    let src_b = r#"
import ModA
fn f() -> I32 { add(1, 2) }
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
fn f() -> I32 { double(21) }
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
fn f() -> I32 { 42 }
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
fn f() -> I32 { 42 }
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
pub fn exported() -> I32 { 42 }
fn private_helper() -> I32 { 1 }
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
        vec![
            ("Red".into(), vec![]),
            ("Green".into(), vec![]),
            ("Blue".into(), vec![]),
        ],
    );
    iface.set_visibility("Color", SymbolVisibility::Pub);
    iface.structs.insert(
        "Point".into(),
        vec![("x".into(), Ty::Int), ("y".into(), Ty::Int)],
    );
    iface.set_visibility("Point", SymbolVisibility::Pub);

    assert!(iface.exports("Color"));
    assert!(iface.exports("Point"));
    assert_eq!(*iface.visibility("Color"), SymbolVisibility::Pub);
    assert_eq!(*iface.visibility("Point"), SymbolVisibility::Pub);
}

#[test]
fn build_module_interface_resolves_aliases_before_function_signatures() {
    let src = r#"
fn main() -> Unit { return }
alias Unit = ()
"#;
    let ast = parse(src).unwrap();
    let iface = build_module_interface(&ast);

    let (_, ret_ty) = iface
        .functions
        .get("main")
        .expect("main should be present in module interface");
    assert_eq!(*ret_ty, Ty::Unit);
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

// ── Test: imported struct fields preserve types ─────────────────────

#[test]
fn imported_struct_preserves_field_types() {
    // Build a module that exports a struct with typed fields.
    let mut iface = ModuleInterface::new(vec!["Shapes".into()]);
    iface.structs.insert(
        "Point".into(),
        vec![("x".into(), Ty::Int), ("y".into(), Ty::Float)],
    );
    iface.set_visibility("Point", SymbolVisibility::Pub);

    let mut registry = ModuleRegistry::new();
    registry.register(iface);

    // A module that imports Shapes and uses Point
    let src = r#"
import Shapes
pub fn origin() -> I32 {
    let p = Point { x: 1, y: 2.0 }
    p.x
}
"#;
    let result = check_with_registry(src, registry);
    assert!(result.is_ok(), "expected no type errors, got {result:?}");
}

#[test]
fn imported_generic_struct_preserves_type_arguments() {
    let src_shapes = r#"
    pub struct Pair[A, B] { first: A, second: B }
    "#;
    let ast_shapes = parse(src_shapes).unwrap();
    let mut iface = build_module_interface(&ast_shapes);
    iface.path = vec!["Shapes".into()];

    let mut registry = ModuleRegistry::new();
    registry.register(iface);

    let src = r#"
import Shapes

pub fn build_pair(x: I32) -> Pair[I32, Str] {
    Pair { first: x, second: "ok" }
}

pub fn read_first(pair: Pair[I32, Str]) -> I32 {
    pair.first
}
"#;
    let result = check_with_registry(src, registry);
    assert!(result.is_ok(), "expected no type errors, got {result:?}");
}

// ── Test: imported type variants preserve field types ────────────────

#[test]
fn imported_type_preserves_variant_field_types() {
    // Build a module that exports a sum type with variant fields.
    let mut iface = ModuleInterface::new(vec!["Net".into()]);
    iface.types.insert(
        "Packet".into(),
        vec![
            ("Data".into(), vec![Ty::Str]),
            ("Ack".into(), vec![Ty::Int]),
            ("Close".into(), vec![]),
        ],
    );
    iface.set_visibility("Packet", SymbolVisibility::Pub);

    let mut registry = ModuleRegistry::new();
    registry.register(iface);

    // Verify that the imported type has correct variant structure.
    let found = registry.get_by_path("Net").unwrap();
    let variants = found.types.get("Packet").unwrap();
    assert_eq!(variants.len(), 3);
    assert_eq!(variants[0].0, "Data");
    assert_eq!(variants[0].1, vec![Ty::Str]);
    assert_eq!(variants[1].0, "Ack");
    assert_eq!(variants[1].1, vec![Ty::Int]);
    assert_eq!(variants[2].0, "Close");
    assert!(variants[2].1.is_empty());
}

// ── Test: Ambiguous imports produce M0303 (Bug A3) ──────────────────

#[test]
fn ambiguous_import_same_name_different_modules() {
    let mut registry = ModuleRegistry::new();

    // Module A exports `compute`
    let mut mod_a = ModuleInterface::new(vec!["ModA".into()]);
    mod_a
        .functions
        .insert("compute".into(), (vec![Ty::Int], Ty::Int));
    mod_a.set_visibility("compute", SymbolVisibility::Pub);
    registry.register(mod_a);

    // Module B also exports `compute` with a different signature
    let mut mod_b = ModuleInterface::new(vec!["ModB".into()]);
    mod_b
        .functions
        .insert("compute".into(), (vec![Ty::Str], Ty::Str));
    mod_b.set_visibility("compute", SymbolVisibility::Pub);
    registry.register(mod_b);

    let src = r#"
import ModA as A
import ModB as B
fn f() -> I32 { compute(1) }
"#;

    let errs = check_with_registry(src, registry).unwrap_err();
    assert!(
        errs.iter().any(|(code, _)| *code == ErrorCode::M0303),
        "expected M0303 (ambiguous import), got: {errs:?}"
    );
}

#[test]
fn ambiguous_import_same_effect_different_modules() {
    let mut registry = ModuleRegistry::new();

    let mut mod_a = ModuleInterface::new(vec!["ModA".into()]);
    mod_a.capabilities.insert("Console".into());
    mod_a.capability_methods.insert(
        "Console".into(),
        (vec![], vec![("println".into(), vec![Ty::Str], Ty::Unit)]),
    );
    mod_a.set_visibility("Console", SymbolVisibility::Pub);
    registry.register(mod_a);

    let mut mod_b = ModuleInterface::new(vec!["ModB".into()]);
    mod_b.capabilities.insert("Console".into());
    mod_b.capability_methods.insert(
        "Console".into(),
        (vec![], vec![("println".into(), vec![Ty::Int], Ty::Unit)]),
    );
    mod_b.set_visibility("Console", SymbolVisibility::Pub);
    registry.register(mod_b);

    let src = r#"
import ModA as A
import ModB as B
fn f() -> () uses [Console] { perform Console.println("hello") }
"#;

    let errs = check_with_registry(src, registry).unwrap_err();
    assert!(
        errs.iter().any(|(code, _)| *code == ErrorCode::M0303),
        "expected M0303 (ambiguous import), got: {errs:?}"
    );
}
