use crate::ast::*;

use super::{Formatter, binop_str, escape_str, unaryop_str};

impl<'a> Formatter<'a> {
    /// Format a function/block body. Single expressions go inline `{ expr }`,
    /// multi-statement blocks go on new lines.
    pub(super) fn fmt_body(&mut self, expr: &Expr) {
        match expr {
            Expr::Block(stmts, trailing) => {
                if stmts.is_empty() && trailing.is_some() {
                    self.write("{ ");
                    self.fmt_expr(trailing.as_ref().unwrap());
                    self.write(" }");
                } else {
                    self.write("{");
                    self.newline();
                    self.indent += 1;
                    for stmt in stmts {
                        self.fmt_stmt(stmt);
                    }
                    if let Some(trail) = trailing {
                        self.write_indent();
                        self.fmt_expr(trail);
                        self.newline();
                    }
                    self.indent -= 1;
                    self.write_indent();
                    self.write("}");
                }
            }
            _ => {
                self.write("{ ");
                self.fmt_expr(expr);
                self.write(" }");
            }
        }
    }

    pub(super) fn fmt_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::IntLit(n) => self.write(&n.to_string()),
            Expr::FloatLit(n) => {
                let s = format!("{n}");
                if s.contains('.') {
                    self.write(&s);
                } else {
                    self.write(&format!("{s}.0"));
                }
            }
            Expr::StrLit(s) => {
                self.write("\"");
                self.write(&escape_str(s));
                self.write("\"");
            }
            Expr::BoolLit(b) => self.write(if *b { "true" } else { "false" }),
            Expr::Var(v) => self.write(v),
            Expr::Call(func, args) => {
                self.fmt_expr(func);
                self.write("(");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.fmt_expr(a);
                }
                self.write(")");
            }
            Expr::Lambda(params, body) => {
                self.write("|");
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&p.name);
                    self.write(": ");
                    self.fmt_type_expr(&p.ty);
                }
                self.write("| ");
                self.fmt_expr(body);
            }
            Expr::BinOp(lhs, op, rhs) => {
                self.fmt_expr(lhs);
                self.write(" ");
                self.write(binop_str(op));
                self.write(" ");
                self.fmt_expr(rhs);
            }
            Expr::UnaryOp(op, expr) => {
                self.write(unaryop_str(op));
                self.fmt_expr(expr);
            }
            Expr::FieldAccess(expr, field) => {
                self.fmt_expr(expr);
                self.write(".");
                self.write(field);
            }
            Expr::Pipe(lhs, rhs) => {
                self.fmt_expr(lhs);
                self.write(" |> ");
                self.fmt_expr(rhs);
            }
            Expr::If(cond, then, else_) => {
                self.write("if ");
                self.fmt_expr(cond);
                self.write(" ");
                self.fmt_body(then);
                if let Some(el) = else_ {
                    self.write(" else ");
                    self.fmt_body(el);
                }
            }
            Expr::Match(scrutinee, arms) => {
                self.write("match ");
                self.fmt_expr(scrutinee);
                self.write(" {");
                self.newline();
                self.indent += 1;
                for arm in arms {
                    self.write_indent();
                    self.fmt_pattern(&arm.pattern);
                    if let Some(guard) = &arm.guard {
                        self.write(" if ");
                        self.fmt_expr(guard);
                    }
                    self.write(" => ");
                    self.fmt_expr(&arm.body);
                    self.write(",");
                    self.newline();
                }
                self.indent -= 1;
                self.write_indent();
                self.write("}");
            }
            Expr::Block(stmts, trailing) => {
                self.fmt_block(stmts, trailing.as_deref());
            }
            Expr::Try(expr) => {
                self.write("try ");
                self.fmt_expr(expr);
            }
            Expr::Hole(name, ty, allows) => {
                self.write("?");
                if let Some(name) = name {
                    self.write(name);
                }
                if let Some(t) = ty {
                    self.write(": ");
                    self.fmt_type_expr(t);
                }
                if let Some(allows) = allows {
                    self.write(" @allows[");
                    for (i, name) in allows.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.write(name);
                    }
                    self.write("]");
                }
            }
            Expr::StructLit(name, fields) => {
                self.write(name);
                self.write(" { ");
                for (i, (fname, fexpr)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(fname);
                    self.write(": ");
                    self.fmt_expr(fexpr);
                }
                self.write(" }");
            }
            Expr::Spawn(expr) => {
                self.write("spawn ");
                self.fmt_expr(expr);
            }
            Expr::Await(expr) => {
                self.fmt_expr(expr);
                self.write(".await");
            }
            Expr::ChannelNew { elem_type, buffer } => {
                self.write("Channel.new[");
                self.fmt_type_expr(elem_type);
                self.write("](buffer: ");
                self.fmt_expr(buffer);
                self.write(")");
            }
            Expr::Return(expr) => {
                self.write("return");
                if let Some(e) = expr {
                    self.write(" ");
                    self.fmt_expr(e);
                }
            }
            Expr::Throw(expr) => {
                self.write("throw ");
                self.fmt_expr(expr);
            }
            Expr::List(elems) => {
                self.write("[");
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.fmt_expr(e);
                }
                self.write("]");
            }
            Expr::FString(parts) => {
                self.write("f\"");
                for part in parts {
                    match part {
                        FStringPart::Literal(s) => self.write(&escape_str(s)),
                        FStringPart::Expr(e) => {
                            self.write("{");
                            self.fmt_expr(e);
                            self.write("}");
                        }
                    }
                }
                self.write("\"");
            }
            Expr::TString(parts) => {
                self.write("t\"");
                for part in parts {
                    match part {
                        TStringPart::Literal(s) => self.write(&escape_str(s)),
                        TStringPart::Expr(e) => {
                            self.write("{");
                            self.fmt_expr(e);
                            self.write("}");
                        }
                    }
                }
                self.write("\"");
            }
            Expr::ParallelScope { lanes, body } => {
                self.write("parallel_scope");
                if let Some(l) = lanes {
                    self.write("(lanes: ");
                    self.fmt_expr(l);
                    self.write(")");
                }
                self.write(" ");
                self.fmt_body(body);
            }
            Expr::Select(arms) => {
                self.write("select {");
                self.newline();
                self.indent += 1;
                for arm in arms {
                    self.write_indent();
                    match arm {
                        SelectArm::Recv {
                            binding,
                            source,
                            body,
                        } => {
                            self.write(binding);
                            self.write(" from ");
                            self.fmt_expr(source);
                            self.write(" => ");
                            self.fmt_expr(body);
                        }
                        SelectArm::Timeout { duration, body } => {
                            self.write("timeout(");
                            self.fmt_expr(duration);
                            self.write(") => ");
                            self.fmt_expr(body);
                        }
                    }
                    self.write(",");
                    self.newline();
                }
                self.indent -= 1;
                self.write_indent();
                self.write("}");
            }
            Expr::Placeholder => self.write("_"),
            Expr::Perform {
                effect,
                operation,
                args,
            } => {
                self.write("perform ");
                self.write(effect);
                self.write(".");
                self.write(operation);
                self.write("(");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.fmt_expr(a);
                }
                self.write(")");
            }
            Expr::Handle { body, handlers } => {
                self.write("handle ");
                self.fmt_body(body);
                self.write(" with {");
                self.newline();
                self.indent += 1;
                for binding in handlers {
                    self.write_indent();
                    match binding {
                        HandleBinding::Use(handler_use) => {
                            self.write("use ");
                            self.write(&handler_use.handler);
                            self.write(" {");
                            for (idx, (field, value)) in handler_use.payload.iter().enumerate() {
                                if idx > 0 {
                                    self.write(", ");
                                }
                                self.write(field);
                                self.write(": ");
                                self.fmt_expr(value);
                            }
                            self.write("}");
                        }
                        HandleBinding::On(arm) => {
                            self.write("on ");
                            self.write(&arm.effect);
                            self.write(".");
                            self.write(&arm.operation);
                            self.write("(");
                            self.write(&arm.params.join(", "));
                            self.write(") => ");
                            self.fmt_expr(&arm.body);
                        }
                    }
                    self.write(",");
                    self.newline();
                }
                self.indent -= 1;
                self.write_indent();
                self.write("}");
            }
        }
    }

    pub(super) fn fmt_block(&mut self, stmts: &[Stmt], trailing: Option<&Expr>) {
        if stmts.is_empty()
            && let Some(trail) = trailing
        {
            self.write("{ ");
            self.fmt_expr(trail);
            self.write(" }");
            return;
        }
        self.write("{");
        self.newline();
        self.indent += 1;
        for stmt in stmts {
            self.fmt_stmt(stmt);
        }
        if let Some(trail) = trailing {
            self.write_indent();
            self.fmt_expr(trail);
            self.newline();
        }
        self.indent -= 1;
        self.write_indent();
        self.write("}");
    }

    pub(super) fn fmt_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(name, ty, expr) => {
                self.write_indent();
                self.write("let ");
                self.write(name);
                if let Some(t) = ty {
                    self.write(": ");
                    self.fmt_type_expr(t);
                }
                self.write(" = ");
                self.fmt_expr(expr);
                self.write(";");
                self.newline();
            }
            Stmt::Expr(expr) => {
                self.write_indent();
                self.fmt_expr(expr);
                self.write(";");
                self.newline();
            }
        }
    }

    pub(super) fn fmt_pattern(&mut self, pat: &Pattern) {
        match pat {
            Pattern::Wildcard => self.write("_"),
            Pattern::Var(v) => self.write(v),
            Pattern::IntLit(n) => self.write(&n.to_string()),
            Pattern::StrLit(s) => {
                self.write("\"");
                self.write(&escape_str(s));
                self.write("\"");
            }
            Pattern::BoolLit(b) => self.write(if *b { "true" } else { "false" }),
            Pattern::Constructor(name, pats) => {
                self.write(name);
                if !pats.is_empty() {
                    self.write("(");
                    for (i, p) in pats.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.fmt_pattern(p);
                    }
                    self.write(")");
                }
            }
            Pattern::Struct(name, fields) => {
                self.write(name);
                self.write(" { ");
                for (i, (fname, fpat)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(fname);
                    self.write(": ");
                    self.fmt_pattern(fpat);
                }
                self.write(" }");
            }
            Pattern::Or(pats) => {
                for (i, p) in pats.iter().enumerate() {
                    if i > 0 {
                        self.write(" | ");
                    }
                    self.fmt_pattern(p);
                }
            }
            Pattern::List(elems, rest) => {
                self.write("[");
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.fmt_pattern(e);
                }
                if let Some(r) = rest {
                    if !elems.is_empty() {
                        self.write(", ");
                    }
                    self.write("..");
                    self.write(r);
                }
                self.write("]");
            }
        }
    }

    pub(super) fn fmt_type_expr(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Named(n) => self.write(n),
            TypeExpr::Hole(name) => {
                self.write("?");
                if let Some(name) = name {
                    self.write(name);
                }
            }
            TypeExpr::Generic(name, args) => {
                self.write(name);
                self.write("[");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.fmt_type_expr(a);
                }
                self.write("]");
            }
            TypeExpr::Tuple(elems) => {
                self.write("(");
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.fmt_type_expr(e);
                }
                self.write(")");
            }
            TypeExpr::Function(params, ret, errors) => {
                self.write("(");
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.fmt_type_expr(p);
                }
                self.write(") -> ");
                self.fmt_type_expr(ret);
                if !errors.is_empty() {
                    self.write(" ! ");
                    for (i, e) in errors.iter().enumerate() {
                        if i > 0 {
                            self.write(" | ");
                        }
                        self.fmt_type_expr(e);
                    }
                }
            }
            TypeExpr::Refinement(base, _binding, pred) => {
                self.fmt_type_expr(base);
                self.write(" when ");
                self.fmt_expr(pred);
            }
            TypeExpr::Record(fields) => {
                self.write("{ ");
                for (i, (name, ty)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(name);
                    self.write(": ");
                    self.fmt_type_expr(ty);
                }
                self.write(" }");
            }
        }
    }
}
