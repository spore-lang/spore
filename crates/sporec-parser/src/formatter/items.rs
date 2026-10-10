use crate::ast::*;

use super::{Formatter, escape_str};

impl<'a> Formatter<'a> {
    pub(super) fn fmt_fn_def(&mut self, f: &FnDef) {
        self.fmt_attributes(&f.attributes);
        self.write_indent();
        self.fmt_visibility(&f.visibility);
        self.write("fn ");
        self.write(&f.name);

        self.fmt_type_params_with_inline_bounds(&f.type_params, &f.type_param_bounds);

        self.write("(");
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.fmt_param(p);
        }
        self.write(")");

        if let Some(ret) = &f.return_type {
            self.write(" -> ");
            self.fmt_type_expr(ret);
        }

        if let Some(uc) = &f.uses_clause {
            self.write(" ");
            self.fmt_uses_clause(uc);
        }

        if let Some(bc) = &f.budget_clause {
            self.newline();
            self.write_indent();
            self.fmt_budget_clause(bc);
        }

        if let Some(pc) = &f.properties_clause {
            self.newline();
            self.write_indent();
            self.fmt_properties_clause(pc);
        }

        let has_block_clause = f.budget_clause.is_some() || f.properties_clause.is_some();

        match &f.body {
            None => {
                self.write(";");
                self.newline();
            }
            Some(body) => {
                if has_block_clause {
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

    fn fmt_param(&mut self, param: &Param) {
        self.write(&param.name);
        if param.name != "self" || !matches!(&param.ty, TypeExpr::Named(name) if name == "Self") {
            self.write(": ");
            self.fmt_type_expr(&param.ty);
        }
    }

    pub(super) fn fmt_budget_clause(&mut self, budget: &BudgetClause) {
        self.write("budget {");
        self.newline();
        self.indent += 1;
        for item in &budget.items {
            self.write_indent();
            self.write(&item.field);
            self.write(": ");
            self.write(&item.limit.to_string());
            self.newline();
        }
        self.indent -= 1;
        self.write_indent();
        self.write("}");
    }

    pub(super) fn fmt_properties_clause(&mut self, properties: &PropertiesClause) {
        self.write("properties {");
        self.newline();
        self.indent += 1;
        for item in &properties.items {
            self.write_indent();
            self.write(&item.name);
            self.write("(");
            for (idx, param) in item.params.iter().enumerate() {
                if idx > 0 {
                    self.write(", ");
                }
                self.write(&param.name);
                self.write(": ");
                self.fmt_type_expr(&param.ty);
            }
            self.write("): ");
            self.fmt_expr(item.predicate.as_ref());
            self.newline();
        }
        self.indent -= 1;
        self.write_indent();
        self.write("}");
    }

    pub(super) fn fmt_const(&mut self, c: &ConstDef) {
        self.fmt_attributes(&c.attributes);
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
        self.fmt_attributes(&s.attributes);
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
        self.fmt_attributes(&t.attributes);
        self.write_indent();
        self.fmt_visibility(&t.visibility);
        self.write("enum ");
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
        self.fmt_attributes(&t.attributes);
        self.write_indent();
        self.fmt_visibility(&t.visibility);
        self.write("trait ");
        self.write(&t.name);
        self.fmt_type_params_with_inline_bounds(&t.type_params, &t.type_param_bounds);
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
            self.fmt_fn_def(m);
        }
        self.indent -= 1;
        self.write_indent();
        self.write("}");
        self.newline();
    }

    pub(super) fn fmt_effect_def(&mut self, e: &EffectDef) {
        self.fmt_attributes(&e.attributes);
        self.write_indent();
        self.fmt_visibility(&e.visibility);
        self.write("effect ");
        self.write(&e.name);
        self.fmt_type_params_with_inline_bounds(&e.type_params, &e.type_param_bounds);
        self.write(" {");
        self.newline();
        self.indent += 1;
        for op in &e.operations {
            self.fmt_fn_def(op);
        }
        self.indent -= 1;
        self.write_indent();
        self.write("}");
        self.newline();
    }

    pub(super) fn fmt_surface_def(&mut self, surface: &SurfaceDef) {
        self.fmt_attributes(&surface.attributes);
        self.write_indent();
        self.fmt_visibility(&surface.visibility);
        self.write("surface ");
        self.write(&surface.name);
        self.fmt_type_params_with_inline_bounds(&surface.type_params, &surface.type_param_bounds);
        self.write(" = ");
        self.fmt_surface_expr(&surface.surface);
        self.newline();
    }

    pub(super) fn fmt_handler_def(&mut self, h: &HandlerDef) {
        self.fmt_attributes(&h.attributes);
        self.write_indent();
        self.fmt_visibility(&h.visibility);
        self.write("handler ");
        self.write(&h.name);
        self.write(" for ");
        self.fmt_surface_expr(&h.surface);
        self.write(" {");
        self.newline();
        self.indent += 1;
        for handler_impl in &h.impls {
            for method in &handler_impl.methods {
                self.fmt_attributes(&method.attributes);
                self.write_indent();
                self.write("fn ");
                self.write(&handler_impl.effect);
                self.write(".");
                self.write(&method.name);
                self.fmt_fn_def_tail(method);
            }
        }
        self.indent -= 1;
        self.write_indent();
        self.write("}");
        self.newline();
    }

    fn fmt_fn_def_tail(&mut self, f: &FnDef) {
        self.fmt_type_params_with_inline_bounds(&f.type_params, &f.type_param_bounds);
        self.write("(");
        for (idx, param) in f.params.iter().enumerate() {
            if idx > 0 {
                self.write(", ");
            }
            self.fmt_param(param);
        }
        self.write(")");
        if let Some(return_type) = &f.return_type {
            self.write(" -> ");
            self.fmt_type_expr(return_type);
        }
        if let Some(uses_clause) = &f.uses_clause {
            self.write(" ");
            self.fmt_uses_clause(uses_clause);
        }
        match &f.body {
            Some(body) => {
                self.write(" ");
                self.fmt_body(body);
            }
            None => self.write(";"),
        }
        self.newline();
    }

    pub(super) fn fmt_impl_def(&mut self, i: &ImplDef) {
        self.fmt_attributes(&i.attributes);
        self.write_indent();
        self.write("impl ");
        self.fmt_type_params_with_inline_bounds(&i.type_params, &i.type_param_bounds);
        if !i.type_params.is_empty() {
            self.write(" ");
        }
        self.fmt_type_expr(&i.interface_type);
        if let Some(target_type) = &i.target_type {
            self.write(" for ");
            self.fmt_type_expr(target_type);
        }
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
        self.fmt_attributes(&a.attributes);
        self.write_indent();
        self.fmt_visibility(&a.visibility);
        self.write("type ");
        self.write(&a.name);
        if !a.type_params.is_empty() {
            self.write("[");
            self.write(&a.type_params.join(", "));
            self.write("]");
        }
        self.write(" = ");
        self.fmt_type_expr(&a.target);
        self.newline();
    }

    pub(super) fn fmt_opaque_type(&mut self, t: &OpaqueTypeDef) {
        self.fmt_attributes(&t.attributes);
        self.write_indent();
        self.fmt_visibility(&t.visibility);
        self.write("type ");
        self.write(&t.name);
        if !t.type_params.is_empty() {
            self.write("[");
            self.write(&t.type_params.join(", "));
            self.write("]");
        }
        self.write(";");
        self.newline();
    }

    fn fmt_type_params_with_inline_bounds(
        &mut self,
        type_params: &[String],
        bounds: &[TypeConstraint],
    ) {
        if type_params.is_empty() {
            return;
        }

        self.write("[");
        for (idx, type_param) in type_params.iter().enumerate() {
            if idx > 0 {
                self.write(", ");
            }
            self.write(type_param);
            let param_bounds = bounds
                .iter()
                .filter(|constraint| constraint.type_var == *type_param)
                .map(|constraint| constraint.bound.as_str())
                .collect::<Vec<_>>();
            if !param_bounds.is_empty() {
                self.write(": ");
                self.write(&param_bounds.join(" + "));
            }
        }
        self.write("]");
    }

    fn fmt_attributes(&mut self, attributes: &[Attribute]) {
        for attribute in attributes {
            self.write_indent();
            self.write("@");
            self.write(&attribute.name);
            if !attribute.args.is_empty() {
                self.write("(");
                for (idx, arg) in attribute.args.iter().enumerate() {
                    if idx > 0 {
                        self.write(", ");
                    }
                    match arg {
                        AttrArg::Positional(value) => self.fmt_attr_value(value),
                        AttrArg::Named { name, value } => {
                            self.write(name);
                            self.write(" = ");
                            self.fmt_attr_value(value);
                        }
                    }
                }
                self.write(")");
            }
            self.newline();
        }
    }

    fn fmt_attr_value(&mut self, value: &AttrValue) {
        match value {
            AttrValue::Ident(value) => self.write(value),
            AttrValue::Str(value) => {
                self.write("\"");
                self.write(&escape_str(value));
                self.write("\"");
            }
            AttrValue::Int(value) => self.write(&value.to_string()),
        }
    }

    pub(super) fn fmt_uses_clause(&mut self, uc: &UsesClause) {
        self.write("uses ");
        self.fmt_surface_expr(&uc.surface);
    }

    fn fmt_surface_expr(&mut self, surface: &SurfaceExpr) {
        match surface {
            SurfaceExpr::Named(reference) => self.fmt_surface_ref(reference),
            SurfaceExpr::Set(references) => {
                self.write("[");
                for (idx, reference) in references.iter().enumerate() {
                    if idx > 0 {
                        self.write(", ");
                    }
                    self.fmt_surface_ref(reference);
                }
                self.write("]");
            }
        }
    }

    fn fmt_surface_ref(&mut self, reference: &SurfaceRef) {
        self.write(&reference.name);
        if !reference.type_args.is_empty() {
            self.write("[");
            for (idx, type_arg) in reference.type_args.iter().enumerate() {
                if idx > 0 {
                    self.write(", ");
                }
                self.fmt_type_expr(type_arg);
            }
            self.write("]");
        }
    }
}
