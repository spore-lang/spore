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
    let m = parse_ok("fn add(a: I64, b: I64) -> I64 { a + b }");
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

#[test]
fn test_suffixed_integer_literal_expr() {
    let m = parse_ok("fn main() -> U8 { 7u8 }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            assert!(matches!(
                f.body.as_ref(),
                Some(sporec_parser::ast::Expr::Block(_, Some(expr)))
                    if matches!(
                        expr.as_ref(),
                        sporec_parser::ast::Expr::SuffixedIntLit(n, suffix)
                            if *n == 7 && suffix == "u8"
                    )
            ));
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_suffixed_integer_literal_direct_call() {
    let m = parse_ok("fn main() -> Never uses [Exit] { exit(7u8) }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            assert!(matches!(
                f.body.as_ref(),
                Some(sporec_parser::ast::Expr::Block(_, Some(expr)))
                    if matches!(
                        expr.as_ref(),
                        sporec_parser::ast::Expr::Call(_, args)
                            if matches!(
                                args.as_slice(),
                                [sporec_parser::ast::Expr::SuffixedIntLit(n, suffix)]
                                    if *n == 7 && suffix == "u8"
                            )
                    )
            ));
        }
        _ => panic!("expected function"),
    }
}
// ── Visibility ───────────────────────────────────────────────────────────

#[test]
fn test_pub_fn() {
    let m = parse_ok("pub fn greet() -> Str { \"hello\" }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            assert!(matches!(f.visibility, sporec_parser::ast::Visibility::Pub));
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_pub_pkg_fn() {
    let m = parse_ok("pub(pkg) fn internal() -> I64 { 42 }");
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
    let m = parse_ok("fn fetch(url: Str) -> Str uses [NetRead] { \"data\" }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let uses = f.uses_clause.as_ref().unwrap();
            assert_eq!(
                uses.surface,
                sporec_parser::ast::SurfaceExpr::Set(vec!["NetRead".into()])
            );
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_with_budget() {
    let m = parse_ok(
        r#"
        fn sort(xs: List[I64]) -> List[I64]
        budget {
            calls: 1
            holes: 0
        }
        { xs }
    "#,
    );
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let budget = f
                .budget_clause
                .as_ref()
                .expect("budget clause should parse");
            assert_eq!(budget.items.len(), 2);
            assert_eq!(budget.items[0].field, "calls");
            assert_eq!(budget.items[0].limit, 1);
            assert_eq!(budget.items[1].field, "holes");
            assert_eq!(budget.items[1].limit, 0);
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_removed_cost_clause_is_rejected() {
    let err = parse("fn wild(n: I32) -> I32 cost [O(n), 1, 0, 0] { n }")
        .expect_err("cost vector clause should be rejected");
    assert!(
        err.iter().any(|e| e
            .message
            .contains("use `budget { field: limit }` for signature budgets")),
        "unexpected parse errors: {err:?}"
    );
}

#[test]
fn test_fn_with_inline_bound() {
    let m = parse_ok("fn show[T: Display](x: T) -> Str { \"\" }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            assert_eq!(f.type_params, vec!["T"]);
            assert_eq!(f.type_param_bounds.len(), 1);
            assert_eq!(f.type_param_bounds[0].type_var, "T");
            assert_eq!(f.type_param_bounds[0].bound, "Display");
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_removed_where_clause_is_rejected() {
    let err = parse("fn show(x: T) -> Str where T: Display + Debug { \"\" }")
        .expect_err("where clause should be rejected");
    assert!(
        err.iter().any(|e| {
            e.message
                .contains("put generic bounds inline, e.g. `fn f[T: Trait](...)`")
        }),
        "unexpected parse errors: {err:?}"
    );
}

#[test]
fn test_intent_signature_inline_bounds_budget_and_properties() {
    let m = parse_ok(
        r#"
        fn group_by[T, K: Eq + Hash](xs: List[T], key: Fn[T, K]) -> List[T]
        uses [Console]
        budget {
            branches: 4
            nesting: 3
            holes: 1
        }
        properties {
            empty(): true
            preserves_count(xs: List[T]): true
        }
        {
            ?group_by_body
        }
    "#,
    );
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            assert_eq!(f.type_params, vec!["T", "K"]);
            assert_eq!(f.type_param_bounds.len(), 2);
            assert!(
                f.type_param_bounds
                    .iter()
                    .any(|bound| bound.type_var == "K" && bound.bound == "Eq")
            );
            assert!(
                f.type_param_bounds
                    .iter()
                    .any(|bound| bound.type_var == "K" && bound.bound == "Hash")
            );
            assert_eq!(f.budget_clause.as_ref().unwrap().items.len(), 3);
            assert_eq!(f.properties_clause.as_ref().unwrap().items.len(), 2);
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_with_properties_clause_preserves_item_order() {
    let m = parse_ok(
        r#"
        fn add(a: I64, b: I64) -> I64
        properties {
            left_identity(a: I64, b: I64): add(0, b) == b
            right_identity(a: I64, b: I64): add(a, 0) == a
        }
        { a + b }
    "#,
    );
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let properties = f.properties_clause.as_ref().unwrap();
            assert_eq!(properties.items.len(), 2);
            assert_eq!(properties.items[0].name, "left_identity");
            assert_eq!(properties.items[1].name, "right_identity");
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_with_block_property_predicate() {
    let m = parse_ok(
        r#"
        fn add(a: I64, b: I64) -> I64
        properties {
            block_identity(): {
                let sum = add(2, 3);
                sum == 5
            }
        }
        { a + b }
    "#,
    );
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let properties = f.properties_clause.as_ref().unwrap();
            assert!(matches!(
                properties.items[0].predicate.as_ref(),
                sporec_parser::ast::Expr::Block(_, Some(_))
            ));
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_with_refined_property_param() {
    let m = parse_ok(
        r#"
        fn abs(x: I32) -> I32
        properties {
            non_negative_identity(x: I32 when self >= 0): x >= 0
        }
        {
            if x < 0 { 0 - x } else { x }
        }
    "#,
    );
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let properties = f.properties_clause.as_ref().unwrap();
            assert_eq!(properties.items[0].params.len(), 1);
            match &properties.items[0].params[0].ty {
                sporec_parser::ast::TypeExpr::Refinement(base, binding, _) => {
                    assert!(matches!(
                        base.as_ref(),
                        sporec_parser::ast::TypeExpr::Named(name) if name == "I32"
                    ));
                    assert_eq!(binding, "self");
                }
                other => panic!("expected refinement type, got: {other:?}"),
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_fn_clauses_out_of_order_are_rejected() {
    let errors = parse(
        r#"
        fn show[T: Display](x: T) -> T
        properties {
            identity(x: T): true
        }
        uses [Console]
        budget {
            calls: 1
        }
        { x }
    "#,
    )
    .expect_err("out-of-order intent clauses should be rejected");
    assert!(errors.iter().any(|error| error.message.contains(
        "intent signature clauses must appear in order: `uses`, `budget`, `properties`"
    )));
}

#[test]
fn test_budget_accepts_named_integer_fields() {
    let m = parse_ok(
        r#"
        fn f(n: I64) -> I64
        budget {
            calls: 2
            parallelism: 1
        }
        { n }
    "#,
    );
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let budget = f.budget_clause.as_ref().expect("budget should parse");
            assert_eq!(budget.items[0].field, "calls");
            assert_eq!(budget.items[0].limit, 2);
            assert_eq!(budget.items[1].field, "parallelism");
            assert_eq!(budget.items[1].limit, 1);
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_budget_rejects_symbolic_values() {
    let err = parse(
        r#"
        fn f(n: I64) -> I64
        budget {
            calls: n
        }
        { n }
    "#,
    )
    .expect_err("budget values must be literal limits");
    assert!(
        err.iter().any(|e| e
            .message
            .contains("expected non-negative integer literal for budget `calls`")),
        "unexpected parse errors: {err:?}"
    );
}

#[test]
fn test_throw_signature_clause_is_rejected() {
    let errs =
        sporec_parser::parse("fn read(path: Str) -> Str throw [IoError] { \"x\" }").unwrap_err();
    assert!(!errs.is_empty());
}

#[test]
fn test_throw_expression_is_rejected() {
    let errs = sporec_parser::parse(r#"fn read() -> Str ! IoError { throw "error" }"#)
        .expect_err("removed throw expression should fail");
    assert!(
        errs.iter()
            .any(|error| error.message.contains("use `fail error`")),
        "unexpected parse errors: {errs:?}"
    );
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
fn test_unit_value_expression() {
    let m = parse_ok("fn main() -> () { () }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            assert!(matches!(
                f.body.as_ref(),
                Some(sporec_parser::ast::Expr::Block(_, Some(value)))
                    if matches!(value.as_ref(), sporec_parser::ast::Expr::Unit)
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
            fn show(self: T) -> Str;
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
            fn println(msg: Str) -> Unit;
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
fn test_surface_ast_shape() {
    let m = parse_ok("surface IO = [Console, FileRead, FileWrite]");
    match &m.items[0] {
        sporec_parser::ast::Item::SurfaceDef(surface) => {
            assert_eq!(surface.name, "IO");
            assert_eq!(
                surface.surface,
                sporec_parser::ast::SurfaceExpr::Set(vec![
                    "Console".into(),
                    "FileRead".into(),
                    "FileWrite".into()
                ])
            );
        }
        other => panic!("expected SurfaceDef, got {other:?}"),
    }
}

#[test]
fn test_generic_surface_reference_ast_shape() {
    let m = parse_ok("surface StateIO[T] = [State[T], Log]");
    match &m.items[0] {
        sporec_parser::ast::Item::SurfaceDef(surface) => {
            assert_eq!(surface.type_params, vec!["T"]);
            let sporec_parser::ast::SurfaceExpr::Set(references) = &surface.surface else {
                panic!("expected surface set");
            };
            assert_eq!(references[0].name, "State");
            assert!(
                matches!(&references[0].type_args[..], [sporec_parser::ast::TypeExpr::Named(name)] if name == "T")
            );
            assert_eq!(references[1].name, "Log");
            assert!(references[1].type_args.is_empty());
        }
        other => panic!("expected SurfaceDef, got {other:?}"),
    }
}

#[test]
fn test_named_surface_uses_clause_ast_shape() {
    let m = parse_ok("fn run() -> () uses IO { return }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(function) => {
            assert_eq!(
                function.uses_clause.as_ref().unwrap().surface,
                sporec_parser::ast::SurfaceExpr::Named("IO".into())
            );
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_removed_effect_alias_is_rejected() {
    let errors = sporec_parser::parse("effect IO = Console | FileRead").unwrap_err();
    assert!(
        errors[0]
            .message
            .contains("surface IO = [EffectA, EffectB]")
    );
}

#[test]
fn test_handler_item_ast_shape() {
    let m = parse_ok(
        r#"
        handler MockConsole for Console {
            fn Console.println(msg: Str) -> Unit { return }
        }
    "#,
    );
    match &m.items[0] {
        sporec_parser::ast::Item::HandlerDef(handler) => {
            assert_eq!(handler.name, "MockConsole");
            assert_eq!(
                handler.surface,
                sporec_parser::ast::SurfaceExpr::Named("Console".into())
            );
            assert_eq!(handler.impls.len(), 1);
            assert_eq!(handler.impls[0].effect, "Console");
            assert_eq!(handler.impls[0].methods[0].name, "println");
        }
        other => panic!("expected HandlerDef, got {other:?}"),
    }
}

#[test]
fn test_removed_handler_handles_form_is_rejected() {
    let errors = sporec_parser::parse(
        "handler MockConsole handles [Console] { fn Console.println(msg: Str) -> (); }",
    )
    .expect_err("removed handler form should fail");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("handler Name for Surface")),
        "expected handler migration diagnostic, got {errors:?}"
    );
}

#[test]
fn test_handler_requires_qualified_operation_names() {
    let errors =
        sporec_parser::parse("handler MockConsole for Console { fn println(msg: Str) -> (); }")
            .expect_err("unqualified handler operation should fail");
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("handler methods must name an effect operation")),
        "expected qualified operation diagnostic, got {errors:?}"
    );
}

// ── Expressions ──────────────────────────────────────────────────────────

#[test]
fn test_arithmetic_precedence() {
    // 1 + 2 * 3 should parse as 1 + (2 * 3)
    let m = parse_ok("fn f() -> I64 { 1 + 2 * 3 }");
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
    let m = parse_ok("fn f(x: I64) -> I64 { if x > 0 { x } else { 0 } }");
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
    let src = r#"fn f(x: I64) -> Str {
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
    let m = parse_ok("fn f() -> I64 { let x = 42; x }");
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
    let m = parse_ok("fn f(x: I64) -> I64 { x |> double }");
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
    let m = parse_ok("fn f() -> I64 { |x| x + 1 }");
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
    let m = parse_ok("fn f(x: I64 ! Str) -> I64 ! Str { x? }");
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
    let m = parse_ok("fn f() -> I64 { ?todo }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                sporec_parser::ast::Expr::Block(_, Some(tail)) => match tail.as_ref() {
                    sporec_parser::ast::Expr::Hole(Some(name), _, _) => {
                        assert_eq!(name, "todo")
                    }
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
    let m = parse_ok("fn f() -> I64 { ? }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            let body = f.body.as_ref().unwrap();
            match body {
                sporec_parser::ast::Expr::Block(_, Some(tail)) => match tail.as_ref() {
                    sporec_parser::ast::Expr::Hole(None, None, _) => {}
                    _ => panic!("expected unnamed hole, got {:?}", tail),
                },
                _ => panic!("expected block"),
            }
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_typed_hole() {
    let m = parse_ok("fn f() -> I64 { ?todo: I64 }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => match f.body.as_ref().unwrap() {
            sporec_parser::ast::Expr::Block(_, Some(tail)) => match tail.as_ref() {
                sporec_parser::ast::Expr::Hole(Some(name), Some(ty), _) => {
                    assert_eq!(name, "todo");
                    assert!(
                        matches!(ty.as_ref(), sporec_parser::ast::TypeExpr::Named(name) if name == "I64")
                    );
                }
                other => panic!("expected typed hole, got {other:?}"),
            },
            other => panic!("expected block, got {other:?}"),
        },
        other => panic!("expected function, got {other:?}"),
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
fn test_removed_bracketed_attribute_arguments_are_rejected() {
    let err = parse("@allows[validate, sanitize]\nfn f() -> I64 { ? }")
        .expect_err("bracketed attribute arguments should be rejected");
    assert!(
        err.iter().any(|e| e
            .message
            .contains("attribute arguments must use parentheses")),
        "unexpected parse errors: {err:?}"
    );
}

#[test]
fn test_removed_hole_annotation_is_rejected() {
    let err = parse("fn f() -> I64 { ?todo @allows[validate, sanitize] }")
        .expect_err("hole annotations should be rejected");
    assert!(
        err.iter().any(|e| e
            .message
            .contains("hole metadata annotations are not part of the current syntax")),
        "unexpected parse errors: {err:?}"
    );
}

// ── Items ────────────────────────────────────────────────────────────────

#[test]
fn test_struct_def() {
    let m = parse_ok("struct Point { x: F64, y: F64 }");
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
    let m = parse_ok("enum Option[T] { Some(T), None }");
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
fn test_capability_keyword_is_not_reserved() {
    let errs = sporec_parser::parse("capability Display[T] { fn show(self: T) -> Str }")
        .expect_err("capability-led top-level items should fail generically");
    assert!(
        errs.iter().any(|e| e.message.contains("expected item")),
        "expected generic item diagnostic, got {errs:?}"
    );
}

// ── Generic types ────────────────────────────────────────────────────────

#[test]
fn test_generic_type() {
    let m = parse_ok("fn f(xs: List[I64]) -> List[Str] { xs }");
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

#[test]
fn test_outcome_type_nested_in_generic() {
    let m = parse_ok("fn f(xs: List[I64 ! ParseError]) -> List[I64 ! ParseError] { xs }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => match &f.params[0].ty {
            sporec_parser::ast::TypeExpr::Generic(name, args) => {
                assert_eq!(name, "List");
                assert!(matches!(
                    args.as_slice(),
                    [sporec_parser::ast::TypeExpr::Outcome(_, _)]
                ));
            }
            _ => panic!("expected generic type"),
        },
        _ => panic!("expected function"),
    }
}

#[test]
fn test_outcome_type_rejects_unparenthesized_chain() {
    let errs = sporec_parser::parse("fn f() -> I64 ! ParseError ! IoError { 1 }")
        .expect_err("unparenthesized outcome chain should fail");
    assert!(
        errs.iter().any(|error| error
            .message
            .contains("cannot be chained without parentheses")),
        "unexpected parse errors: {errs:?}"
    );
}

#[test]
fn test_outcome_type_allows_explicit_nesting() {
    let m = parse_ok("fn f() -> (I64 ! ParseError) ! IoError { 1 }");
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => {
            assert!(matches!(
                f.return_type.as_ref(),
                Some(sporec_parser::ast::TypeExpr::Outcome(success, _))
                    if matches!(success.as_ref(), sporec_parser::ast::TypeExpr::Outcome(_, _))
            ));
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_outcome_patterns() {
    let m = parse_ok(
        "fn f(value: I64 ! Str) -> I64 {
            match value {
                ok number => number,
                fail _ => 0,
            }
        }",
    );
    match &m.items[0] {
        sporec_parser::ast::Item::Function(f) => match f.body.as_ref() {
            Some(sporec_parser::ast::Expr::Block(_, Some(tail))) => match tail.as_ref() {
                sporec_parser::ast::Expr::Match(_, arms) => {
                    assert!(matches!(
                        arms[0].pattern,
                        sporec_parser::ast::Pattern::OutcomeOk(_)
                    ));
                    assert!(matches!(
                        arms[1].pattern,
                        sporec_parser::ast::Pattern::OutcomeFail(_)
                    ));
                }
                _ => panic!("expected match expression"),
            },
            _ => panic!("expected function body"),
        },
        _ => panic!("expected function"),
    }
}

// ── Multiple items ───────────────────────────────────────────────────────

#[test]
fn test_multiple_items() {
    let src = r#"
        struct Point { x: F64, y: F64 }
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
    let m = parse_ok("fn f() -> I64 { add(1, 2) }");
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
    let m = parse_ok("fn f(x: Str) -> I64 { x.len() }");
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
    let m = parse_ok("fn f() -> I64 { -42 }");
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
    let m = parse_ok("fn add(a: I64, b: I64) -> I64 { a + b }");
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
    let m = parse_ok("const MAX: I64 = 100");
    assert_eq!(m.items.len(), 1);
    match &m.items[0] {
        sporec_parser::ast::Item::Const(c) => {
            assert_eq!(c.name, "MAX");
            assert!(matches!(
                c.visibility,
                sporec_parser::ast::Visibility::Private
            ));
            assert!(matches!(&c.ty, sporec_parser::ast::TypeExpr::Named(n) if n == "I64"));
            assert!(matches!(&c.value, sporec_parser::ast::Expr::IntLit(100)));
        }
        _ => panic!("expected const"),
    }
}

#[test]
fn test_pub_const_item() {
    let m = parse_ok("pub const NAME: Str = \"hello\"");
    assert_eq!(m.items.len(), 1);
    match &m.items[0] {
        sporec_parser::ast::Item::Const(c) => {
            assert_eq!(c.name, "NAME");
            assert!(matches!(c.visibility, sporec_parser::ast::Visibility::Pub));
            assert!(matches!(&c.ty, sporec_parser::ast::TypeExpr::Named(n) if n == "Str"));
            assert!(matches!(&c.value, sporec_parser::ast::Expr::StrLit(_)));
        }
        _ => panic!("expected const"),
    }
}

// ── Return / Throw / List / Str prefix tests ────────────────────────────────

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
    let tail = get_tail("fn foo(x: I64) -> I64 { return x }");
    assert!(matches!(tail, Expr::Return(Some(_))));
}

#[test]
fn test_return_no_value() {
    let tail = get_tail("fn foo() -> () { return }");
    assert!(matches!(tail, Expr::Return(None)));
}

#[test]
fn test_fail_expr() {
    let tail = get_tail(r#"fn foo() -> () ! Str { fail "error" }"#);
    assert!(matches!(tail, Expr::Fail(_)));
}

#[test]
fn test_list_literal() {
    let tail = get_tail("fn foo() -> () { [1, 2, 3] }");
    if let Expr::List(elems) = tail {
        assert_eq!(elems.len(), 3);
    } else {
        panic!("expected list literal");
    }
}

#[test]
fn test_empty_list() {
    let tail = get_tail("fn foo() -> () { [] }");
    if let Expr::List(elems) = tail {
        assert_eq!(elems.len(), 0);
    } else {
        panic!("expected empty list");
    }
}

#[test]
fn test_char_literal_is_rejected() {
    let errs = parse("fn foo() { 'a' }").expect_err("character literals should be rejected");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("character literals are not supported")),
        "unexpected parse errors: {errs:?}"
    );
}

#[test]
fn test_char_escape_is_rejected() {
    let errs = parse("fn foo() { '\\n' }").expect_err("character literals should be rejected");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("character literals are not supported")),
        "unexpected parse errors: {errs:?}"
    );
}

#[test]
fn test_raw_string() {
    let tail = get_tail("fn foo() -> Str { r\"C:\\Users\\path\" }");
    if let Expr::StrLit(s) = tail {
        assert_eq!(s, "C:\\Users\\path");
    } else {
        panic!("expected raw string, got {:?}", tail);
    }
}

#[test]
fn test_fstring() {
    let tail = get_tail("fn foo(name: Str) -> Str { f\"hello {name}\" }");
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
    let tail = get_tail("fn foo(name: Str) -> Str { t\"dear {name}\" }");
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
    let tail = get_tail("fn f() -> I64 { parallel_scope { 1 + 2 } }");
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
    let tail = get_tail("fn f() -> I64 { parallel_scope(lanes: 4) { 1 + 2 } }");
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
    let src = r#"fn f(rx1: Chan, rx2: Chan) -> I64 {
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
    let src = r#"fn f(rx1: Chan) -> I64 {
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
    let tail = get_tail("fn f() -> I64 { let t = spawn 41; t.await }");
    match tail {
        Expr::Await(inner) => assert!(matches!(*inner, Expr::Var(ref name) if name == "t")),
        other => panic!("expected Await from postfix sugar, got {:?}", other),
    }
}

#[test]
fn test_prefix_await_is_rejected() {
    let errs =
        parse("fn f() -> I64 { let t = spawn 41; await t }").expect_err("expected parse error");
    assert!(
        errs.iter()
            .any(|e| e.to_string().contains("expected expression, found Await")),
        "expected prefix await parse rejection, got: {errs:?}"
    );
}

#[test]
fn test_channel_new_sugar() {
    let tail = get_tail("fn f() -> () { Channel.new[I64](buffer: 8) }");
    match tail {
        Expr::ChannelNew { elem_type, buffer } => {
            assert!(matches!(elem_type, TypeExpr::Named(ref n) if n == "I64"));
            assert!(matches!(*buffer, Expr::IntLit(8)));
        }
        other => panic!("expected ChannelNew sugar, got {:?}", other),
    }
}

// ── Item 3: module declarations are rejected ───────────────────────────

#[test]
fn test_module_header_is_rejected() {
    let errs = parse("module mymod\nfn foo() -> I64 { 42 }").expect_err("expected parse error");
    assert!(
        errs.iter().any(|e| e
            .to_string()
            .contains("module declarations are not supported")),
        "expected module declaration rejection, got: {errs:?}"
    );
}

#[test]
fn test_module_header_with_uses_is_rejected() {
    let errs = parse("module mymod uses [NetRead]\nfn foo() -> I64 { 42 }")
        .expect_err("expected parse error");
    assert!(
        errs.iter().any(|e| e
            .to_string()
            .contains("module declarations are not supported")),
        "expected module declaration rejection, got: {errs:?}"
    );
}

// ── Item 4: transparent type alias declaration ─────────────────────────

use sporec_parser::ast::{AliasDef, Item, Visibility};

#[test]
fn test_type_alias_def() {
    let m = parse_ok("type MyInt = I64");
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
            assert!(matches!(target, TypeExpr::Named(n) if n == "I64"));
        }
        other => panic!("expected Alias, got {:?}", other),
    }
}

#[test]
fn test_pub_type_alias_def() {
    let m = parse_ok("pub type StrList = List[Str]");
    match &m.items[0] {
        Item::Alias(AliasDef {
            name,
            visibility,
            target,
            ..
        }) => {
            assert_eq!(name, "StrList");
            assert!(matches!(visibility, Visibility::Pub));
            assert!(matches!(target, TypeExpr::Generic(n, _) if n == "List"));
        }
        other => panic!("expected Alias, got {:?}", other),
    }
}

#[test]
fn test_foreign_opaque_type_def() {
    let m = parse_ok("@foreign\ntype Map[K, V];");
    match &m.items[0] {
        Item::OpaqueType(type_def) => {
            assert_eq!(type_def.name, "Map");
            assert_eq!(type_def.type_params, vec!["K", "V"]);
            assert_eq!(type_def.attributes.len(), 1);
            assert_eq!(type_def.attributes[0].name, "foreign");
        }
        other => panic!("expected OpaqueType, got {other:?}"),
    }
}

#[test]
fn test_removed_alias_keyword_is_rejected() {
    let errs = sporec_parser::parse("alias MyInt = I64")
        .expect_err("removed alias keyword should fail with a migration diagnostic");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("use `type Name = Type`")),
        "expected type alias migration diagnostic, got {errs:?}"
    );
}

#[test]
fn test_removed_type_sum_declaration_is_rejected() {
    let errs = sporec_parser::parse("type Color { Red, Green, Blue }")
        .expect_err("removed type sum declaration should fail with a migration diagnostic");
    assert!(
        errs.iter()
            .any(|e| e.message.contains("use `enum Name { ... }`")),
        "expected enum migration diagnostic, got {errs:?}"
    );
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

#[test]
fn test_receiver_self_shorthand() {
    let m = parse_ok("trait Show { fn show(self) -> Str; }");
    match &m.items[0] {
        Item::TraitDef(t) => {
            let receiver = &t.methods[0].params[0];
            assert_eq!(receiver.name, "self");
            assert!(matches!(&receiver.ty, TypeExpr::Named(n) if n == "Self"));
        }
        other => panic!("expected TraitDef, got {other:?}"),
    }
}

#[test]
fn test_receiver_self_shorthand_in_impl() {
    let m = parse_ok("impl Show for Point { fn show(self) -> Str { \"point\" } }");
    match &m.items[0] {
        Item::ImplDef(impl_def) => {
            let receiver = &impl_def.methods[0].params[0];
            assert_eq!(receiver.name, "self");
            assert!(matches!(&receiver.ty, TypeExpr::Named(n) if n == "Self"));
        }
        other => panic!("expected ImplDef, got {other:?}"),
    }
}

#[test]
fn test_generic_inherent_impl_ast_shape() {
    let m = parse_ok("impl[T: Eq + Hash] Set[T] { fn contains(self, item: T) -> Bool; }");
    match &m.items[0] {
        Item::ImplDef(impl_def) => {
            assert_eq!(impl_def.type_params, vec!["T"]);
            assert_eq!(impl_def.type_param_bounds.len(), 2);
            assert!(impl_def.target_type.is_none());
            assert!(
                matches!(&impl_def.interface_type, TypeExpr::Generic(name, args) if name == "Set" && args.len() == 1)
            );
        }
        other => panic!("expected ImplDef, got {other:?}"),
    }
}

#[test]
fn test_receiver_self_is_rejected_in_top_level_function() {
    let errors = sporec_parser::parse("fn show(self) -> Str { \"value\" }")
        .expect_err("top-level receiver should fail");
    assert!(
        errors.iter().any(|error| error.message.contains(
            "receiver `self` is only valid as the first parameter of a trait or impl member"
        )),
        "expected receiver placement diagnostic, got {errors:?}"
    );
}

#[test]
fn test_receiver_self_is_rejected_after_first_parameter() {
    let errors = sporec_parser::parse("trait Show { fn show(prefix: Str, self) -> Str; }")
        .expect_err("non-leading receiver should fail");
    assert!(
        errors.iter().any(|error| error.message.contains(
            "receiver `self` is only valid as the first parameter of a trait or impl member"
        )),
        "expected receiver placement diagnostic, got {errors:?}"
    );
}

// ── Item 6: list pattern ────────────────────────────────────────────────

use sporec_parser::ast::Pattern;

#[test]
fn test_list_pattern_basic() {
    let src = r#"fn f(xs: List) -> I64 {
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
    let src = r#"fn f(xs: List) -> I64 {
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
    let tail = get_tail("fn f() -> F64 { 1.5e10 }");
    match tail {
        Expr::FloatLit(v) => assert_eq!(v, 1.5e10),
        other => panic!("expected FloatLit, got {:?}", other),
    }
}

#[test]
fn test_float_scientific_negative_exponent() {
    let tail = get_tail("fn f() -> F64 { 2.3E-4 }");
    match tail {
        Expr::FloatLit(v) => assert!((v - 2.3e-4).abs() < 1e-20),
        other => panic!("expected FloatLit, got {:?}", other),
    }
}

#[test]
fn test_float_scientific_positive_exponent() {
    let tail = get_tail("fn f() -> F64 { 1.0e+3 }");
    match tail {
        Expr::FloatLit(v) => assert_eq!(v, 1.0e+3),
        other => panic!("expected FloatLit, got {:?}", other),
    }
}

#[test]
fn test_int_scientific_notation() {
    // An integer followed by e should also become a float
    let tail = get_tail("fn f() -> F64 { 5e2 }");
    match tail {
        Expr::FloatLit(v) => assert_eq!(v, 5e2),
        other => panic!("expected FloatLit, got {:?}", other),
    }
}

// ── Batch 4 Item 1: Anonymous record types ─────────────────────────────

#[test]
fn test_record_type_in_param() {
    use sporec_parser::ast::*;
    let m = parse_ok("fn f(p: { x: I64, y: I64 }) -> I64 { 0 }");
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
            fn next(self: T) -> Output;
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
            fn get(self: T) -> Item;
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
    let m = parse_ok("fn main() -> I64 { f(_, 2) }");
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
    let m = parse_ok("fn main() -> I64 { f(_, b, _) }");
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
    let m = parse_ok("fn main() -> I64 { f(a, 2) }");
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
        fn main() -> I64 {
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

// ── Foreign attributes ───────────────────────────────────────────────────

#[test]
fn test_foreign_attribute_basic() {
    let m = parse_ok("@foreign\nfn c_add(a: I64, b: I64) -> I64;");
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
fn test_foreign_attribute_with_uses() {
    let m = parse_ok("@foreign\nfn read_file(path: Str) -> Str uses [FileRead];");
    match &m.items[0] {
        Item::Function(f) => {
            assert_eq!(f.name, "read_file");
            assert!(f.is_foreign);
            assert!(f.body.is_none());
            let uses = f.uses_clause.as_ref().unwrap();
            assert_eq!(
                uses.surface,
                sporec_parser::ast::SurfaceExpr::Set(vec!["FileRead".into()])
            );
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_foreign_attribute_with_unit_return_type() {
    let m = parse_ok("@foreign\nfn log(msg: Str) -> ();");
    match &m.items[0] {
        Item::Function(f) => {
            assert_eq!(f.name, "log");
            assert!(f.is_foreign);
            assert!(f.body.is_none());
            assert!(f.return_type.is_some());
        }
        _ => panic!("expected function"),
    }
}

#[test]
fn test_foreign_attribute_arguments() {
    use sporec_parser::ast::{AttrArg, AttrValue, Item};

    let m = parse_ok("@foreign(\"ssl\", name = \"SSL_new\")\nfn ssl_new() -> Ptr[SSL];");
    let Item::Function(function) = &m.items[0] else {
        panic!("expected function");
    };
    assert_eq!(
        function.attributes[0].args,
        vec![
            AttrArg::Positional(AttrValue::Str("ssl".into())),
            AttrArg::Named {
                name: "name".into(),
                value: AttrValue::Str("SSL_new".into()),
            },
        ]
    );
}

#[test]
fn test_removed_foreign_keyword_is_rejected() {
    let errors = sporec_parser::parse("foreign fn log(msg: Str) -> ();").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("use `@foreign`")),
        "expected foreign attribute migration diagnostic, got {errors:?}"
    );
}

#[test]
fn test_bodyless_fn_requires_semicolon() {
    let errors = sporec_parser::parse("trait Show { fn show(self) -> Str }").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("expected Semicolon")),
        "expected missing semicolon diagnostic, got {errors:?}"
    );
}

#[test]
fn test_fn_requires_explicit_return_type() {
    let errors = sporec_parser::parse("fn main() {}").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("must declare a return type")),
        "expected explicit return type diagnostic, got {errors:?}"
    );
}

// ── Perform expression ──────────────────────────────────────────────────

#[test]
fn test_parse_perform() {
    let m = parse_ok(r#"fn main() -> () { perform StdIO.println("hello") }"#);
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
    let m = parse_ok(r#"fn main() -> () { perform IO.write("hello", 42) }"#);
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
        fn main() -> I64 {
            handle {
                perform StdIO.println("hello")
            } with {
                on StdIO.println(msg) => 42
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
                        match &handlers[0] {
                            sporec_parser::ast::HandleBinding::On(arm) => {
                                assert_eq!(arm.effect, "StdIO");
                                assert_eq!(arm.operation, "println");
                                assert_eq!(arm.params, vec!["msg".to_string()]);
                            }
                            other => panic!("expected inline handler arm, got {other:?}"),
                        }
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
        fn main() -> I64 {
            handle {
                42
            } with {
                on StdIO.println(msg) => 0,
                on StdIO.read_line() => "input"
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
                        match (&handlers[0], &handlers[1]) {
                            (
                                sporec_parser::ast::HandleBinding::On(first),
                                sporec_parser::ast::HandleBinding::On(second),
                            ) => {
                                assert_eq!(first.operation, "println");
                                assert_eq!(second.operation, "read_line");
                                assert!(second.params.is_empty());
                            }
                            other => panic!("expected inline handler arms, got {other:?}"),
                        }
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
fn test_parse_handle_named_and_inline_bindings() {
    let m = parse_ok(
        r#"
        fn main() -> I64 {
            handle {
                perform Math.double(21)
            } with {
                use DoubleMath { multiplier: 2 },
                on Console.println(msg) => 0
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
                        match &handlers[0] {
                            sporec_parser::ast::HandleBinding::Use(binding) => {
                                assert_eq!(binding.handler, "DoubleMath");
                                assert_eq!(binding.payload.len(), 1);
                                assert_eq!(binding.payload[0].0, "multiplier");
                            }
                            other => panic!("expected named handler use, got {other:?}"),
                        }
                        match &handlers[1] {
                            sporec_parser::ast::HandleBinding::On(arm) => {
                                assert_eq!(arm.effect, "Console");
                                assert_eq!(arm.operation, "println");
                            }
                            other => panic!("expected inline handler arm, got {other:?}"),
                        }
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
    let src = "fn add(a: I64, b: I64) -> I64 { a + b }";
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
    let m = parse_ok("pub struct Foo { x: I64 }");
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
    let src = "struct Point { x: I64, y: I64 }";
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
    let m = parse_ok("pub(pkg) struct Bar { y: I64 }");
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
    let src = "enum Color { Red, Green, Blue }";
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
    let m = parse_ok("struct Point { x: I64, y: I64 }");
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
    let m = parse_ok("pub enum Color { Red, Green, Blue }");
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
    let src = "const X: I64 = 1\nfn foo() -> I64 { 42 }";
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
    let m = parse_ok("enum Direction { Up, Down }");
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
    let src =
        "trait Greet {\n    fn greet(self: Self) -> Str;\n}\nstruct Bot {}\nimpl Greet for Bot {}";
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
    let m = parse_ok("pub trait Show { fn show(self: Self) -> Str { \"\" } }");
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
    let m = parse_ok("trait Debug { fn debug(self: Self) -> Str { \"\" } }");
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
        .expect_err("capability aliases should fail as ordinary invalid items");
    assert!(
        errs.iter().any(|e| e.message.contains("expected item")),
        "expected generic item diagnostic, got {errs:?}"
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
