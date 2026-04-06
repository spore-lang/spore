//! AST-based formatter for the Spore language.
//!
//! Takes a parsed `Module` and produces canonical, formatted source code.

use crate::ast::*;

/// Format a parsed Spore module back to canonical source text.
pub fn format_module(module: &Module) -> String {
    let mut f = Formatter::new();
    f.fmt_module(module);
    let mut result = f.output;
    // Ensure trailing newline
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

struct Formatter {
    output: String,
    indent: usize,
}

impl Formatter {
    fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn writeln(&mut self, s: &str) {
        self.write_indent();
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }

    // ── Module ──────────────────────────────────────────────────────────

    fn fmt_module(&mut self, module: &Module) {
        // Module header if it has a uses clause
        if let Some(uses) = &module.uses_clause {
            self.write("module ");
            self.write(&module.name);
            self.write(" ");
            self.fmt_uses_clause(uses);
            self.newline();
            self.newline();
        }

        for (i, item) in module.items.iter().enumerate() {
            if i > 0 {
                self.newline();
            }
            self.fmt_item(item);
        }
    }

    // ── Items ───────────────────────────────────────────────────────────

    fn fmt_item(&mut self, item: &Item) {
        match item {
            Item::Function(f) => self.fmt_fn_def(f),
            Item::Const(c) => self.fmt_const(c),
            Item::StructDef(s) => self.fmt_struct_def(s),
            Item::TypeDef(t) => self.fmt_type_def(t),
            Item::CapabilityDef(c) => self.fmt_capability_def(c),
            Item::CapabilityAlias {
                name, components, ..
            } => {
                self.fmt_capability_alias(name, components);
            }
            Item::ImplDef(i) => self.fmt_impl_def(i),
            Item::Import(i) => self.fmt_import(i),
            Item::Alias(a) => self.fmt_alias(a),
        }
    }

    fn fmt_visibility(&mut self, vis: &Visibility) {
        match vis {
            Visibility::Pub => self.write("pub "),
            Visibility::PubPkg => self.write("pub(pkg) "),
            Visibility::Private => {}
        }
    }

    fn fmt_fn_def(&mut self, f: &FnDef) {
        self.write_indent();
        if f.is_unbounded {
            self.write("@unbounded\n");
            self.write_indent();
        }
        self.fmt_visibility(&f.visibility);
        self.write("fn ");
        self.write(&f.name);

        // Type params
        if !f.type_params.is_empty() {
            self.write("[");
            self.write(&f.type_params.join(", "));
            self.write("]");
        }

        // Params
        self.write("(");
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write(&p.name);
            self.write(": ");
            self.fmt_type_expr(&p.ty);
        }
        self.write(")");

        // Return type
        if let Some(ret) = &f.return_type {
            self.write(" -> ");
            self.fmt_type_expr(ret);
        }

        // Error types
        if !f.errors.is_empty() {
            self.write(" ! ");
            for (i, e) in f.errors.iter().enumerate() {
                if i > 0 {
                    self.write(" | ");
                }
                self.fmt_type_expr(e);
            }
        }

        // Where clause
        if let Some(wc) = &f.where_clause {
            self.fmt_where_clause(wc);
        }

        // Cost clause
        if let Some(cc) = &f.cost_clause {
            self.write(" cost <= ");
            self.fmt_cost_expr(&cc.bound);
        }

        // Uses clause
        if let Some(uc) = &f.uses_clause {
            self.write(" ");
            self.fmt_uses_clause(uc);
        }

        // Body
        match &f.body {
            None => {
                self.newline();
            }
            Some(body) => {
                self.write(" ");
                self.fmt_body(body);
                self.newline();
            }
        }
    }

    /// Format a function/block body. Single expressions go inline `{ expr }`,
    /// multi-statement blocks go on new lines.
    fn fmt_body(&mut self, expr: &Expr) {
        match expr {
            Expr::Block(stmts, trailing) => {
                if stmts.is_empty() && trailing.is_some() {
                    // Single-expression block — keep inline
                    self.write("{ ");
                    self.fmt_expr(trailing.as_ref().unwrap());
                    self.write(" }");
                } else {
                    // Multi-statement block
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

    fn fmt_const(&mut self, c: &ConstDef) {
        self.write_indent();
        self.fmt_visibility(&c.visibility);
        self.write("const ");
        self.write(&c.name);
        self.write(": ");
        self.fmt_type_expr(&c.ty);
        self.write(" = ");
        self.fmt_expr(&c.value);
        self.newline();
    }

    fn fmt_struct_def(&mut self, s: &StructDef) {
        self.write_indent();
        self.fmt_visibility(&s.visibility);
        self.write("struct ");
        self.write(&s.name);

        if !s.type_params.is_empty() {
            self.write("[");
            self.write(&s.type_params.join(", "));
            self.write("]");
        }

        self.write(" {");

        if s.fields.is_empty() {
            self.write("}");
        } else if s.fields.len() == 1 && s.implements.is_empty() && s.deriving.is_empty() {
            // Single field — inline
            self.write(" ");
            self.write(&s.fields[0].name);
            self.write(": ");
            self.fmt_type_expr(&s.fields[0].ty);
            self.write(" }");
        } else {
            self.newline();
            self.indent += 1;
            for field in &s.fields {
                self.write_indent();
                self.write(&field.name);
                self.write(": ");
                self.fmt_type_expr(&field.ty);
                self.write(",");
                self.newline();
            }
            // Deriving
            if !s.deriving.is_empty() {
                self.newline();
                self.writeln(&format!("deriving [{}]", s.deriving.join(", ")));
            }
            // Impl blocks
            for imp in &s.implements {
                self.newline();
                self.fmt_impl_block(imp);
            }
            self.indent -= 1;
            self.write_indent();
            self.write("}");
        }
        self.newline();
    }

    fn fmt_type_def(&mut self, t: &TypeDef) {
        self.write_indent();
        self.fmt_visibility(&t.visibility);
        self.write("type ");
        self.write(&t.name);

        if !t.type_params.is_empty() {
            self.write("[");
            self.write(&t.type_params.join(", "));
            self.write("]");
        }

        self.write(" {");
        self.newline();
        self.indent += 1;

        for variant in &t.variants {
            self.write_indent();
            self.write(&variant.name);
            if !variant.fields.is_empty() {
                self.write("(");
                for (i, f) in variant.fields.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.fmt_type_expr(f);
                }
                self.write(")");
            }
            self.write(",");
            self.newline();
        }

        // Deriving
        if !t.deriving.is_empty() {
            self.newline();
            self.writeln(&format!("deriving [{}]", t.deriving.join(", ")));
        }

        // Impl blocks
        for imp in &t.implements {
            self.newline();
            self.fmt_impl_block(imp);
        }

        self.indent -= 1;
        self.write_indent();
        self.write("}");
        self.newline();
    }

    fn fmt_impl_block(&mut self, imp: &ImplBlock) {
        self.writeln(&format!("impl {} {{", imp.capability));
        self.indent += 1;
        for (name, expr) in &imp.methods {
            self.write_indent();
            self.write("fn ");
            self.write(name);
            self.write("() ");
            self.fmt_body(expr);
            self.newline();
        }
        self.indent -= 1;
        self.writeln("}");
    }

    fn fmt_capability_def(&mut self, c: &CapabilityDef) {
        self.write_indent();
        self.fmt_visibility(&c.visibility);
        self.write("capability ");
        self.write(&c.name);

        if !c.type_params.is_empty() {
            self.write("[");
            self.write(&c.type_params.join(", "));
            self.write("]");
        }

        self.write(" {");
        self.newline();
        self.indent += 1;

        for at in &c.assoc_types {
            self.write_indent();
            self.write("type ");
            self.write(&at.name);
            if !at.bounds.is_empty() {
                self.write(": ");
                for (i, b) in at.bounds.iter().enumerate() {
                    if i > 0 {
                        self.write(" + ");
                    }
                    self.fmt_type_expr(b);
                }
            }
            self.newline();
        }

        for method in &c.methods {
            self.fmt_fn_def(method);
        }

        self.indent -= 1;
        self.write_indent();
        self.write("}");
        self.newline();
    }

    fn fmt_capability_alias(&mut self, name: &str, components: &[String]) {
        self.write_indent();
        self.write("capability ");
        self.write(name);
        self.write(" = [");
        self.write(&components.join(", "));
        self.write("]");
        self.newline();
    }

    fn fmt_impl_def(&mut self, i: &ImplDef) {
        self.write_indent();
        self.write("impl ");
        self.write(&i.capability);

        if !i.type_args.is_empty() {
            self.write("[");
            for (idx, ta) in i.type_args.iter().enumerate() {
                if idx > 0 {
                    self.write(", ");
                }
                self.fmt_type_expr(ta);
            }
            self.write("]");
        }

        self.write(" for ");
        self.write(&i.target_type);
        self.write(" {");
        self.newline();
        self.indent += 1;

        for method in &i.methods {
            self.fmt_fn_def(method);
        }

        self.indent -= 1;
        self.write_indent();
        self.write("}");
        self.newline();
    }

    fn fmt_import(&mut self, imp: &ImportDecl) {
        self.write_indent();
        match imp {
            ImportDecl::Import { path, alias, .. } => {
                self.write("import ");
                self.write(path);
                if !alias.is_empty() && alias != path {
                    self.write(" as ");
                    self.write(alias);
                }
            }
            ImportDecl::Alias { name, path, .. } => {
                self.write("import ");
                self.write(path);
                self.write(" as ");
                self.write(name);
            }
        }
        self.newline();
    }

    fn fmt_alias(&mut self, a: &AliasDef) {
        self.write_indent();
        self.fmt_visibility(&a.visibility);
        self.write("alias ");
        self.write(&a.name);
        self.write(" = ");
        self.fmt_type_expr(&a.target);
        self.newline();
    }

    // ── Clauses ─────────────────────────────────────────────────────────

    fn fmt_where_clause(&mut self, wc: &WhereClause) {
        self.write(" where ");
        for (i, c) in wc.constraints.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write(&c.type_var);
            self.write(": ");
            self.write(&c.bound);
        }
    }

    fn fmt_cost_expr(&mut self, ce: &CostExpr) {
        match ce {
            CostExpr::Literal(n) => self.write(&n.to_string()),
            CostExpr::Var(v) => self.write(v),
            CostExpr::Mul(a, b) => {
                self.fmt_cost_expr(a);
                self.write(" * ");
                self.fmt_cost_expr(b);
            }
            CostExpr::Add(a, b) => {
                self.fmt_cost_expr(a);
                self.write(" + ");
                self.fmt_cost_expr(b);
            }
        }
    }

    fn fmt_uses_clause(&mut self, uc: &UsesClause) {
        self.write("uses [");
        self.write(&uc.resources.join(", "));
        self.write("]");
    }

    // ── Type expressions ────────────────────────────────────────────────

    fn fmt_type_expr(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Named(n) => self.write(n),
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
            TypeExpr::Refinement(base, binding, pred) => {
                self.write("{ ");
                self.write(binding);
                self.write(": ");
                self.fmt_type_expr(base);
                self.write(" when ");
                self.fmt_expr(pred);
                self.write(" }");
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

    // ── Expressions ─────────────────────────────────────────────────────

    fn fmt_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::IntLit(n) => self.write(&n.to_string()),
            Expr::FloatLit(n) => {
                let s = format!("{n}");
                // Ensure there's always a decimal point
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
            Expr::CharLit(c) => {
                self.write("'");
                self.write(&escape_char(*c));
                self.write("'");
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
            Expr::Hole(name, ty, _ctx) => {
                self.write("?");
                self.write(name);
                if let Some(t) = ty {
                    self.write(": ");
                    self.fmt_type_expr(t);
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
                self.write("await ");
                self.fmt_expr(expr);
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
                    self.write(&arm.binding);
                    self.write(" from ");
                    self.fmt_expr(&arm.source);
                    self.write(" => ");
                    self.fmt_expr(&arm.body);
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
                for arm in handlers {
                    self.write_indent();
                    self.write(&arm.effect);
                    self.write(".");
                    self.write(&arm.operation);
                    self.write("(");
                    self.write(&arm.params.join(", "));
                    self.write(") => ");
                    self.fmt_expr(&arm.body);
                    self.write(",");
                    self.newline();
                }
                self.indent -= 1;
                self.write_indent();
                self.write("}");
            }
        }
    }

    fn fmt_block(&mut self, stmts: &[Stmt], trailing: Option<&Expr>) {
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

    // ── Statements ──────────────────────────────────────────────────────

    fn fmt_stmt(&mut self, stmt: &Stmt) {
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
                self.newline();
            }
            Stmt::Expr(expr) => {
                self.write_indent();
                self.fmt_expr(expr);
                self.newline();
            }
        }
    }

    // ── Patterns ────────────────────────────────────────────────────────

    fn fmt_pattern(&mut self, pat: &Pattern) {
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
}

// ── String escaping helpers ─────────────────────────────────────────────

fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn escape_char(c: char) -> String {
    match c {
        '\\' => "\\\\".to_string(),
        '\'' => "\\'".to_string(),
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        c => c.to_string(),
    }
}

fn binop_str(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::BitXor => "^",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
    }
}

fn unaryop_str(op: &UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
        UnaryOp::BitNot => "~",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    /// Helper: parse source then format it.
    fn roundtrip(source: &str) -> String {
        let module = parse(source).expect("parse failed");
        format_module(&module)
    }

    #[test]
    fn test_simple_function() {
        let src = "fn add(a: Int, b: Int) -> Int { a + b }\n";
        assert_eq!(roundtrip(src), src);
    }

    #[test]
    fn test_struct_def() {
        let src = "struct Point { x: Int, y: Int }\n";
        let out = roundtrip(src);
        // Two-field struct should be multi-line
        assert!(out.contains("struct Point {"));
        assert!(out.contains("x: Int,"));
        assert!(out.contains("y: Int,"));
    }

    #[test]
    fn test_type_def_and_match() {
        let src = concat!(
            "type Shape {\n",
            "    Circle(Int),\n",
            "    Rect(Int, Int),\n",
            "}\n",
        );
        let out = roundtrip(src);
        assert!(out.contains("type Shape {"));
        assert!(out.contains("Circle(Int),"));
        assert!(out.contains("Rect(Int, Int),"));
    }

    #[test]
    fn test_pipe_operator() {
        let src = "fn main() -> Int { 10 |> double }\n";
        let out = roundtrip(src);
        assert!(out.contains("10 |> double"));
    }

    #[test]
    fn test_lambda() {
        let src = "fn apply(f: (Int) -> Int, x: Int) -> Int { f(x) }\n";
        assert_eq!(roundtrip(src), src);
    }

    #[test]
    fn test_multi_statement_block() {
        let src = concat!(
            "fn main() -> Int {\n",
            "    let x = 1\n",
            "    let y = 2\n",
            "    x + y\n",
            "}\n",
        );
        assert_eq!(roundtrip(src), src);
    }

    #[test]
    fn test_match_expression() {
        let src = concat!(
            "fn area(s: Shape) -> Int {\n",
            "    match s {\n",
            "        Circle(r) => r * r * 3,\n",
            "        Rect(w, h) => w * h,\n",
            "    }\n",
            "}\n",
        );
        let out = roundtrip(src);
        assert!(out.contains("match s {"));
        assert!(out.contains("Circle(r) => r * r * 3,"));
        assert!(out.contains("Rect(w, h) => w * h,"));
    }

    #[test]
    fn test_uses_clause() {
        let src = "fn read() -> String uses [IO, FileRead] { ?todo }\n";
        let out = roundtrip(src);
        assert!(out.contains("uses [IO, FileRead]"));
    }

    #[test]
    fn test_blank_line_between_items() {
        let src = concat!("fn a() -> Int { 1 }\n", "\n", "fn b() -> Int { 2 }\n",);
        assert_eq!(roundtrip(src), src);
    }

    #[test]
    fn test_const_def() {
        let src = "const MAX: Int = 100\n";
        let out = roundtrip(src);
        assert!(out.contains("const MAX: Int = 100"));
    }
}
