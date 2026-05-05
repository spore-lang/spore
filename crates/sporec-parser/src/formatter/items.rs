use crate::ast::*;

use super::Formatter;

impl<'a> Formatter<'a> {
    pub(super) fn fmt_fn_def(&mut self, f: &FnDef) {
        self.write_indent();
        if f.is_unbounded {
            self.write("@unbounded\n");
            self.write_indent();
        }
        if let Some(allows) = &f.hole_allows {
            self.write("@allows[");
            self.write(&allows.join(", "));
            self.write("]\n");
            self.write_indent();
        }
        self.fmt_visibility(&f.visibility);
        if f.is_foreign {
            self.write("foreign ");
        }
        self.write("fn ");
        self.write(&f.name);

        if !f.type_params.is_empty() {
            self.write("[");
            self.write(&f.type_params.join(", "));
            self.write("]");
        }

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

        if let Some(ret) = &f.return_type {
            self.write(" -> ");
            self.fmt_type_expr(ret);
        }

        if !f.errors.is_empty() {
            self.write(" ! ");
            for (i, e) in f.errors.iter().enumerate() {
                if i > 0 {
                    self.write(" | ");
                }
                self.fmt_type_expr(e);
            }
        }

        if let Some(wc) = &f.where_clause {
            self.fmt_where_clause(wc);
        }

        if let Some(uc) = &f.uses_clause {
            self.write(" ");
            self.fmt_uses_clause(uc);
        }

        if let Some(cc) = &f.cost_clause {
            self.write(" cost [");
            self.fmt_cost_expr(&cc.compute);
            self.write(", ");
            self.fmt_cost_expr(&cc.alloc);
            self.write(", ");
            self.fmt_cost_expr(&cc.io);
            self.write(", ");
            self.fmt_cost_expr(&cc.parallel);
            self.write("]");
        }

        if let Some(sc) = &f.spec_clause {
            self.newline();
            self.write_indent();
            self.fmt_spec_clause(sc);
        }

        match &f.body {
            None => {
                self.newline();
            }
            Some(body) => {
                if f.spec_clause.is_some() {
                    self.newline();
                    self.write_indent();
                } else {
                    self.write(" ");
                }
                self.fmt_body(body);
                self.newline();
            }
        }
    }

    pub(super) fn fmt_spec_clause(&mut self, spec: &SpecClause) {
        self.write("spec {");
        self.newline();
        self.indent += 1;
        for item in &spec.items {
            self.write_indent();
            self.fmt_spec_item(item);
            self.newline();
        }
        self.indent -= 1;
        self.write_indent();
        self.write("}");
    }

    pub(super) fn fmt_spec_item(&mut self, item: &SpecItem) {
        match item {
            SpecItem::Example(ex) => {
                self.write("example \"");
                self.write(&ex.label);
                self.write("\"");
                match ex.body.as_ref() {
                    Expr::Block(..) => {
                        self.write(" ");
                        self.fmt_expr(ex.body.as_ref());
                    }
                    _ => {
                        self.write(": ");
                        self.fmt_expr(ex.body.as_ref());
                    }
                }
            }
            SpecItem::Property(prop) => {
                self.write("property \"");
                self.write(&prop.label);
                self.write("\": ");
                self.fmt_expr(prop.predicate.as_ref());
            }
        }
    }

    pub(super) fn fmt_const(&mut self, c: &ConstDef) {
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

    pub(super) fn fmt_struct_def(&mut self, s: &StructDef) {
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
            if !s.deriving.is_empty() {
                self.newline();
                self.writeln(&format!("deriving [{}]", s.deriving.join(", ")));
            }
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

    pub(super) fn fmt_type_def(&mut self, t: &TypeDef) {
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

        if !t.deriving.is_empty() {
            self.newline();
            self.writeln(&format!("deriving [{}]", t.deriving.join(", ")));
        }

        for imp in &t.implements {
            self.newline();
            self.fmt_impl_block(imp);
        }

        self.indent -= 1;
        self.write_indent();
        self.write("}");
        self.newline();
    }

    pub(super) fn fmt_impl_block(&mut self, imp: &ImplBlock) {
        self.writeln(&format!("impl {} {{", imp.trait_name));
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

    pub(super) fn fmt_trait_def(&mut self, t: &TraitDef) {
        self.write_indent();
        self.fmt_visibility(&t.visibility);
        self.write("trait ");
        self.write(&t.name);
        if !t.type_params.is_empty() {
            self.write("[");
            self.write(&t.type_params.join(", "));
            self.write("]");
        }
        self.write(" {");
        self.newline();
        self.indent += 1;
        for at in &t.assoc_types {
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
        for m in &t.methods {
            self.write_indent();
            self.fmt_fn_def(m);
            self.newline();
        }
        self.indent -= 1;
        self.write_indent();
        self.write("}");
        self.newline();
    }

    pub(super) fn fmt_effect_def(&mut self, e: &EffectDef) {
        self.write_indent();
        self.fmt_visibility(&e.visibility);
        self.write("effect ");
        self.write(&e.name);
        self.write(" {");
        self.newline();
        self.indent += 1;
        for op in &e.operations {
            self.write_indent();
            self.fmt_fn_def(op);
            self.newline();
        }
        self.indent -= 1;
        self.write_indent();
        self.write("}");
        self.newline();
    }

    pub(super) fn fmt_effect_alias(&mut self, ea: &EffectAlias) {
        self.write_indent();
        self.fmt_visibility(&ea.visibility);
        self.write("effect ");
        self.write(&ea.name);
        self.write(" = ");
        self.write(&ea.effects.join(" | "));
        self.newline();
    }

    pub(super) fn fmt_handler_def(&mut self, h: &HandlerDef) {
        self.write_indent();
        self.write("handler ");
        self.write(&h.name);
        if !h.fields.is_empty() {
            self.write("(");
            for (idx, field) in h.fields.iter().enumerate() {
                if idx > 0 {
                    self.write(", ");
                }
                self.write(&field.name);
                self.write(": ");
                self.fmt_type_expr(&field.ty);
            }
            self.write(")");
        }
        self.write(" handles [");
        self.write(&h.handles_clause.effects.join(", "));
        self.write("]");
        if let Some(uses_clause) = &h.uses_clause {
            self.write(" ");
            self.fmt_uses_clause(uses_clause);
        }
        self.write(" {");
        self.newline();
        self.indent += 1;
        for handler_impl in &h.impls {
            self.write_indent();
            self.write("impl ");
            self.write(&handler_impl.effect);
            self.write(" {");
            self.newline();
            self.indent += 1;
            for method in &handler_impl.methods {
                self.fmt_fn_def(method);
            }
            self.indent -= 1;
            self.write_indent();
            self.write("}");
            self.newline();
        }
        self.indent -= 1;
        self.write_indent();
        self.write("}");
        self.newline();
    }

    pub(super) fn fmt_impl_def(&mut self, i: &ImplDef) {
        self.write_indent();
        self.write("impl ");
        self.write(&i.trait_name);

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

    pub(super) fn fmt_import(&mut self, imp: &ImportDecl) {
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

    pub(super) fn fmt_alias(&mut self, a: &AliasDef) {
        self.write_indent();
        self.fmt_visibility(&a.visibility);
        self.write("alias ");
        self.write(&a.name);
        self.write(" = ");
        self.fmt_type_expr(&a.target);
        self.newline();
    }

    pub(super) fn fmt_where_clause(&mut self, wc: &WhereClause) {
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

    pub(super) fn fmt_cost_expr(&mut self, ce: &CostExpr) {
        match ce {
            CostExpr::Literal(n) => self.write(&n.to_string()),
            CostExpr::Var(v) => self.write(v),
            CostExpr::Linear(v) => {
                self.write("O(");
                self.write(v);
                self.write(")");
            }
        }
    }

    pub(super) fn fmt_uses_clause(&mut self, uc: &UsesClause) {
        self.write("uses [");
        self.write(&uc.resources.join(", "));
        self.write("]");
    }
}
