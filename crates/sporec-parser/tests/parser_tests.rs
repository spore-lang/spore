use sporec_parser::parse;

fn parse_ok(src: &str) -> sporec_parser::ast::Module {
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
        sporec_parser::ast::Item::Function(f) => {
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
        sporec_parser::ast::Item::Function(f) => {
            assert!(matches!(f.visibility, sporec_parser::ast::Visibility::Pub));
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_pub_pkg_fn() {
    let m = parse_ok("pub(pkg) fn internal() -> Int { 42 }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            assert!(matches!(
                f.visibility,
                sporec_parser::ast::Visibility::PubPkg
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
        sporec_parser::ast::Item::Function(f) => {
            let uses = f.uses_clause.as_ref().unwrap();
            assert_eq!(uses.resources, vec!["NetRead"]);
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_with_cost() {
    let m = parse_ok("fn sort(xs: List) -> List cost [O(n), 0, 0, 0] { xs }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let cost = f.cost_clause.as_ref().expect("cost clause should parse");
            assert!(
                matches!(cost.compute, sporec_parser::ast::CostExpr::Linear(ref v) if v == "n")
            );
            assert!(matches!(
                cost.alloc,
                sporec_parser::ast::CostExpr::Literal(0)
            ));
            assert!(matches!(cost.io, sporec_parser::ast::CostExpr::Literal(0)));
            assert!(matches!(
                cost.parallel,
                sporec_parser::ast::CostExpr::Literal(0)
            ));
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_with_where() {
    let m = parse_ok("fn show(x: T) -> String where T: Display { \"\" }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let wc = f.where_clause.as_ref().unwrap();
            assert_eq!(wc.constraints.len(), 1);
            assert_eq!(wc.constraints[0].type_var, "T");
            assert_eq!(wc.constraints[0].bound, "Display");
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_where_multi_bound_is_rejected() {
    let err = parse("fn show(x: T) -> String where T: Display + Debug { \"\" }")
        .expect_err("multi-bound where clause should be rejected");
    assert!(
        err.iter().any(|e| {
            e.message
                .contains("multiple trait bounds are not supported yet")
        }),
        "unexpected parse errors: {err:?}"
    );
}

#[test]
fn test_fn_with_spec_clause_preserves_item_order() {
    let m = parse_ok(
        r#"
        fn add(a: Int, b: Int) -> Int
        spec {
            property "commutative": |a: Int, b: Int| add(a, b) == add(b, a)
            example "identity": add(0, 42) == 42
        }
        { a + b }
    "#,
    );
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let spec = f.spec_clause.as_ref().unwrap();
            assert_eq!(spec.items.len(), 2);
            assert!(matches!(
                &spec.items[0],
                sporec_parser::ast::SpecItem::Property(prop) if prop.label == "commutative"
            ));
            assert!(matches!(
                &spec.items[1],
                sporec_parser::ast::SpecItem::Example(ex) if ex.label == "identity"
            ));
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_with_block_spec_example() {
    let m = parse_ok(
        r#"
        fn add(a: Int, b: Int) -> Int
        spec {
            example "block" {
                let sum = add(2, 3)
                sum == 5
            }
        }
        { a + b }
    "#,
    );
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let spec = f.spec_clause.as_ref().unwrap();
            assert!(matches!(
                &spec.items[0],
                sporec_parser::ast::SpecItem::Example(ex)
                    if matches!(ex.body.as_ref(), sporec_parser::ast::Expr::Block(_, Some(_)))
            ));
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_clauses_parse_in_any_order() {
    let m = parse_ok(
        r#"
        fn show[T](x: T) -> T
        cost [5, 0, 0, 0]
        spec {
            example "identity": true
        }
        uses [Console]
        where T: Display
        { x }
    "#,
    );
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            assert!(f.where_clause.is_some());
            assert!(f.uses_clause.is_some());
            assert!(f.cost_clause.is_some());
            assert!(f.spec_clause.is_some());
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_scalar_cost_syntax_is_rejected() {
    let errs = parse("fn f(x: Int) -> Int cost <= 5 { x }")
        .expect_err("scalar cost syntax should be rejected");
    assert!(
        errs.iter().any(|e| e
            .message
            .contains("scalar `cost <= expr` syntax was removed")),
        "unexpected errors: {errs:?}"
    );
}

#[test]
fn test_composed_cost_slot_syntax_is_rejected() {
    let errs = parse("fn f(n: Int) -> Int cost [n + 1, 0, 0, 0] { n }")
        .expect_err("composed cost slot syntax should be rejected");
    assert!(
        errs.iter().any(|e| e.message.contains(
            "cost slot expressions only support integer literals, parameter variables, or linear `O(n)`"
        )),
        "unexpected errors: {errs:?}"
    );
}

#[test]
fn test_parenthesized_cost_slot_syntax_is_rejected() {
    let errs = parse("fn f(n: Int) -> Int cost [(n), 0, 0, 0] { n }")
        .expect_err("parenthesized cost slot syntax should be rejected");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("expected cost expression, found LParen")),
        "unexpected errors: {errs:?}"
    );
}

#[test]
fn test_throw_signature_clause_is_rejected() {
    let errs =
        sporec_parser::parse("fn read(path: Str) -> Str throw [IoError] { \"x\" }").unwrap_err();
    assert!(!errs.is_empty());
}

#[test]
fn test_width_primitive_and_unit_syntax() {
    let m = parse_ok("fn f(x: I32, y: F64, s: Str) -> () { return }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            assert!(matches!(
                f.return_type.as_ref(),
                Some(sporec_parser::ast::TypeExpr::Tuple(ts)) if ts.is_empty()
            ));
            assert!(matches!(
                &f.params[0].ty,
                sporec_parser::ast::TypeExpr::Named(n) if n == "I32"
            ));
            assert!(matches!(
                &f.params[1].ty,
                sporec_parser::ast::TypeExpr::Named(n) if n == "F64"
            ));
            assert!(matches!(
                &f.params[2].ty,
                sporec_parser::ast::TypeExpr::Named(n) if n == "Str"
            ));
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_trait_item_ast_shape() {
    let m = parse_ok(
        r#"
        trait Display[T] {
            type Output
            fn show(self: T) -> String
        }
    "#,
    );
    match &m.items[0] {
        sporec_parser::ast::Item::TraitDef(t) => {
            assert_eq!(t.name, "Display");
            assert_eq!(t.type_params, vec!["T"]);
            assert_eq!(t.assoc_types.len(), 1);
            assert_eq!(t.methods.len(), 1);
        }
        other => panic!("expected TraitDef, got {other:?}"),
    }
}

#[test]
fn test_effect_item_ast_shape() {
    let m = parse_ok(
        r#"
        effect Console {
            fn println(msg: String) -> Unit
        }
    "#,
    );
    match &m.items[0] {
        sporec_parser::ast::Item::EffectDef(effect) => {
            assert_eq!(effect.name, "Console");
            assert_eq!(effect.operations.len(), 1);
            assert_eq!(effect.operations[0].name, "println");
        }
        other => panic!("expected EffectDef, got {other:?}"),
    }
}

#[test]
fn test_effect_alias_ast_shape() {
    let m = parse_ok("effect IO = Console | FileRead | FileWrite");
    match &m.items[0] {
        sporec_parser::ast::Item::EffectAlias(alias) => {
            assert_eq!(alias.name, "IO");
            assert_eq!(alias.effects, vec!["Console", "FileRead", "FileWrite"]);
        }
        other => panic!("expected EffectAlias, got {other:?}"),
    }
}

#[test]
fn test_handler_item_ast_shape() {
    let m = parse_ok(
        r#"
        handler MockConsole for Console {
            fn println(msg: String) -> Unit { return }
        }
    "#,
    );
    match &m.items[0] {
        sporec_parser::ast::Item::HandlerDef(handler) => {
            assert_eq!(handler.name, "MockConsole");
            assert_eq!(handler.effect, "Console");
            assert_eq!(handler.methods.len(), 1);
            assert_eq!(handler.methods[0].name, "println");
        }
        other => panic!("expected HandlerDef, got {other:?}"),
    }
}

// ── Expressions ──────────────────────────────────────────────────────────

#[test]
fn test_arithmetic_precedence() {
    // 1 + 2 * 3 should parse as 1 + (2 * 3)
    let m = parse_ok("fn f() -> Int { 1 + 2 * 3 }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            // Body is Block([], Some(1 + 2*3))
            match body {
                sporec_parser::ast::Expr::Block(stmts, Some(tail)) => {
                    assert!(stmts.is_empty());
                    match tail.as_ref() {
                        sporec_parser::ast::Expr::BinOp(_, sporec_parser::ast::BinOp::Add, rhs) => {
                            assert!(matches!(
                                rhs.as_ref(),
                                sporec_parser::ast::Expr::BinOp(
                                    _,
                                    sporec_parser::ast::BinOp::Mul,
                                    _
                                )
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
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                sporec_parser::ast::Expr::Block(_, Some(tail)) => {
                    assert!(matches!(
                        tail.as_ref(),
                        sporec_parser::ast::Expr::If(_, _, Some(_))
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
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                sporec_parser::ast::Expr::Block(_, Some(tail)) => match tail.as_ref() {
                    sporec_parser::ast::Expr::Match(_, arms) => {
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
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                sporec_parser::ast::Expr::Block(stmts, Some(_tail)) => {
                    assert_eq!(stmts.len(), 1);
                    match &stmts[0] {
                        sporec_parser::ast::Stmt::Let(name, _, _) => assert_eq!(name, "x"),
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
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                sporec_parser::ast::Expr::Block(_, Some(tail)) => {
                    assert!(matches!(
                        tail.as_ref(),
                        sporec_parser::ast::Expr::Pipe(_, _)
                    ));
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
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                sporec_parser::ast::Expr::Block(_, Some(tail)) => {
                    assert!(matches!(
                        tail.as_ref(),
                        sporec_parser::ast::Expr::Lambda(_, _)
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
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                sporec_parser::ast::Expr::Block(_, Some(tail)) => {
                    assert!(matches!(tail.as_ref(), sporec_parser::ast::Expr::Try(_)));
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
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                sporec_parser::ast::Expr::Block(_, Some(tail)) => match tail.as_ref() {
                    sporec_parser::ast::Expr::Hole(Some(name), _, _) => assert_eq!(name, "todo"),
                    _ => panic!("expected hole, got {:?}", tail),
                },
                _ => panic!("expected block"),
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_unnamed_hole() {
    let m = parse_ok("fn f() -> Int { ? }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                sporec_parser::ast::Expr::Block(_, Some(tail)) => match tail.as_ref() {
                    sporec_parser::ast::Expr::Hole(None, None, None) => {}
                    _ => panic!("expected unnamed hole, got {:?}", tail),
                },
                _ => panic!("expected block"),
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_signature_type_holes() {
    let m = parse_ok("fn mystery(x: ?) -> ? { x }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            assert!(matches!(
                f.params[0].ty,
                sporec_parser::ast::TypeExpr::Hole(None)
            ));
            assert!(matches!(
                f.return_type.as_ref(),
                Some(sporec_parser::ast::TypeExpr::Hole(None))
            ));
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_allows_annotation_on_function() {
    let m = parse_ok("@allows[validate, sanitize]\nfn f() -> Int { ? }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            assert_eq!(
                f.hole_allows.as_ref().unwrap(),
                &vec!["validate".to_string(), "sanitize".to_string()]
            );
        }
        _ => panic!("expected function"),
    }
}

// ── Items ────────────────────────────────────────────────────────────────

#[test]
fn test_struct_def() {
    let m = parse_ok("struct Point { x: Float, y: Float }");
    match &m.items[0] {
        sporec_parser::ast::Item::StructDef(s) => {
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
        sporec_parser::ast::Item::TypeDef(t) => {
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
        sporec_parser::ast::Item::Import(sporec_parser::ast::ImportDecl::Import {
            path,
            alias,
            ..
        }) => {
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
        sporec_parser::ast::Item::Import(sporec_parser::ast::ImportDecl::Import {
            path,
            alias,
            ..
        }) => {
            assert_eq!(path, "std.collections.HashMap");
            assert_eq!(alias, "Map");
        }
        _ => panic!("expected import"),
    }
}

#[test]
fn test_capability_keyword_is_rejected() {
    let errs = sporec_parser::parse("capability Display[T] { fn show(self: T) -> String }")
        .expect_err("legacy capability syntax should be rejected");
    assert!(
        errs.iter().any(|e| e
            .message
            .contains("legacy `capability` syntax has been removed")),
        "expected removal diagnostic, got {errs:?}"
    );
}

// ── Generic types ────────────────────────────────────────────────────────

#[test]
fn test_generic_type() {
    let m = parse_ok("fn f(xs: List[Int]) -> List[String] { xs }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => match &f.params[0].ty {
            sporec_parser::ast::TypeExpr::Generic(name, args) => {
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
    assert!(matches!(m.items[0], sporec_parser::ast::Item::StructDef(_)));
    assert!(matches!(m.items[1], sporec_parser::ast::Item::Function(_)));
}

// ── Call expressions ─────────────────────────────────────────────────────

#[test]
fn test_call_expr() {
    let m = parse_ok("fn f() -> Int { add(1, 2) }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                sporec_parser::ast::Expr::Block(_, Some(tail)) => {
                    assert!(matches!(
                        tail.as_ref(),
                        sporec_parser::ast::Expr::Call(_, _)
                    ));
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
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                sporec_parser::ast::Expr::Block(_, Some(tail)) => {
                    assert!(matches!(
                        tail.as_ref(),
                        sporec_parser::ast::Expr::Call(_, _)
                    ));
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
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                sporec_parser::ast::Expr::Block(_, Some(tail)) => match tail.as_ref() {
                    sporec_parser::ast::Expr::StructLit(name, fields) => {
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
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                sporec_parser::ast::Expr::Block(_, Some(tail)) => {
                    assert!(matches!(
                        tail.as_ref(),
                        sporec_parser::ast::Expr::UnaryOp(sporec_parser::ast::UnaryOp::Neg, _)
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
        sporec_parser::ast::Item::Function(f) => {
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
        sporec_parser::ast::Item::Function(f) => {
            assert_eq!(f.type_params, vec!["A".to_string(), "B".to_string()]);
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_no_type_params() {
    let m = parse_ok("fn add(a: Int, b: Int) -> Int { a + b }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
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
        sporec_parser::ast::Item::Const(c) => {
            assert_eq!(c.name, "MAX");
            assert!(matches!(
                c.visibility,
                sporec_parser::ast::Visibility::Private
            ));
            assert!(matches!(&c.ty, sporec_parser::ast::TypeExpr::Named(n) if n == "Int"));
            assert!(matches!(&c.value, sporec_parser::ast::Expr::IntLit(100)));
        }
        _ => panic!("expected const"),
    }
}

#[test]
fn test_pub_const_item() {
    let m = parse_ok("pub const NAME: String = \"hello\"");
    assert_eq!(m.items.len(), 1);
    match &m.items[0] {
        sporec_parser::ast::Item::Const(c) => {
            assert_eq!(c.name, "NAME");
            assert!(matches!(c.visibility, sporec_parser::ast::Visibility::Pub));
            assert!(matches!(&c.ty, sporec_parser::ast::TypeExpr::Named(n) if n == "String"));
            assert!(matches!(&c.value, sporec_parser::ast::Expr::StrLit(_)));
        }
        _ => panic!("expected const"),
    }
}

// ── Return / Throw / List / Char / String prefix tests ──────────────────────

use sporec_parser::ast::{Expr, FStringPart, SelectArm, TStringPart, TypeExpr};

fn get_fn_body(src: &str) -> Expr {
    let m = parse_ok(src);
    let f = match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => f,
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
            assert!(matches!(
                &arms[0],
                SelectArm::Recv { binding, .. } if binding == "val"
            ));
            assert!(matches!(
                &arms[1],
                SelectArm::Recv { binding, .. } if binding == "msg"
            ));
        }
        other => panic!("expected Select, got {:?}", other),
    }
}

#[test]
fn test_select_expr_with_timeout_arm() {
    let src = r#"fn f(rx1: Chan) -> Int {
        select {
            val from rx1 => val,
            timeout(5) => 0
        }
    }"#;
    let tail = get_tail(src);
    match tail {
        Expr::Select(arms) => {
            assert_eq!(arms.len(), 2);
            assert!(matches!(
                &arms[1],
                SelectArm::Timeout {
                    duration: Expr::IntLit(5),
                    body: Expr::IntLit(0)
                }
            ));
        }
        other => panic!("expected Select with timeout, got {:?}", other),
    }
}

#[test]
fn test_task_await_postfix_sugar() {
    let tail = get_tail("fn f() -> Int { let t = spawn 41; t.await }");
    match tail {
        Expr::Await(inner) => assert!(matches!(*inner, Expr::Var(ref name) if name == "t")),
        other => panic!("expected Await from postfix sugar, got {:?}", other),
    }
}

#[test]
fn test_prefix_await_is_rejected() {
    let errs =
        parse("fn f() -> Int { let t = spawn 41; await t }").expect_err("expected parse error");
    assert!(
        errs.iter()
            .any(|e| e.to_string().contains("expected expression, found Await")),
        "expected prefix await parse rejection, got: {errs:?}"
    );
}

#[test]
fn test_channel_new_sugar() {
    let tail = get_tail("fn f() { Channel.new[Int](buffer: 8) }");
    match tail {
        Expr::ChannelNew { elem_type, buffer } => {
            assert!(matches!(elem_type, TypeExpr::Named(ref n) if n == "Int"));
            assert!(matches!(*buffer, Expr::IntLit(8)));
        }
        other => panic!("expected ChannelNew sugar, got {:?}", other),
    }
}

// ── Item 3: module declarations are rejected ───────────────────────────

#[test]
fn test_module_header_is_rejected() {
    let errs = parse("module mymod\nfn foo() -> Int { 42 }").expect_err("expected parse error");
    assert!(
        errs.iter().any(|e| e
            .to_string()
            .contains("module declarations are not supported")),
        "expected module declaration rejection, got: {errs:?}"
    );
}

#[test]
fn test_module_header_with_uses_is_rejected() {
    let errs = parse("module mymod uses [NetRead]\nfn foo() -> Int { 42 }")
        .expect_err("expected parse error");
    assert!(
        errs.iter().any(|e| e
            .to_string()
            .contains("module declarations are not supported")),
        "expected module declaration rejection, got: {errs:?}"
    );
}

// ── Item 4: alias declaration ───────────────────────────────────────────

use sporec_parser::ast::{AliasDef, Item, Visibility};

#[test]
fn test_alias_def() {
    let m = parse_ok("alias MyInt = Int");
    assert_eq!(m.items.len(), 1);
    match &m.items[0] {
        Item::Alias(AliasDef {
            name,
            visibility,
            target,
            ..
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
            ..
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

use sporec_parser::ast::Pattern;

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
    use sporec_parser::ast::*;
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

// ── Batch 4 Item 2: Associated types in traits ─────────────────────────

#[test]
fn test_trait_assoc_type() {
    use sporec_parser::ast::*;
    let m = parse_ok(
        r#"
        trait Iterator[T] {
            type Output
            fn next(self: T) -> Output
        }
    "#,
    );
    match &m.items[0] {
        Item::TraitDef(trait_def) => {
            assert_eq!(trait_def.name, "Iterator");
            assert_eq!(trait_def.assoc_types.len(), 1);
            assert_eq!(trait_def.assoc_types[0].name, "Output");
            assert!(trait_def.assoc_types[0].bounds.is_empty());
            assert_eq!(trait_def.methods.len(), 1);
        }
        _ => panic!("expected TraitDef"),
    }
}

#[test]
fn test_trait_assoc_type_with_bound() {
    use sporec_parser::ast::*;
    let m = parse_ok(
        r#"
        trait Container[T] {
            type Item: Display
            fn get(self: T) -> Item
        }
    "#,
    );
    match &m.items[0] {
        Item::TraitDef(trait_def) => {
            assert_eq!(trait_def.assoc_types.len(), 1);
            assert_eq!(trait_def.assoc_types[0].name, "Item");
            assert_eq!(trait_def.assoc_types[0].bounds.len(), 1);
        }
        _ => panic!("expected TraitDef"),
    }
}

// ── Placeholder partial application ─────────────────────────────────────

/// Extract the tail expression from a function body (which is a Block).
fn body_tail(f: &sporec_parser::ast::FnDef) -> &sporec_parser::ast::Expr {
    match f.body.as_ref().unwrap() {
        sporec_parser::ast::Expr::Block(_, Some(tail)) => tail.as_ref(),
        other => other,
    }
}

#[test]
fn test_placeholder_desugars_to_lambda() {
    use sporec_parser::ast::*;
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
    use sporec_parser::ast::*;
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
    use sporec_parser::ast::*;
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
    use sporec_parser::ast::*;
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
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            if let sporec_parser::ast::Expr::Block(_, Some(tail)) = body {
                match tail.as_ref() {
                    sporec_parser::ast::Expr::Perform {
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
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            if let sporec_parser::ast::Expr::Block(_, Some(tail)) = body {
                match tail.as_ref() {
                    sporec_parser::ast::Expr::Perform { args, .. } => {
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
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            if let sporec_parser::ast::Expr::Block(_, Some(tail)) = body {
                match tail.as_ref() {
                    sporec_parser::ast::Expr::Handle { body: _, handlers } => {
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
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            if let sporec_parser::ast::Expr::Block(_, Some(tail)) = body {
                match tail.as_ref() {
                    sporec_parser::ast::Expr::Handle { handlers, .. } => {
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

// ── Span tracking tests ─────────────────────────────────────────────────

#[test]
fn test_fn_def_has_span() {
    let src = "fn add(a: Int, b: Int) -> Int { a + b }";
    let m = parse_ok(src);
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let span = f.span.expect("FnDef should have a span");
            assert_eq!(span.start, 0);
            assert_eq!(span.end, src.len());
            assert_eq!(&src[span.start..span.end], src);
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

// ── Visibility for struct, type, trait ──────────────────────────────────

#[test]
fn test_pub_struct() {
    let m = parse_ok("pub struct Foo { x: Int }");
    match &m.items[0] {
        Item::StructDef(s) => {
            assert_eq!(s.name, "Foo");
            assert!(matches!(s.visibility, Visibility::Pub));
            assert_eq!(s.fields.len(), 1);
            assert_eq!(s.fields[0].name, "x");
        }
        other => panic!("expected StructDef, got {:?}", other),
    }
}

#[test]
fn test_struct_def_has_span() {
    let src = "struct Point { x: Int, y: Int }";
    let m = parse_ok(src);
    match &m.items[0] {
        sporec_parser::ast::Item::StructDef(s) => {
            let span = s.span.expect("StructDef should have a span");
            assert_eq!(span.start, 0);
            assert_eq!(span.end, src.len());
            assert_eq!(&src[span.start..span.end], src);
        }
        other => panic!("expected StructDef, got {other:?}"),
    }
}

#[test]
fn test_pub_pkg_struct() {
    let m = parse_ok("pub(pkg) struct Bar { y: Int }");
    match &m.items[0] {
        Item::StructDef(s) => {
            assert_eq!(s.name, "Bar");
            assert!(matches!(s.visibility, Visibility::PubPkg));
            assert_eq!(s.fields.len(), 1);
            assert_eq!(s.fields[0].name, "y");
        }
        other => panic!("expected StructDef, got {:?}", other),
    }
}

#[test]
fn test_type_def_has_span() {
    let src = "type Color { Red, Green, Blue }";
    let m = parse_ok(src);
    match &m.items[0] {
        sporec_parser::ast::Item::TypeDef(t) => {
            let span = t.span.expect("TypeDef should have a span");
            assert_eq!(span.start, 0);
            assert_eq!(span.end, src.len());
        }
        other => panic!("expected TypeDef, got {other:?}"),
    }
}

#[test]
fn test_private_struct_still_works() {
    let m = parse_ok("struct Point { x: Int, y: Int }");
    match &m.items[0] {
        Item::StructDef(s) => {
            assert_eq!(s.name, "Point");
            assert!(matches!(s.visibility, Visibility::Private));
            assert_eq!(s.fields.len(), 2);
        }
        other => panic!("expected StructDef, got {:?}", other),
    }
}

#[test]
fn test_import_has_span() {
    let src = "import std.io.File";
    let m = parse_ok(src);
    match &m.items[0] {
        sporec_parser::ast::Item::Import(sporec_parser::ast::ImportDecl::Import {
            span, ..
        }) => {
            let span = span.expect("ImportDecl should have a span");
            assert_eq!(span.start, 0);
            assert_eq!(span.end, src.len());
        }
        other => panic!("expected Import, got {other:?}"),
    }
}

#[test]
fn test_pub_type() {
    let m = parse_ok("pub type Color { Red, Green, Blue }");
    match &m.items[0] {
        Item::TypeDef(t) => {
            assert_eq!(t.name, "Color");
            assert!(matches!(t.visibility, Visibility::Pub));
            assert_eq!(t.variants.len(), 3);
        }
        other => panic!("expected TypeDef, got {:?}", other),
    }
}

#[test]
fn test_fn_span_with_leading_items() {
    let src = "const X: Int = 1\nfn foo() -> Int { 42 }";
    let m = parse_ok(src);
    // The fn item starts after the const
    match &m.items[1] {
        sporec_parser::ast::Item::Function(f) => {
            let span = f.span.expect("FnDef should have a span");
            let fn_src = &src[span.start..span.end];
            assert!(fn_src.starts_with("fn foo"), "got: {fn_src}");
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_private_type_still_works() {
    let m = parse_ok("type Direction { Up, Down }");
    match &m.items[0] {
        Item::TypeDef(t) => {
            assert_eq!(t.name, "Direction");
            assert!(matches!(t.visibility, Visibility::Private));
            assert_eq!(t.variants.len(), 2);
        }
        other => panic!("expected TypeDef, got {:?}", other),
    }
}

#[test]
fn test_error_includes_span() {
    // A missing method in an impl should report the impl's span
    let src = "trait Greet {\n    fn greet(self: Self) -> String\n}\nstruct Bot {}\nimpl Greet for Bot {}";
    let ast = parse_ok(src);
    let errs = sporec_typeck::type_check(&ast).unwrap_err();
    // The error for missing method should have a span pointing to the impl block
    let e = errs
        .iter()
        .find(|e| e.message.contains("missing method"))
        .expect("should have missing-method error");
    assert!(
        e.span.is_some(),
        "TypeError for missing method should have a span"
    );
    let span = e.span.unwrap();
    // Span should cover the impl block
    let impl_src = &src[span.start..span.end];
    assert!(
        impl_src.starts_with("impl"),
        "span should point to impl block, got: {impl_src}"
    );
}

#[test]
fn test_pub_trait() {
    let m = parse_ok("pub trait Show { fn show(self: Self) -> String { \"\" } }");
    match &m.items[0] {
        Item::TraitDef(t) => {
            assert_eq!(t.name, "Show");
            assert!(matches!(t.visibility, Visibility::Pub));
            assert_eq!(t.methods.len(), 1);
        }
        other => panic!("expected TraitDef, got {:?}", other),
    }
}

#[test]
fn test_private_trait_still_works() {
    let m = parse_ok("trait Debug { fn debug(self: Self) -> String { \"\" } }");
    match &m.items[0] {
        Item::TraitDef(t) => {
            assert_eq!(t.name, "Debug");
            assert!(matches!(t.visibility, Visibility::Private));
            assert_eq!(t.methods.len(), 1);
        }
        other => panic!("expected TraitDef, got {:?}", other),
    }
}

#[test]
fn test_capability_alias_is_rejected() {
    let errs = sporec_parser::parse("capability IO = [FileRead, FileWrite]")
        .expect_err("legacy capability aliases should be rejected");
    assert!(
        errs.iter().any(|e| e
            .message
            .contains("legacy `capability` syntax has been removed")),
        "expected removal diagnostic, got {errs:?}"
    );
}

#[test]
fn test_trait_alias_is_rejected() {
    let err = sporec_parser::parse("trait IO = FileRead | FileWrite").unwrap_err();
    assert!(
        err.iter()
            .any(|e| e.message.contains("trait aliases are not supported")),
        "expected trait alias diagnostic, got {err:?}"
    );
}
