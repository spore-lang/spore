use insta::assert_debug_snapshot;
use sporec_parser::parse;
use sporec_typeck::{lower, type_check};

fn parse_ok(src: &str) -> sporec_parser::ast::Module {
    parse(src).unwrap_or_else(|errs| {
        panic!(
            "parse failed:\n{}",
            errs.iter()
                .map(|err| err.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )
    })
}

fn lower_ok(src: &str) -> sporec_typeck::hir::HirModule {
    let module = parse_ok(src);
    lower(&module)
}

fn diagnostic_summary(src: &str) -> Vec<(String, String, String)> {
    let module = parse_ok(src);
    type_check(&module)
        .expect_err("source should produce diagnostics")
        .into_iter()
        .map(|err| {
            (
                err.code.to_string(),
                err.code.severity().to_string(),
                err.message,
            )
        })
        .collect()
}

fn warning_summary(src: &str) -> Vec<(String, String, String)> {
    let module = parse_ok(src);
    type_check(&module)
        .expect("source should type-check with warnings")
        .warnings
        .into_iter()
        .map(|warning| {
            (
                warning.code.to_string(),
                warning.code.severity().to_string(),
                warning.message,
            )
        })
        .collect()
}

#[test]
fn hir_pipe_and_name_resolution_snapshot() {
    let hir = lower_ok(
        r#"
        fn double(x: I32) -> I32 { x + x }
        fn main() -> I32 { 5 |> double }
        "#,
    );

    assert_debug_snapshot!(hir);
}

#[test]
fn hir_surface_items_snapshot() {
    let hir = lower_ok(
        r#"
        import std.io as io

        struct Point[T] { x: T, y: T }

        type Option[T] { Some(T), None }

        trait Display[T] {
            type Output
            fn show(self: T) -> Str
        }

        impl Display for Point {
            fn show(self: Point) -> Str { "point" }
        }
        "#,
    );

    assert_debug_snapshot!(hir);
}

#[test]
fn diagnostics_missing_effect_snapshot() {
    let diagnostics = diagnostic_summary(
        r#"
        fn needs_console() -> () {
            println("hello");
            return
        }
        "#,
    );

    assert_debug_snapshot!(diagnostics);
}

#[test]
fn diagnostics_cost_warning_snapshot() {
    let warnings = warning_summary(
        r#"
        fn expensive(x: I64) -> I64 cost [100, 0, 0, 0] { x + x }

        fn over_budget(a: I64) -> I64 cost [2, 0, 0, 0] {
            expensive(expensive(a))
        }
        "#,
    );

    assert_debug_snapshot!(warnings);
}

#[test]
fn diagnostics_trait_impl_contract_snapshot() {
    let diagnostics = diagnostic_summary(
        r#"
        trait Display[T] {
            fn show(self: T) -> Str
        }

        struct Point { x: I64, y: I64 }

        impl Display for Point {
        }
        "#,
    );

    assert_debug_snapshot!(diagnostics);
}
