use spore_parser::parse;
use spore_typeck::hir::{HirExpr, HirItem, UNRESOLVED};
use spore_typeck::lower;

fn lower_src(src: &str) -> spore_typeck::hir::HirModule {
    let ast = parse(src).unwrap_or_else(|e| panic!("parse error: {e:?}"));
    lower(&ast)
}

#[test]
fn pipe_desugared_to_call() {
    let hir = lower_src(
        r#"
        fn double(x: Int) -> Int { x + x }
        fn main() -> Int { 5 |> double }
    "#,
    );

    // main is the second item
    let main_fn = match &hir.items[1] {
        HirItem::Function(f) => f,
        other => panic!("expected Function, got {other:?}"),
    };
    assert_eq!(main_fn.name, "main");

    match &main_fn.body {
        Some(HirExpr::Call(_, args)) => {
            assert_eq!(args.len(), 1, "pipe desugar should pass one argument");
            match &args[0] {
                HirExpr::IntLit(5) => {}
                other => panic!("expected IntLit(5) as arg, got {other:?}"),
            }
        }
        // Parser may wrap the body in a Block
        Some(HirExpr::Block(_, Some(call))) => match call.as_ref() {
            HirExpr::Call(_, args) => {
                assert_eq!(args.len(), 1, "pipe desugar should pass one argument");
                match &args[0] {
                    HirExpr::IntLit(5) => {}
                    other => panic!("expected IntLit(5) as arg, got {other:?}"),
                }
            }
            other => panic!("expected Call inside Block, got {other:?}"),
        },
        other => panic!("expected pipe desugared to Call, got {other:?}"),
    }
}

#[test]
fn names_resolved() {
    let hir = lower_src(
        r#"
        fn foo() -> Int { 42 }
        fn bar() -> Int { foo() }
    "#,
    );

    assert_eq!(hir.items.len(), 2);

    let foo = match &hir.items[0] {
        HirItem::Function(f) => f,
        other => panic!("expected Function, got {other:?}"),
    };
    assert_eq!(foo.name, "foo");
    assert!(foo.def_id < UNRESOLVED, "foo should have a resolved def_id");

    let bar = match &hir.items[1] {
        HirItem::Function(f) => f,
        other => panic!("expected Function, got {other:?}"),
    };
    assert_eq!(bar.name, "bar");
    assert!(bar.def_id < UNRESOLVED);
    assert_ne!(
        foo.def_id, bar.def_id,
        "foo and bar should have different def_ids"
    );

    // Inside bar's body, `foo` reference should resolve to foo's def_id.
    if let Some(HirExpr::Call(callee, _)) = &bar.body {
        if let HirExpr::Var(name, id) = callee.as_ref() {
            assert_eq!(name, "foo");
            assert_eq!(*id, foo.def_id);
        } else {
            panic!("expected Var callee in bar body");
        }
    }
}

#[test]
fn struct_lowering() {
    let hir = lower_src(
        r#"
        struct Point { x: Int, y: Int }
    "#,
    );

    let s = match &hir.items[0] {
        HirItem::StructDef(s) => s,
        other => panic!("expected StructDef, got {other:?}"),
    };
    assert_eq!(s.name, "Point");
    assert_eq!(s.fields.len(), 2);
    assert_eq!(s.fields[0].0, "x");
    assert_eq!(s.fields[1].0, "y");
}

#[test]
fn block_with_let_lowering() {
    let hir = lower_src(
        r#"
        fn example() -> Int {
            let x: Int = 10
            x
        }
    "#,
    );

    let f = match &hir.items[0] {
        HirItem::Function(f) => f,
        other => panic!("expected Function, got {other:?}"),
    };
    assert_eq!(f.name, "example");

    // Body should be a Block with a Let stmt and a tail Var expression.
    match &f.body {
        Some(HirExpr::Block(stmts, Some(tail))) => {
            assert_eq!(stmts.len(), 1);
            match &stmts[0] {
                hir::HirStmt::Let(name, _, _) => assert_eq!(name, "x"),
                other => panic!("expected Let, got {other:?}"),
            }
            match tail.as_ref() {
                HirExpr::Var(name, _) => assert_eq!(name, "x"),
                other => panic!("expected Var tail, got {other:?}"),
            }
        }
        other => panic!("expected Block, got {other:?}"),
    }
}

// Bring hir module into scope for pattern matching.
use spore_typeck::hir;

#[test]
fn imports_are_skipped() {
    let hir = lower_src(
        r#"
        import std::io as io
        fn main() -> Int { 0 }
    "#,
    );
    // Import should be filtered out; only the function remains.
    assert_eq!(hir.items.len(), 1);
    assert!(matches!(&hir.items[0], HirItem::Function(_)));
}

#[test]
fn type_def_lowering() {
    let hir = lower_src(
        r#"
        type Option[T] { Some(T), None }
    "#,
    );

    let td = match &hir.items[0] {
        HirItem::TypeDef(t) => t,
        other => panic!("expected TypeDef, got {other:?}"),
    };
    assert_eq!(td.name, "Option");
    assert_eq!(td.type_params, vec!["T".to_string()]);
    assert_eq!(td.variants.len(), 2);
    assert_eq!(td.variants[0].name, "Some");
    assert_eq!(td.variants[1].name, "None");
}
