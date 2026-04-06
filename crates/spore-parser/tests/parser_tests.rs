use spore_parser::parse;

fn parse_ok(src: &str) -> spore_parser::ast::Module {
    parse(src).unwrap_or_else(|errs| {
        panic!(
            "parse failed:\n{}",
            errs.iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )
    })
}

// ── Empty module ─────────────────────────────────────────────────────────

#[test]
fn test_empty_module() {
    let m = parse_ok("");
    assert!(m.items.is_empty());
}

// ── Simple function ──────────────────────────────────────────────────────

#[test]
fn test_simple_fn() {
    let m = parse_ok("fn add(a: Int, b: Int) -> Int { a + b }");
    assert_eq!(m.items.len(), 1);
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            assert_eq!(f.name, "add");
            assert_eq!(f.params.len(), 2);
            assert_eq!(f.params[0].name, "a");
            assert_eq!(f.params[1].name, "b");
            assert!(f.return_type.is_some());
        }
        _ => panic!("expected function"),
    }
}

// ── Visibility ───────────────────────────────────────────────────────────

#[test]
fn test_pub_fn() {
    let m = parse_ok("pub fn greet() -> String { \"hello\" }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            assert!(matches!(f.visibility, spore_parser::ast::Visibility::Pub));
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_pub_pkg_fn() {
    let m = parse_ok("pub(pkg) fn internal() -> Int { 42 }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            assert!(matches!(
                f.visibility,
                spore_parser::ast::Visibility::PubPkg
            ));
        }
        _ => panic!("expected function"),
    }
}

// ── Function with clauses ────────────────────────────────────────────────

#[test]
fn test_fn_with_uses() {
    let m = parse_ok("fn fetch(url: String) -> String uses [NetRead] { \"data\" }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let uses = f.uses_clause.as_ref().unwrap();
            assert_eq!(uses.resources, vec!["NetRead"]);
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_with_cost() {
    let m = parse_ok("fn sort(xs: List) -> List cost ≤ n * n { xs }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            assert!(f.cost_clause.is_some());
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_with_where() {
    let m = parse_ok("fn show(x: T) -> String where T: Display { \"\" }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let wc = f.where_clause.as_ref().unwrap();
            assert_eq!(wc.constraints.len(), 1);
            assert_eq!(wc.constraints[0].type_var, "T");
            assert_eq!(wc.constraints[0].bound, "Display");
        }
        _ => panic!("expected function"),
    }
}

// ── Expressions ──────────────────────────────────────────────────────────

#[test]
fn test_arithmetic_precedence() {
    // 1 + 2 * 3 should parse as 1 + (2 * 3)
    let m = parse_ok("fn f() -> Int { 1 + 2 * 3 }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            // Body is Block([], Some(1 + 2*3))
            match body {
                spore_parser::ast::Expr::Block(stmts, Some(tail)) => {
                    assert!(stmts.is_empty());
                    match tail.as_ref() {
                        spore_parser::ast::Expr::BinOp(_, spore_parser::ast::BinOp::Add, rhs) => {
                            assert!(matches!(
                                rhs.as_ref(),
                                spore_parser::ast::Expr::BinOp(_, spore_parser::ast::BinOp::Mul, _)
                            ));
                        }
                        _ => panic!("expected Add at top"),
                    }
                }
                _ => panic!("expected block"),
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_if_expr() {
    let m = parse_ok("fn f(x: Int) -> Int { if x > 0 { x } else { 0 } }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                spore_parser::ast::Expr::Block(_, Some(tail)) => {
                    assert!(matches!(
                        tail.as_ref(),
                        spore_parser::ast::Expr::If(_, _, Some(_))
                    ));
                }
                _ => panic!("expected block with if tail"),
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_match_expr() {
    let src = r#"fn f(x: Int) -> String {
        match x {
            0 => "zero",
            1 => "one",
            _ => "other"
        }
    }"#;
    let m = parse_ok(src);
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                spore_parser::ast::Expr::Block(_, Some(tail)) => match tail.as_ref() {
                    spore_parser::ast::Expr::Match(_, arms) => {
                        assert_eq!(arms.len(), 3);
                    }
                    _ => panic!("expected match"),
                },
                _ => panic!("expected block"),
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_let_stmt() {
    let m = parse_ok("fn f() -> Int { let x = 42; x }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                spore_parser::ast::Expr::Block(stmts, Some(_tail)) => {
                    assert_eq!(stmts.len(), 1);
                    match &stmts[0] {
                        spore_parser::ast::Stmt::Let(name, _, _) => assert_eq!(name, "x"),
                        _ => panic!("expected let"),
                    }
                }
                _ => panic!("expected block"),
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_pipe_expr() {
    let m = parse_ok("fn f(x: Int) -> Int { x |> double }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                spore_parser::ast::Expr::Block(_, Some(tail)) => {
                    assert!(matches!(tail.as_ref(), spore_parser::ast::Expr::Pipe(_, _)));
                }
                _ => panic!("expected block"),
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_lambda() {
    let m = parse_ok("fn f() -> Int { |x| x + 1 }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                spore_parser::ast::Expr::Block(_, Some(tail)) => {
                    assert!(matches!(
                        tail.as_ref(),
                        spore_parser::ast::Expr::Lambda(_, _)
                    ));
                }
                _ => panic!("expected block"),
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_try_expr() {
    let m = parse_ok("fn f(x: Result) -> Int { x? }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                spore_parser::ast::Expr::Block(_, Some(tail)) => {
                    assert!(matches!(tail.as_ref(), spore_parser::ast::Expr::Try(_)));
                }
                _ => panic!("expected block"),
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_hole() {
    let m = parse_ok("fn f() -> Int { ?todo }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                spore_parser::ast::Expr::Block(_, Some(tail)) => match tail.as_ref() {
                    spore_parser::ast::Expr::Hole(name, _, _) => assert_eq!(name, "todo"),
                    _ => panic!("expected hole, got {:?}", tail),
                },
                _ => panic!("expected block"),
            }
        }
        _ => panic!("expected function"),
    }
}

// ── Items ────────────────────────────────────────────────────────────────

#[test]
fn test_struct_def() {
    let m = parse_ok("struct Point { x: Float, y: Float }");
    match &m.items[0] {
        spore_parser::ast::Item::StructDef(s) => {
            assert_eq!(s.name, "Point");
            assert_eq!(s.fields.len(), 2);
        }
        _ => panic!("expected struct"),
    }
}

#[test]
fn test_type_def() {
    let m = parse_ok("type Option[T] { Some(T), None }");
    match &m.items[0] {
        spore_parser::ast::Item::TypeDef(t) => {
            assert_eq!(t.name, "Option");
            assert_eq!(t.type_params, vec!["T"]);
            assert_eq!(t.variants.len(), 2);
            assert_eq!(t.variants[0].name, "Some");
            assert_eq!(t.variants[1].name, "None");
        }
        _ => panic!("expected type def"),
    }
}

#[test]
fn test_import() {
    let m = parse_ok("import std.io.File");
    match &m.items[0] {
        spore_parser::ast::Item::Import(spore_parser::ast::ImportDecl::Import { path, alias }) => {
            assert_eq!(path, "std.io.File");
            assert_eq!(alias, "File");
        }
        _ => panic!("expected import"),
    }
}

#[test]
fn test_import_with_alias() {
    let m = parse_ok("import std.collections.HashMap as Map");
    match &m.items[0] {
        spore_parser::ast::Item::Import(spore_parser::ast::ImportDecl::Import { path, alias }) => {
            assert_eq!(path, "std.collections.HashMap");
            assert_eq!(alias, "Map");
        }
        _ => panic!("expected import"),
    }
}

#[test]
fn test_capability_def() {
    let m = parse_ok("capability Display[T] { fn show(self: T) -> String }");
    match &m.items[0] {
        spore_parser::ast::Item::CapabilityDef(c) => {
            assert_eq!(c.name, "Display");
            assert_eq!(c.type_params, vec!["T"]);
            assert_eq!(c.methods.len(), 1);
            assert_eq!(c.methods[0].name, "show");
        }
        _ => panic!("expected capability"),
    }
}

// ── Generic types ────────────────────────────────────────────────────────

#[test]
fn test_generic_type() {
    let m = parse_ok("fn f(xs: List[Int]) -> List[String] { xs }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => match &f.params[0].ty {
            spore_parser::ast::TypeExpr::Generic(name, args) => {
                assert_eq!(name, "List");
                assert_eq!(args.len(), 1);
            }
            _ => panic!("expected generic type"),
        },
        _ => panic!("expected function"),
    }
}

// ── Multiple items ───────────────────────────────────────────────────────

#[test]
fn test_multiple_items() {
    let src = r#"
        struct Point { x: Float, y: Float }
        fn origin() -> Point { Point { x: 0.0, y: 0.0 } }
    "#;
    let m = parse_ok(src);
    assert_eq!(m.items.len(), 2);
    assert!(matches!(m.items[0], spore_parser::ast::Item::StructDef(_)));
    assert!(matches!(m.items[1], spore_parser::ast::Item::Function(_)));
}

// ── Call expressions ─────────────────────────────────────────────────────

#[test]
fn test_call_expr() {
    let m = parse_ok("fn f() -> Int { add(1, 2) }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                spore_parser::ast::Expr::Block(_, Some(tail)) => {
                    assert!(matches!(tail.as_ref(), spore_parser::ast::Expr::Call(_, _)));
                }
                _ => panic!("expected block"),
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_method_call() {
    let m = parse_ok("fn f(x: String) -> Int { x.len() }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                spore_parser::ast::Expr::Block(_, Some(tail)) => {
                    assert!(matches!(tail.as_ref(), spore_parser::ast::Expr::Call(_, _)));
                }
                _ => panic!("expected block"),
            }
        }
        _ => panic!("expected function"),
    }
}

// ── Struct literal ───────────────────────────────────────────────────────

#[test]
fn test_struct_literal() {
    let m = parse_ok("fn f() -> Point { Point { x: 1.0, y: 2.0 } }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                spore_parser::ast::Expr::Block(_, Some(tail)) => match tail.as_ref() {
                    spore_parser::ast::Expr::StructLit(name, fields) => {
                        assert_eq!(name, "Point");
                        assert_eq!(fields.len(), 2);
                    }
                    _ => panic!("expected struct lit"),
                },
                _ => panic!("expected block"),
            }
        }
        _ => panic!("expected function"),
    }
}

// ── Unary expressions ────────────────────────────────────────────────────

#[test]
fn test_unary_neg() {
    let m = parse_ok("fn f() -> Int { -42 }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                spore_parser::ast::Expr::Block(_, Some(tail)) => {
                    assert!(matches!(
                        tail.as_ref(),
                        spore_parser::ast::Expr::UnaryOp(spore_parser::ast::UnaryOp::Neg, _)
                    ));
                }
                _ => panic!("expected block"),
            }
        }
        _ => panic!("expected function"),
    }
}

// ── Generic type parameters on functions ─────────────────────────────────

#[test]
fn test_fn_type_params() {
    let m = parse_ok("fn identity[T](x: T) -> T { x }");
    assert_eq!(m.items.len(), 1);
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            assert_eq!(f.name, "identity");
            assert_eq!(f.type_params, vec!["T".to_string()]);
            assert_eq!(f.params.len(), 1);
            assert_eq!(f.params[0].name, "x");
            assert!(f.return_type.is_some());
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_multiple_type_params() {
    let m = parse_ok("fn pair[A, B](a: A, b: B) -> Tuple { a }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            assert_eq!(f.type_params, vec!["A".to_string(), "B".to_string()]);
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_no_type_params() {
    let m = parse_ok("fn add(a: Int, b: Int) -> Int { a + b }");
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            assert!(f.type_params.is_empty());
        }
        _ => panic!("expected function"),
    }
}

// ── Const declarations ──────────────────────────────────────────────────

#[test]
fn test_const_item() {
    let m = parse_ok("const MAX: Int = 100");
    assert_eq!(m.items.len(), 1);
    match &m.items[0] {
        spore_parser::ast::Item::Const(c) => {
            assert_eq!(c.name, "MAX");
            assert!(matches!(
                c.visibility,
                spore_parser::ast::Visibility::Private
            ));
            assert!(matches!(&c.ty, spore_parser::ast::TypeExpr::Named(n) if n == "Int"));
            assert!(matches!(&c.value, spore_parser::ast::Expr::IntLit(100)));
        }
        _ => panic!("expected const"),
    }
}

#[test]
fn test_pub_const_item() {
    let m = parse_ok("pub const NAME: String = \"hello\"");
    assert_eq!(m.items.len(), 1);
    match &m.items[0] {
        spore_parser::ast::Item::Const(c) => {
            assert_eq!(c.name, "NAME");
            assert!(matches!(c.visibility, spore_parser::ast::Visibility::Pub));
            assert!(matches!(&c.ty, spore_parser::ast::TypeExpr::Named(n) if n == "String"));
            assert!(matches!(&c.value, spore_parser::ast::Expr::StrLit(_)));
        }
        _ => panic!("expected const"),
    }
}

// ── Return / Throw / List / Char / String prefix tests ──────────────────────

use spore_parser::ast::{Expr, FStringPart, TStringPart};

fn get_fn_body(src: &str) -> Expr {
    let m = parse_ok(src);
    let f = match &m.items[0] {
        spore_parser::ast::Item::Function(f) => f,
        _ => panic!("expected function"),
    };
    f.body.clone().expect("expected body")
}

fn get_tail(src: &str) -> Expr {
    let body = get_fn_body(src);
    if let Expr::Block(_, Some(tail)) = body {
        *tail
    } else {
        panic!("expected block with tail, got {:?}", body);
    }
}

#[test]
fn test_return_expr() {
    let tail = get_tail("fn foo(x: Int) -> Int { return x }");
    assert!(matches!(tail, Expr::Return(Some(_))));
}

#[test]
fn test_return_no_value() {
    let tail = get_tail("fn foo() { return }");
    assert!(matches!(tail, Expr::Return(None)));
}

#[test]
fn test_throw_expr() {
    let tail = get_tail(r#"fn foo() { throw "error" }"#);
    assert!(matches!(tail, Expr::Throw(_)));
}

#[test]
fn test_list_literal() {
    let tail = get_tail("fn foo() { [1, 2, 3] }");
    if let Expr::List(elems) = tail {
        assert_eq!(elems.len(), 3);
    } else {
        panic!("expected list literal");
    }
}

#[test]
fn test_empty_list() {
    let tail = get_tail("fn foo() { [] }");
    if let Expr::List(elems) = tail {
        assert_eq!(elems.len(), 0);
    } else {
        panic!("expected empty list");
    }
}

#[test]
fn test_char_literal() {
    let tail = get_tail("fn foo() { 'a' }");
    assert!(matches!(tail, Expr::CharLit('a')));
}

#[test]
fn test_char_escape() {
    let tail = get_tail("fn foo() { '\\n' }");
    assert!(matches!(tail, Expr::CharLit('\n')));
}

#[test]
fn test_raw_string() {
    let tail = get_tail("fn foo() { r\"C:\\Users\\path\" }");
    if let Expr::StrLit(s) = tail {
        assert_eq!(s, "C:\\Users\\path");
    } else {
        panic!("expected raw string, got {:?}", tail);
    }
}

#[test]
fn test_fstring() {
    let tail = get_tail("fn foo(name: Str) { f\"hello {name}\" }");
    if let Expr::FString(parts) = tail {
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[0], FStringPart::Literal(s) if s == "hello "));
        assert!(matches!(&parts[1], FStringPart::Expr(Expr::Var(n)) if n == "name"));
    } else {
        panic!("expected fstring, got {:?}", tail);
    }
}

#[test]
fn test_tstring() {
    let tail = get_tail("fn foo(name: Str) { t\"dear {name}\" }");
    if let Expr::TString(parts) = tail {
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[0], TStringPart::Literal(s) if s == "dear "));
        assert!(matches!(&parts[1], TStringPart::Expr(Expr::Var(n)) if n == "name"));
    } else {
        panic!("expected tstring, got {:?}", tail);
    }
}

#[test]
fn test_raw_string_parse() {
    let tail = get_tail(r#"fn foo() { r"C:\Users\path" }"#);
    match tail {
        Expr::StrLit(s) => assert_eq!(s, r"C:\Users\path"),
        other => panic!("expected StrLit, got {:?}", other),
    }
}

#[test]
fn test_fstring_parse() {
    let tail = get_tail(r#"fn foo(x: Int) { f"val={x}" }"#);
    if let Expr::FString(parts) = tail {
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[0], FStringPart::Literal(s) if s == "val="));
        assert!(matches!(&parts[1], FStringPart::Expr(Expr::Var(n)) if n == "x"));
    } else {
        panic!("expected FString, got {:?}", tail);
    }
}

// ── Item 1: parallel_scope expression ───────────────────────────────────

#[test]
fn test_parallel_scope_basic() {
    let tail = get_tail("fn f() -> Int { parallel_scope { 1 + 2 } }");
    match tail {
        Expr::ParallelScope { lanes, body } => {
            assert!(lanes.is_none());
            assert!(matches!(*body, Expr::Block(_, _)));
        }
        other => panic!("expected ParallelScope, got {:?}", other),
    }
}

#[test]
fn test_parallel_scope_with_lanes() {
    let tail = get_tail("fn f() -> Int { parallel_scope(lanes: 4) { 1 + 2 } }");
    match tail {
        Expr::ParallelScope { lanes, body } => {
            assert!(matches!(*lanes.unwrap(), Expr::IntLit(4)));
            assert!(matches!(*body, Expr::Block(_, _)));
        }
        other => panic!("expected ParallelScope with lanes, got {:?}", other),
    }
}

// ── Item 2: select expression ───────────────────────────────────────────

#[test]
fn test_select_expr() {
    let src = r#"fn f(rx1: Chan, rx2: Chan) -> Int {
        select {
            val from rx1 => val,
            msg from rx2 => msg
        }
    }"#;
    let tail = get_tail(src);
    match tail {
        Expr::Select(arms) => {
            assert_eq!(arms.len(), 2);
            assert_eq!(arms[0].binding, "val");
            assert_eq!(arms[1].binding, "msg");
        }
        other => panic!("expected Select, got {:?}", other),
    }
}

// ── Item 3: module uses clause ──────────────────────────────────────────

#[test]
fn test_module_uses_clause() {
    let src = r#"module mymod uses [NetRead, FileSystem]
        fn foo() -> Int { 42 }
    "#;
    let m = parse_ok(src);
    assert_eq!(m.name, "mymod");
    let uses = m.uses_clause.as_ref().unwrap();
    assert_eq!(uses.resources, vec!["NetRead", "FileSystem"]);
    assert_eq!(m.items.len(), 1);
}

#[test]
fn test_module_without_uses() {
    let m = parse_ok("fn foo() -> Int { 42 }");
    assert!(m.uses_clause.is_none());
}

// ── Item 4: alias declaration ───────────────────────────────────────────

use spore_parser::ast::{AliasDef, Item, TypeExpr, Visibility};

#[test]
fn test_alias_def() {
    let m = parse_ok("alias MyInt = Int");
    assert_eq!(m.items.len(), 1);
    match &m.items[0] {
        Item::Alias(AliasDef {
            name,
            visibility,
            target,
        }) => {
            assert_eq!(name, "MyInt");
            assert!(matches!(visibility, Visibility::Private));
            assert!(matches!(target, TypeExpr::Named(n) if n == "Int"));
        }
        other => panic!("expected Alias, got {:?}", other),
    }
}

#[test]
fn test_pub_alias_def() {
    let m = parse_ok("pub alias StringList = List[String]");
    match &m.items[0] {
        Item::Alias(AliasDef {
            name,
            visibility,
            target,
        }) => {
            assert_eq!(name, "StringList");
            assert!(matches!(visibility, Visibility::Pub));
            assert!(matches!(target, TypeExpr::Generic(n, _) if n == "List"));
        }
        other => panic!("expected Alias, got {:?}", other),
    }
}

// ── Item 5: Self type ───────────────────────────────────────────────────

#[test]
fn test_self_type_in_param() {
    let m = parse_ok("fn foo(other: Self) -> Self { other }");
    match &m.items[0] {
        Item::Function(f) => {
            assert!(matches!(&f.params[0].ty, TypeExpr::Named(n) if n == "Self"));
            assert!(matches!(f.return_type.as_ref().unwrap(), TypeExpr::Named(n) if n == "Self"));
        }
        other => panic!("expected Function, got {:?}", other),
    }
}

// ── Item 6: list pattern ────────────────────────────────────────────────

use spore_parser::ast::Pattern;

#[test]
fn test_list_pattern_basic() {
    let src = r#"fn f(xs: List) -> Int {
        match xs {
            [h, ..tail] => h,
            _ => 0
        }
    }"#;
    let m = parse_ok(src);
    match &m.items[0] {
        Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            if let Expr::Block(_, Some(tail)) = body {
                if let Expr::Match(_, arms) = tail.as_ref() {
                    match &arms[0].pattern {
                        Pattern::List(elems, rest) => {
                            assert_eq!(elems.len(), 1);
                            assert!(matches!(&elems[0], Pattern::Var(n) if n == "h"));
                            assert_eq!(rest.as_deref(), Some("tail"));
                        }
                        other => panic!("expected List pattern, got {:?}", other),
                    }
                } else {
                    panic!("expected match");
                }
            } else {
                panic!("expected block");
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_list_pattern_no_rest() {
    let src = r#"fn f(xs: List) -> Int {
        match xs {
            [a, b] => a,
            _ => 0
        }
    }"#;
    let m = parse_ok(src);
    match &m.items[0] {
        Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            if let Expr::Block(_, Some(tail)) = body {
                if let Expr::Match(_, arms) = tail.as_ref() {
                    match &arms[0].pattern {
                        Pattern::List(elems, rest) => {
                            assert_eq!(elems.len(), 2);
                            assert!(rest.is_none());
                        }
                        other => panic!("expected List pattern, got {:?}", other),
                    }
                } else {
                    panic!("expected match");
                }
            } else {
                panic!("expected block");
            }
        }
        _ => panic!("expected function"),
    }
}

// ── Item 7: float scientific notation ───────────────────────────────────

#[test]
fn test_float_scientific_notation() {
    let tail = get_tail("fn f() -> Float { 1.5e10 }");
    match tail {
        Expr::FloatLit(v) => assert_eq!(v, 1.5e10),
        other => panic!("expected FloatLit, got {:?}", other),
    }
}

#[test]
fn test_float_scientific_negative_exponent() {
    let tail = get_tail("fn f() -> Float { 2.3E-4 }");
    match tail {
        Expr::FloatLit(v) => assert!((v - 2.3e-4).abs() < 1e-20),
        other => panic!("expected FloatLit, got {:?}", other),
    }
}

#[test]
fn test_float_scientific_positive_exponent() {
    let tail = get_tail("fn f() -> Float { 1.0e+3 }");
    match tail {
        Expr::FloatLit(v) => assert_eq!(v, 1.0e+3),
        other => panic!("expected FloatLit, got {:?}", other),
    }
}

#[test]
fn test_int_scientific_notation() {
    // An integer followed by e should also become a float
    let tail = get_tail("fn f() -> Float { 5e2 }");
    match tail {
        Expr::FloatLit(v) => assert_eq!(v, 5e2),
        other => panic!("expected FloatLit, got {:?}", other),
    }
}

// ── Batch 4 Item 1: Anonymous record types ─────────────────────────────

#[test]
fn test_record_type_in_param() {
    use spore_parser::ast::*;
    let m = parse_ok("fn f(p: { x: Int, y: Int }) -> Int { 0 }");
    match &m.items[0] {
        Item::Function(f) => match &f.params[0].ty {
            TypeExpr::Record(fields) => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].0, "x");
                assert_eq!(fields[1].0, "y");
            }
            other => panic!("expected Record type, got {:?}", other),
        },
        _ => panic!("expected function"),
    }
}

// ── Batch 4 Item 2: Associated types in capabilities ───────────────────

#[test]
fn test_capability_assoc_type() {
    use spore_parser::ast::*;
    let m = parse_ok(
        r#"
        capability Iterator[T] {
            type Output
            fn next(self: T) -> Output
        }
    "#,
    );
    match &m.items[0] {
        Item::CapabilityDef(cap) => {
            assert_eq!(cap.name, "Iterator");
            assert_eq!(cap.assoc_types.len(), 1);
            assert_eq!(cap.assoc_types[0].name, "Output");
            assert!(cap.assoc_types[0].bounds.is_empty());
            assert_eq!(cap.methods.len(), 1);
        }
        _ => panic!("expected CapabilityDef"),
    }
}

#[test]
fn test_capability_assoc_type_with_bound() {
    use spore_parser::ast::*;
    let m = parse_ok(
        r#"
        capability Container[T] {
            type Item: Display
            fn get(self: T) -> Item
        }
    "#,
    );
    match &m.items[0] {
        Item::CapabilityDef(cap) => {
            assert_eq!(cap.assoc_types.len(), 1);
            assert_eq!(cap.assoc_types[0].name, "Item");
            assert_eq!(cap.assoc_types[0].bounds.len(), 1);
        }
        _ => panic!("expected CapabilityDef"),
    }
}

// ── Placeholder partial application ─────────────────────────────────────

/// Extract the tail expression from a function body (which is a Block).
fn body_tail(f: &spore_parser::ast::FnDef) -> &spore_parser::ast::Expr {
    match f.body.as_ref().unwrap() {
        spore_parser::ast::Expr::Block(_, Some(tail)) => tail.as_ref(),
        other => other,
    }
}

#[test]
fn test_placeholder_desugars_to_lambda() {
    use spore_parser::ast::*;
    let m = parse_ok("fn main() -> Int { f(_, 2) }");
    match &m.items[0] {
        Item::Function(f) => {
            let expr = body_tail(f);
            assert!(
                matches!(expr, Expr::Lambda(params, _) if params.len() == 1 && params[0].name == "_p0"),
                "expected Lambda with 1 placeholder param, got: {expr:?}"
            );
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_placeholder_multi_params() {
    use spore_parser::ast::*;
    let m = parse_ok("fn main() -> Int { f(_, b, _) }");
    match &m.items[0] {
        Item::Function(f) => {
            let expr = body_tail(f);
            match expr {
                Expr::Lambda(params, inner) => {
                    assert_eq!(params.len(), 2);
                    assert_eq!(params[0].name, "_p0");
                    assert_eq!(params[1].name, "_p1");
                    assert!(matches!(inner.as_ref(), Expr::Call(_, args) if args.len() == 3));
                }
                _ => panic!("expected Lambda, got: {expr:?}"),
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_no_placeholder_no_desugar() {
    use spore_parser::ast::*;
    let m = parse_ok("fn main() -> Int { f(a, 2) }");
    match &m.items[0] {
        Item::Function(f) => {
            let expr = body_tail(f);
            assert!(
                matches!(expr, Expr::Call(_, _)),
                "expected Call, got: {expr:?}"
            );
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_wildcard_in_match_unchanged() {
    use spore_parser::ast::*;
    let m = parse_ok(
        r#"
        fn main() -> Int {
            match 1 {
                _ => 42,
            }
        }
    "#,
    );
    match &m.items[0] {
        Item::Function(f) => {
            let expr = body_tail(f);
            if let Expr::Match(_, arms) = expr {
                assert!(matches!(arms[0].pattern, Pattern::Wildcard));
            } else {
                panic!("expected match, got: {expr:?}");
            }
        }
        _ => panic!("expected function"),
    }
}

// ── Foreign fn ───────────────────────────────────────────────────────────

#[test]
fn test_foreign_fn_basic() {
    let m = parse_ok("foreign fn c_add(a: Int, b: Int) -> Int");
    assert_eq!(m.items.len(), 1);
    match &m.items[0] {
        Item::Function(f) => {
            assert_eq!(f.name, "c_add");
            assert!(f.is_foreign);
            assert!(f.body.is_none());
            assert_eq!(f.params.len(), 2);
            assert!(f.return_type.is_some());
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_foreign_fn_with_uses() {
    let m = parse_ok("foreign fn read_file(path: String) -> String uses [FileRead]");
    match &m.items[0] {
        Item::Function(f) => {
            assert_eq!(f.name, "read_file");
            assert!(f.is_foreign);
            assert!(f.body.is_none());
            let uses = f.uses_clause.as_ref().unwrap();
            assert_eq!(uses.resources, vec!["FileRead"]);
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_foreign_fn_no_return_type() {
    let m = parse_ok("foreign fn log(msg: String)");
    match &m.items[0] {
        Item::Function(f) => {
            assert_eq!(f.name, "log");
            assert!(f.is_foreign);
            assert!(f.body.is_none());
            assert!(f.return_type.is_none());
        }
        _ => panic!("expected function"),
    }
}

// ── Perform expression ──────────────────────────────────────────────────

#[test]
fn test_parse_perform() {
    let m = parse_ok(r#"fn main() { perform StdIO.println("hello") }"#);
    assert_eq!(m.items.len(), 1);
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            if let spore_parser::ast::Expr::Block(_, Some(tail)) = body {
                match tail.as_ref() {
                    spore_parser::ast::Expr::Perform {
                        effect,
                        operation,
                        args,
                    } => {
                        assert_eq!(effect, "StdIO");
                        assert_eq!(operation, "println");
                        assert_eq!(args.len(), 1);
                    }
                    other => panic!("expected Perform, got {other:?}"),
                }
            } else {
                panic!("expected block with tail");
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_parse_perform_multiple_args() {
    let m = parse_ok(r#"fn main() { perform IO.write("hello", 42) }"#);
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            if let spore_parser::ast::Expr::Block(_, Some(tail)) = body {
                match tail.as_ref() {
                    spore_parser::ast::Expr::Perform { args, .. } => {
                        assert_eq!(args.len(), 2);
                    }
                    other => panic!("expected Perform, got {other:?}"),
                }
            } else {
                panic!("expected block with tail");
            }
        }
        _ => panic!("expected function"),
    }
}

// ── Handle expression ───────────────────────────────────────────────────

#[test]
fn test_parse_handle() {
    let m = parse_ok(
        r#"
        fn main() {
            handle {
                perform StdIO.println("hello")
            } with {
                StdIO.println(msg) => 42
            }
        }
        "#,
    );
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            if let spore_parser::ast::Expr::Block(_, Some(tail)) = body {
                match tail.as_ref() {
                    spore_parser::ast::Expr::Handle { body: _, handlers } => {
                        assert_eq!(handlers.len(), 1);
                        assert_eq!(handlers[0].effect, "StdIO");
                        assert_eq!(handlers[0].operation, "println");
                        assert_eq!(handlers[0].params, vec!["msg".to_string()]);
                    }
                    other => panic!("expected Handle, got {other:?}"),
                }
            } else {
                panic!("expected block with tail");
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_parse_handle_multiple_arms() {
    let m = parse_ok(
        r#"
        fn main() {
            handle {
                42
            } with {
                StdIO.println(msg) => 0,
                StdIO.read_line() => "input"
            }
        }
        "#,
    );
    match &m.items[0] {
        spore_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            if let spore_parser::ast::Expr::Block(_, Some(tail)) = body {
                match tail.as_ref() {
                    spore_parser::ast::Expr::Handle { handlers, .. } => {
                        assert_eq!(handlers.len(), 2);
                        assert_eq!(handlers[0].operation, "println");
                        assert_eq!(handlers[1].operation, "read_line");
                        assert!(handlers[1].params.is_empty());
                    }
                    other => panic!("expected Handle, got {other:?}"),
                }
            } else {
                panic!("expected block with tail");
            }
        }
        _ => panic!("expected function"),
    }
}
