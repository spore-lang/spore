//! Realization-shape budget checking.
//!
//! This module checks SEP-0004 named `budget { ... }` blocks against
//! source-level realization shape rather than machine resources.

use std::collections::HashMap;

use sporec_parser::ast::{Expr, FnDef, HandleBinding, Item, Module, SelectArm, Stmt};

use crate::error::{ErrorCode, TypeError};
use crate::hole::{BudgetConstraint, BudgetContext, BudgetObservation, HoleReport};

const BUILTIN_FIELDS: &[&str] = &[
    "branches",
    "nesting",
    "recursion",
    "parallelism",
    "calls",
    "effects",
    "holes",
];

#[derive(Debug, Clone, Copy, Default)]
struct Shape {
    branches: u64,
    nesting: u64,
    recursion: u64,
    parallelism: u64,
    calls: u64,
    effects: u64,
    holes: u64,
}

impl Shape {
    fn observed(self, field: &str) -> Option<u64> {
        match field {
            "branches" => Some(self.branches),
            "nesting" => Some(self.nesting),
            "recursion" => Some(self.recursion),
            "parallelism" => Some(self.parallelism),
            "calls" => Some(self.calls),
            "effects" => Some(self.effects),
            "holes" => Some(self.holes),
            _ => None,
        }
    }
}

pub fn check_module_budget_errors(module: &Module) -> Vec<TypeError> {
    let mut errors = Vec::new();

    for item in &module.items {
        let Item::Function(function) = item else {
            continue;
        };
        let Some(budget) = &function.budget_clause else {
            continue;
        };

        let mut limits = HashMap::new();
        for budget_item in &budget.items {
            if !BUILTIN_FIELDS.contains(&budget_item.field.as_str()) {
                let diagnostic = TypeError::new(
                    ErrorCode::B0102,
                    format!(
                        "function `{}` declares unknown budget field `{}`",
                        function.name, budget_item.field
                    ),
                );
                errors.push(if let Some(span) = budget_item.span {
                    TypeError::with_span(diagnostic.code, diagnostic.message, span)
                } else {
                    diagnostic
                });
                continue;
            }
            limits.insert(budget_item.field.as_str(), budget_item.limit);
        }

        let observed = observe_function(function);
        for (field, limit) in limits {
            let Some(actual) = observed.observed(field) else {
                continue;
            };
            if actual <= limit {
                continue;
            }
            let code = match field {
                "recursion" if limit == 0 => ErrorCode::B0201,
                "holes" => ErrorCode::B0202,
                _ => ErrorCode::B0101,
            };
            errors.push(TypeError::new(
                code,
                format!(
                    "function `{}` exceeds budget `{field}`: observed {actual} > limit {limit}",
                    function.name
                ),
            ));
        }
    }

    errors
}

/// Attach intent-signature budget context to holes in the checked module.
pub fn enrich_hole_report_with_budgets(module: &Module, report: &mut HoleReport) {
    for item in &module.items {
        let Item::Function(function) = item else {
            continue;
        };
        let Some(budget) = &function.budget_clause else {
            continue;
        };

        let observed = observe_function(function);
        let constraints = budget
            .items
            .iter()
            .map(|item| BudgetConstraint {
                field: item.field.clone(),
                limit: item.limit,
            })
            .collect::<Vec<_>>();
        let observations = budget
            .items
            .iter()
            .filter_map(|item| {
                observed
                    .observed(&item.field)
                    .map(|actual| BudgetObservation {
                        field: item.field.clone(),
                        observed: actual,
                        remaining: item.limit.checked_sub(actual),
                    })
            })
            .collect::<Vec<_>>();
        let context = BudgetContext {
            constraints,
            observations,
        };

        for hole in report
            .holes
            .iter_mut()
            .filter(|hole| hole.function == function.name)
        {
            hole.budget_context = Some(context.clone());
        }
    }
}

fn observe_function(function: &FnDef) -> Shape {
    let mut shape = Shape::default();
    if let Some(body) = &function.body {
        observe_expr(body, &function.name, 0, &mut shape);
    }
    shape
}

fn observe_stmt(stmt: &Stmt, function_name: &str, depth: u64, shape: &mut Shape) {
    match stmt {
        Stmt::Let(_, _, expr) | Stmt::Expr(expr) => observe_expr(expr, function_name, depth, shape),
    }
}

fn observe_expr(expr: &Expr, function_name: &str, depth: u64, shape: &mut Shape) {
    match expr {
        Expr::IntLit(_)
        | Expr::SuffixedIntLit(_, _)
        | Expr::FloatLit(_)
        | Expr::StrLit(_)
        | Expr::FString(_)
        | Expr::BoolLit(_)
        | Expr::Unit
        | Expr::Var(_)
        | Expr::Return(None)
        | Expr::TString(_)
        | Expr::Placeholder => {}
        Expr::Call(callee, args) => {
            shape.calls += 1;
            if matches!(callee.as_ref(), Expr::Var(name) if name == function_name) {
                shape.recursion += 1;
            }
            observe_expr(callee, function_name, depth, shape);
            for arg in args {
                observe_expr(arg, function_name, depth, shape);
            }
        }
        Expr::Lambda(params, body) => {
            for param in params {
                observe_type_expr(&param.ty, function_name, depth, shape);
            }
            observe_expr(body, function_name, depth, shape);
        }
        Expr::BinOp(lhs, _, rhs) | Expr::Pipe(lhs, rhs) => {
            observe_expr(lhs, function_name, depth, shape);
            observe_expr(rhs, function_name, depth, shape);
        }
        Expr::UnaryOp(_, inner)
        | Expr::FieldAccess(inner, _)
        | Expr::Try(inner)
        | Expr::Spawn(inner)
        | Expr::Await(inner)
        | Expr::Return(Some(inner))
        | Expr::Fail(inner) => {
            if matches!(expr, Expr::Spawn(_)) {
                shape.parallelism += 1;
            }
            observe_expr(inner, function_name, depth, shape);
        }
        Expr::If(cond, then_branch, else_branch) => {
            shape.branches += if else_branch.is_some() { 2 } else { 1 };
            let child_depth = depth + 1;
            shape.nesting = shape.nesting.max(child_depth);
            observe_expr(cond, function_name, child_depth, shape);
            observe_expr(then_branch, function_name, child_depth, shape);
            if let Some(else_branch) = else_branch {
                observe_expr(else_branch, function_name, child_depth, shape);
            }
        }
        Expr::Match(scrutinee, arms) => {
            shape.branches += arms.len() as u64;
            let child_depth = depth + 1;
            shape.nesting = shape.nesting.max(child_depth);
            observe_expr(scrutinee, function_name, child_depth, shape);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    observe_expr(guard, function_name, child_depth, shape);
                }
                observe_expr(&arm.body, function_name, child_depth, shape);
            }
        }
        Expr::Block(stmts, tail) => {
            for stmt in stmts {
                observe_stmt(stmt, function_name, depth, shape);
            }
            if let Some(tail) = tail {
                observe_expr(tail, function_name, depth, shape);
            }
        }
        Expr::Hole(_, ty, _) => {
            shape.holes += 1;
            if let Some(ty) = ty {
                observe_type_expr(ty, function_name, depth, shape);
            }
        }
        Expr::StructLit(_, fields) => {
            for (_, value) in fields {
                observe_expr(value, function_name, depth, shape);
            }
        }
        Expr::List(items) => {
            for item in items {
                observe_expr(item, function_name, depth, shape);
            }
        }
        Expr::ChannelNew { elem_type, buffer } => {
            observe_type_expr(elem_type, function_name, depth, shape);
            observe_expr(buffer, function_name, depth, shape);
        }
        Expr::ParallelScope { lanes, body } => {
            let child_depth = depth + 1;
            shape.nesting = shape.nesting.max(child_depth);
            if let Some(lanes) = lanes {
                if let Expr::IntLit(n) = lanes.as_ref()
                    && *n > 0
                {
                    shape.parallelism = shape.parallelism.max(*n as u64);
                }
                observe_expr(lanes, function_name, child_depth, shape);
            }
            observe_expr(body, function_name, child_depth, shape);
        }
        Expr::Select(arms) => {
            shape.branches += arms.len() as u64;
            let child_depth = depth + 1;
            shape.nesting = shape.nesting.max(child_depth);
            for arm in arms {
                match arm {
                    SelectArm::Recv { source, body, .. } => {
                        observe_expr(source, function_name, child_depth, shape);
                        observe_expr(body, function_name, child_depth, shape);
                    }
                    SelectArm::Timeout { duration, body } => {
                        observe_expr(duration, function_name, child_depth, shape);
                        observe_expr(body, function_name, child_depth, shape);
                    }
                }
            }
        }
        Expr::Perform { args, .. } => {
            shape.effects += 1;
            for arg in args {
                observe_expr(arg, function_name, depth, shape);
            }
        }
        Expr::Handle { body, handlers } => {
            let child_depth = depth + 1;
            shape.nesting = shape.nesting.max(child_depth);
            observe_expr(body, function_name, child_depth, shape);
            for handler in handlers {
                match handler {
                    HandleBinding::Use(handler_use) => {
                        for (_, value) in &handler_use.payload {
                            observe_expr(value, function_name, child_depth, shape);
                        }
                    }
                    HandleBinding::On(effect_arm) => {
                        observe_expr(&effect_arm.body, function_name, child_depth, shape);
                    }
                }
            }
        }
    }
}

fn observe_type_expr(
    ty: &sporec_parser::ast::TypeExpr,
    function_name: &str,
    depth: u64,
    shape: &mut Shape,
) {
    match ty {
        sporec_parser::ast::TypeExpr::Refinement(_, _, predicate) => {
            observe_expr(predicate, function_name, depth, shape);
        }
        sporec_parser::ast::TypeExpr::Generic(_, args)
        | sporec_parser::ast::TypeExpr::Tuple(args) => {
            for arg in args {
                observe_type_expr(arg, function_name, depth, shape);
            }
        }
        sporec_parser::ast::TypeExpr::Function(params, ret) => {
            for param in params {
                observe_type_expr(param, function_name, depth, shape);
            }
            observe_type_expr(ret, function_name, depth, shape);
        }
        sporec_parser::ast::TypeExpr::Outcome(success, failure) => {
            observe_type_expr(success, function_name, depth, shape);
            observe_type_expr(failure, function_name, depth, shape);
        }
        sporec_parser::ast::TypeExpr::Record(fields) => {
            for (_, field_ty) in fields {
                observe_type_expr(field_ty, function_name, depth, shape);
            }
        }
        sporec_parser::ast::TypeExpr::Named(_) | sporec_parser::ast::TypeExpr::Hole(_) => {}
    }
}
