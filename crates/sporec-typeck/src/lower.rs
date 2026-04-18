//! AST to HIR lowering pass.
//!
//! Resolves names, desugars pipe operators into function calls,
//! expands f-strings into string concatenation, and produces HIR
//! for downstream passes.

use crate::hir::*;
use sporec_parser::ast;
use std::collections::HashMap;

pub struct Lowering {
    next_def_id: DefId,
    /// Name → DefId mapping for resolution.
    names: HashMap<String, DefId>,
}

impl Lowering {
    pub fn new() -> Self {
        Self {
            next_def_id: 0,
            names: HashMap::new(),
        }
    }

    fn fresh_def_id(&mut self) -> DefId {
        let id = self.next_def_id;
        self.next_def_id += 1;
        id
    }

    fn register_name(&mut self, name: &str) -> DefId {
        if let Some(&id) = self.names.get(name) {
            id
        } else {
            let id = self.fresh_def_id();
            self.names.insert(name.to_string(), id);
            id
        }
    }

    fn resolve_name(&self, name: &str) -> DefId {
        self.names.get(name).copied().unwrap_or(UNRESOLVED)
    }

    pub fn lower_module(&mut self, module: &ast::Module) -> HirModule {
        // First pass: register all top-level names.
        for item in &module.items {
            match item {
                ast::Item::Function(f) => {
                    self.register_name(&f.name);
                }
                ast::Item::StructDef(s) => {
                    self.register_name(&s.name);
                }
                ast::Item::TypeDef(t) => {
                    self.register_name(&t.name);
                }
                ast::Item::CapabilityDef(c) => {
                    self.register_name(&c.name);
                }
                ast::Item::TraitDef(t) => {
                    self.register_name(&t.name);
                }
                ast::Item::EffectDef(e) => {
                    self.register_name(&e.name);
                }
                ast::Item::Const(c) => {
                    self.register_name(&c.name);
                }
                ast::Item::ImplDef(_)
                | ast::Item::Import(_)
                | ast::Item::Alias(_)
                | ast::Item::CapabilityAlias { .. }
                | ast::Item::EffectAlias(_)
                | ast::Item::HandlerDef(_) => {}
            }
        }

        // Second pass: lower items.
        let items = module
            .items
            .iter()
            .filter_map(|item| self.lower_item(item))
            .collect();
        HirModule { items }
    }

    fn lower_item(&mut self, item: &ast::Item) -> Option<HirItem> {
        match item {
            ast::Item::Function(f) => Some(HirItem::Function(self.lower_fn_def(f))),
            ast::Item::StructDef(s) => Some(HirItem::StructDef(self.lower_struct_def(s))),
            ast::Item::TypeDef(t) => Some(HirItem::TypeDef(self.lower_type_def(t))),
            ast::Item::CapabilityDef(c) => {
                Some(HirItem::CapabilityDef(self.lower_capability_def(c)))
            }
            ast::Item::TraitDef(t) => {
                let cap = ast::CapabilityDef {
                    name: t.name.clone(),
                    visibility: t.visibility.clone(),
                    type_params: t.type_params.clone(),
                    methods: t.methods.clone(),
                    assoc_types: t.assoc_types.clone(),
                    span: t.span,
                };
                Some(HirItem::CapabilityDef(self.lower_capability_def(&cap)))
            }
            ast::Item::ImplDef(i) => Some(HirItem::ImplDef(self.lower_impl_def(i))),
            ast::Item::Import(_)
            | ast::Item::Const(_)
            | ast::Item::Alias(_)
            | ast::Item::CapabilityAlias { .. }
            | ast::Item::EffectDef(_)
            | ast::Item::EffectAlias(_)
            | ast::Item::HandlerDef(_) => None,
        }
    }

    fn lower_fn_def(&mut self, f: &ast::FnDef) -> HirFnDef {
        let def_id = self.resolve_name(&f.name);

        let params: Vec<HirParam> = f
            .params
            .iter()
            .map(|p| {
                self.register_name(&p.name);
                HirParam {
                    name: p.name.clone(),
                    ty: self.lower_type_expr(&p.ty),
                }
            })
            .collect();

        let return_type = f.return_type.as_ref().map(|t| self.lower_type_expr(t));
        let body = f.body.as_ref().map(|e| self.lower_expr(e));

        let uses_clause = f
            .uses_clause
            .as_ref()
            .map(|uc| uc.resources.iter().cloned().collect())
            .unwrap_or_default();

        let throws = f.errors.iter().map(|t| self.lower_type_expr(t)).collect();

        let where_clause = f
            .where_clause
            .as_ref()
            .map(|wc| {
                wc.constraints
                    .iter()
                    .map(|c| HirTypeConstraint {
                        type_var: c.type_var.clone(),
                        bound: c.bound.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        HirFnDef {
            name: f.name.clone(),
            def_id,
            params,
            return_type,
            body,
            uses_clause,
            throws,
            where_clause,
            cost_bound: f
                .cost_clause
                .as_ref()
                .map(|cc| Box::new(self.lower_cost_expr(&cc.compute))),
        }
    }

    fn lower_struct_def(&mut self, s: &ast::StructDef) -> HirStructDef {
        let def_id = self.resolve_name(&s.name);
        let fields = s
            .fields
            .iter()
            .map(|f| (f.name.clone(), self.lower_type_expr(&f.ty)))
            .collect();
        HirStructDef {
            name: s.name.clone(),
            def_id,
            type_params: s.type_params.clone(),
            fields,
        }
    }

    fn lower_type_def(&mut self, t: &ast::TypeDef) -> HirTypeDef {
        let def_id = self.resolve_name(&t.name);
        let variants = t
            .variants
            .iter()
            .map(|v| HirVariant {
                name: v.name.clone(),
                fields: v.fields.iter().map(|f| self.lower_type_expr(f)).collect(),
            })
            .collect();
        HirTypeDef {
            name: t.name.clone(),
            def_id,
            type_params: t.type_params.clone(),
            variants,
        }
    }

    fn lower_capability_def(&mut self, c: &ast::CapabilityDef) -> HirCapabilityDef {
        let def_id = self.resolve_name(&c.name);
        let methods = c.methods.iter().map(|m| self.lower_fn_def(m)).collect();
        HirCapabilityDef {
            name: c.name.clone(),
            def_id,
            type_params: c.type_params.clone(),
            methods,
        }
    }

    fn lower_impl_def(&mut self, i: &ast::ImplDef) -> HirImplDef {
        let methods = i.methods.iter().map(|m| self.lower_fn_def(m)).collect();
        HirImplDef {
            capability: i.capability.clone(),
            target_type: i.target_type.clone(),
            methods,
        }
    }

    fn lower_type_expr(&self, te: &ast::TypeExpr) -> HirTypeRef {
        match te {
            ast::TypeExpr::Named(name) => match name.as_str() {
                "Int" | "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64" => {
                    HirTypeRef::Primitive(PrimitiveTy::Int)
                }
                "F32" => HirTypeRef::Primitive(PrimitiveTy::F32),
                "F64" => HirTypeRef::Primitive(PrimitiveTy::F64),
                "Bool" => HirTypeRef::Primitive(PrimitiveTy::Bool),
                "Str" => HirTypeRef::Primitive(PrimitiveTy::Str),
                "Char" => HirTypeRef::Primitive(PrimitiveTy::Char),
                "Never" => HirTypeRef::Primitive(PrimitiveTy::Never),
                _ => HirTypeRef::Named(name.clone(), self.resolve_name(name)),
            },
            ast::TypeExpr::Hole(name) => {
                HirTypeRef::Named(name.clone().unwrap_or_else(|| "_".to_string()), UNRESOLVED)
            }
            ast::TypeExpr::Generic(name, args) => HirTypeRef::Generic(
                name.clone(),
                args.iter().map(|a| self.lower_type_expr(a)).collect(),
            ),
            ast::TypeExpr::Function(params, ret, _errors) => HirTypeRef::Function(
                params.iter().map(|p| self.lower_type_expr(p)).collect(),
                Box::new(self.lower_type_expr(ret)),
            ),
            ast::TypeExpr::Tuple(ts) => {
                if ts.is_empty() {
                    HirTypeRef::Primitive(PrimitiveTy::Unit)
                } else {
                    HirTypeRef::Generic(
                        "Tuple".to_string(),
                        ts.iter().map(|t| self.lower_type_expr(t)).collect(),
                    )
                }
            }
            ast::TypeExpr::Refinement(base, _, _) => self.lower_type_expr(base),
            ast::TypeExpr::Record(fields) => HirTypeRef::Record(
                fields
                    .iter()
                    .map(|(name, t)| (name.clone(), Box::new(self.lower_type_expr(t))))
                    .collect(),
            ),
        }
    }

    fn lower_expr(&mut self, expr: &ast::Expr) -> HirExpr {
        match expr {
            ast::Expr::IntLit(v) => HirExpr::IntLit(*v),
            ast::Expr::FloatLit(v) => HirExpr::FloatLit(*v),
            ast::Expr::StrLit(v) => HirExpr::StrLit(v.clone()),
            ast::Expr::BoolLit(v) => HirExpr::BoolLit(*v),

            // Desugar f-string into nested string concatenation.
            ast::Expr::FString(parts) => self.lower_fstring(parts),

            // Desugar t-string the same way (template strings evaluate to Str).
            ast::Expr::TString(parts) => self.lower_tstring(parts),

            ast::Expr::Var(name) => {
                let def_id = self.resolve_name(name);
                HirExpr::Var(name.clone(), def_id)
            }

            ast::Expr::BinOp(lhs, op, rhs) => HirExpr::BinOp(
                Box::new(self.lower_expr(lhs)),
                lower_binop(op),
                Box::new(self.lower_expr(rhs)),
            ),

            ast::Expr::UnaryOp(op, inner) => {
                HirExpr::UnaryOp(lower_unaryop(op), Box::new(self.lower_expr(inner)))
            }

            ast::Expr::Call(callee, args) => HirExpr::Call(
                Box::new(self.lower_expr(callee)),
                args.iter().map(|a| self.lower_expr(a)).collect(),
            ),

            // KEY DESUGARING: pipe `lhs |> rhs` → `rhs(lhs)`
            ast::Expr::Pipe(lhs, rhs) => {
                let arg = self.lower_expr(lhs);
                let func = self.lower_expr(rhs);
                HirExpr::Call(Box::new(func), vec![arg])
            }

            ast::Expr::Lambda(params, body) => {
                let hir_params = params
                    .iter()
                    .map(|p| {
                        self.register_name(&p.name);
                        HirParam {
                            name: p.name.clone(),
                            ty: self.lower_type_expr(&p.ty),
                        }
                    })
                    .collect();
                HirExpr::Lambda(hir_params, Box::new(self.lower_expr(body)))
            }

            ast::Expr::If(cond, then_br, else_br) => HirExpr::If(
                Box::new(self.lower_expr(cond)),
                Box::new(self.lower_expr(then_br)),
                else_br.as_ref().map(|e| Box::new(self.lower_expr(e))),
            ),

            ast::Expr::Match(scrutinee, arms) => HirExpr::Match(
                Box::new(self.lower_expr(scrutinee)),
                arms.iter().map(|a| self.lower_match_arm(a)).collect(),
            ),

            ast::Expr::Block(stmts, tail) => {
                let hir_stmts = stmts.iter().map(|s| self.lower_stmt(s)).collect();
                let hir_tail = tail.as_ref().map(|e| Box::new(self.lower_expr(e)));
                HirExpr::Block(hir_stmts, hir_tail)
            }

            ast::Expr::FieldAccess(inner, field) => {
                HirExpr::FieldAccess(Box::new(self.lower_expr(inner)), field.clone())
            }

            ast::Expr::StructLit(name, fields) => HirExpr::StructLit(
                name.clone(),
                fields
                    .iter()
                    .map(|(n, e)| (n.clone(), self.lower_expr(e)))
                    .collect(),
            ),

            ast::Expr::Try(inner) => HirExpr::Try(Box::new(self.lower_expr(inner))),
            ast::Expr::Spawn(inner) => HirExpr::Spawn(Box::new(self.lower_expr(inner))),
            ast::Expr::Await(inner) => HirExpr::Await(Box::new(self.lower_expr(inner))),
            ast::Expr::ChannelNew { buffer, .. } => HirExpr::Call(
                Box::new(HirExpr::FieldAccess(
                    Box::new(HirExpr::Var("Channel".to_string(), UNRESOLVED)),
                    "new".to_string(),
                )),
                vec![self.lower_expr(buffer)],
            ),

            ast::Expr::Return(inner) => {
                HirExpr::Return(inner.as_ref().map(|e| Box::new(self.lower_expr(e))))
            }
            ast::Expr::Throw(inner) => HirExpr::Throw(Box::new(self.lower_expr(inner))),
            ast::Expr::List(elems) => {
                HirExpr::List(elems.iter().map(|e| self.lower_expr(e)).collect())
            }
            ast::Expr::CharLit(c) => HirExpr::CharLit(*c),

            ast::Expr::Hole(name, _ty_hint, _) => {
                HirExpr::Hole(name.clone().unwrap_or_else(|| "_".to_string()))
            }

            ast::Expr::ParallelScope { body, .. } => {
                // PoC: lower to just the body expression
                self.lower_expr(body)
            }
            ast::Expr::Select(arms) => {
                // PoC: lower to a block containing the first arm's body, or unit
                if let Some(first) = arms.first() {
                    match first {
                        ast::SelectArm::Recv { body, .. }
                        | ast::SelectArm::Timeout { body, .. } => self.lower_expr(body),
                    }
                } else {
                    HirExpr::StrLit(String::new()) // empty select → unit-like
                }
            }
            ast::Expr::Placeholder => {
                unreachable!(
                    "`_` placeholder should have been desugared into a lambda by the parser"
                )
            }

            // PoC: lower perform to a call (the effect dispatch happens at runtime)
            ast::Expr::Perform {
                effect: _,
                operation,
                args,
            } => HirExpr::Call(
                Box::new(HirExpr::Var(operation.clone(), UNRESOLVED)),
                args.iter().map(|a| self.lower_expr(a)).collect(),
            ),

            // PoC: lower handle to just the body (handler dispatch is interpreter-only)
            ast::Expr::Handle { body, .. } => self.lower_expr(body),
        }
    }

    /// Desugar an f-string into nested `Add` concatenation of string parts.
    fn lower_fstring(&mut self, parts: &[ast::FStringPart]) -> HirExpr {
        if parts.is_empty() {
            return HirExpr::StrLit(String::new());
        }

        let mut lowered: Vec<HirExpr> = parts
            .iter()
            .map(|part| match part {
                ast::FStringPart::Literal(s) => HirExpr::StrLit(s.clone()),
                ast::FStringPart::Expr(e) => {
                    // Wrap interpolated expression in a to_string call.
                    let inner = self.lower_expr(e);
                    HirExpr::Call(
                        Box::new(HirExpr::Var("to_string".to_string(), UNRESOLVED)),
                        vec![inner],
                    )
                }
            })
            .collect();

        let mut result = lowered.remove(0);
        for part in lowered {
            result = HirExpr::BinOp(Box::new(result), HirBinOp::Add, Box::new(part));
        }
        result
    }

    /// Desugar a t-string into nested `Add` concatenation of string parts.
    fn lower_tstring(&mut self, parts: &[ast::TStringPart]) -> HirExpr {
        if parts.is_empty() {
            return HirExpr::StrLit(String::new());
        }

        let mut lowered: Vec<HirExpr> = parts
            .iter()
            .map(|part| match part {
                ast::TStringPart::Literal(s) => HirExpr::StrLit(s.clone()),
                ast::TStringPart::Expr(e) => {
                    let inner = self.lower_expr(e);
                    HirExpr::Call(
                        Box::new(HirExpr::Var("to_string".to_string(), UNRESOLVED)),
                        vec![inner],
                    )
                }
            })
            .collect();

        let mut result = lowered.remove(0);
        for part in lowered {
            result = HirExpr::BinOp(Box::new(result), HirBinOp::Add, Box::new(part));
        }
        result
    }

    fn lower_stmt(&mut self, stmt: &ast::Stmt) -> HirStmt {
        match stmt {
            ast::Stmt::Let(name, ty, init) => {
                self.register_name(name);
                HirStmt::Let(
                    name.clone(),
                    ty.as_ref().map(|t| self.lower_type_expr(t)),
                    self.lower_expr(init),
                )
            }
            ast::Stmt::Expr(expr) => HirStmt::Expr(self.lower_expr(expr)),
        }
    }

    fn lower_match_arm(&mut self, arm: &ast::MatchArm) -> HirMatchArm {
        HirMatchArm {
            pattern: self.lower_pattern(&arm.pattern),
            guard: arm.guard.as_ref().map(|g| self.lower_expr(g)),
            body: self.lower_expr(&arm.body),
        }
    }

    fn lower_pattern(&mut self, pat: &ast::Pattern) -> HirPattern {
        match pat {
            ast::Pattern::Wildcard => HirPattern::Wildcard,
            ast::Pattern::Var(name) => {
                self.register_name(name);
                HirPattern::Var(name.clone())
            }
            ast::Pattern::IntLit(v) => HirPattern::IntLit(*v),
            ast::Pattern::StrLit(v) => HirPattern::StrLit(v.clone()),
            ast::Pattern::BoolLit(v) => HirPattern::BoolLit(*v),
            ast::Pattern::Constructor(name, pats) => HirPattern::Constructor(
                name.clone(),
                pats.iter().map(|p| self.lower_pattern(p)).collect(),
            ),
            ast::Pattern::Struct(name, fields) => HirPattern::Struct(
                name.clone(),
                fields
                    .iter()
                    .map(|(n, p)| (n.clone(), self.lower_pattern(p)))
                    .collect(),
            ),
            ast::Pattern::Or(pats) => {
                HirPattern::Or(pats.iter().map(|p| self.lower_pattern(p)).collect())
            }
            ast::Pattern::List(elements, _rest) => {
                // PoC: lower list pattern as a constructor-like pattern
                HirPattern::Constructor(
                    "List".to_string(),
                    elements.iter().map(|p| self.lower_pattern(p)).collect(),
                )
            }
        }
    }

    /// Lower a cost expression AST node into an HIR expression.
    fn lower_cost_expr(&self, ce: &ast::CostExpr) -> HirExpr {
        match ce {
            ast::CostExpr::Literal(n) => HirExpr::IntLit(*n as i64),
            ast::CostExpr::Var(name) => {
                let def_id = self.resolve_name(name);
                HirExpr::Var(name.clone(), def_id)
            }
            ast::CostExpr::Linear(name) => {
                let def_id = self.resolve_name(name);
                HirExpr::Var(name.clone(), def_id)
            }
        }
    }
}

impl Default for Lowering {
    fn default() -> Self {
        Self::new()
    }
}

fn lower_binop(op: &ast::BinOp) -> HirBinOp {
    match op {
        ast::BinOp::Add => HirBinOp::Add,
        ast::BinOp::Sub => HirBinOp::Sub,
        ast::BinOp::Mul => HirBinOp::Mul,
        ast::BinOp::Div => HirBinOp::Div,
        ast::BinOp::Mod => HirBinOp::Mod,
        ast::BinOp::Eq => HirBinOp::Eq,
        ast::BinOp::Ne => HirBinOp::Ne,
        ast::BinOp::Lt => HirBinOp::Lt,
        ast::BinOp::Gt => HirBinOp::Gt,
        ast::BinOp::Le => HirBinOp::Le,
        ast::BinOp::Ge => HirBinOp::Ge,
        ast::BinOp::And => HirBinOp::And,
        ast::BinOp::Or => HirBinOp::Or,
        ast::BinOp::BitAnd => HirBinOp::BitAnd,
        ast::BinOp::BitOr => HirBinOp::BitOr,
        ast::BinOp::BitXor => HirBinOp::BitXor,
        ast::BinOp::Shl => HirBinOp::Shl,
        ast::BinOp::Shr => HirBinOp::Shr,
    }
}

fn lower_unaryop(op: &ast::UnaryOp) -> HirUnaryOp {
    match op {
        ast::UnaryOp::Neg => HirUnaryOp::Neg,
        ast::UnaryOp::Not => HirUnaryOp::Not,
        ast::UnaryOp::BitNot => HirUnaryOp::BitNot,
    }
}
