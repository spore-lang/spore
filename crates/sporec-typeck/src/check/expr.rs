use super::*;

impl Checker {
    pub(super) fn check_expr(&mut self, expr: &Expr) -> Ty {
        match expr {
            Expr::IntLit(_) => Ty::I32,
            Expr::FloatLit(_) => Ty::F64,
            Expr::StrLit(_) => Ty::Str,
            Expr::BoolLit(_) => Ty::Bool,
            Expr::FString(_) => Ty::Str,
            Expr::TString(_) => Ty::Str,

            Expr::Var(name) => {
                if let Some(ty) = self.env.lookup(name) {
                    ty.clone()
                } else if let Some((params, ret, caps)) = self.registry.functions.get(name).cloned()
                {
                    if params.is_empty() && self.find_unit_variant(name).is_some() {
                        if let Some(type_params) = self.registry.fn_type_params.get(name).cloned() {
                            let (_, ret, _) = self.instantiate_sig(&type_params, &[], &ret);
                            ret
                        } else {
                            ret
                        }
                    } else {
                        // bare function name as value — return its function type
                        let errors = self
                            .registry
                            .fn_errors
                            .get(name)
                            .cloned()
                            .unwrap_or_default();
                        Ty::Fn(params, Box::new(ret), caps, errors)
                    }
                } else if let Some((params, ret, caps)) = self.lookup_module_function(name) {
                    Ty::Fn(params, Box::new(ret), caps, ErrorSet::new())
                } else {
                    self.err(ErrorCode::E0004, format!("undefined variable `{name}`"));
                    Ty::Error
                }
            }

            Expr::BinOp(lhs, op, rhs) => self.check_binop(lhs, op, rhs),

            Expr::UnaryOp(op, expr) => {
                let ty = self.check_expr(expr);
                match op {
                    UnaryOp::Neg => {
                        if !ty.is_numeric() && !ty.is_error() {
                            self.err(ErrorCode::E0002, format!("cannot negate type `{ty}`"));
                        }
                        ty
                    }
                    UnaryOp::Not => {
                        if ty != Ty::Bool && !ty.is_error() {
                            self.err(ErrorCode::E0002, format!("cannot apply `!` to type `{ty}`"));
                        }
                        Ty::Bool
                    }
                    UnaryOp::BitNot => {
                        if !ty.is_integer() && !ty.is_error() {
                            self.err(ErrorCode::E0002, format!("cannot apply `~` to type `{ty}`"));
                        }
                        ty
                    }
                }
            }

            Expr::Call(callee, args) => self.check_call(callee, args),

            Expr::Lambda(params, body) => {
                self.env.push_scope();
                let param_tys: Vec<Ty> = params
                    .iter()
                    .map(|p| {
                        let ty = self.resolve_type(&p.ty);
                        self.env.define(p.name.clone(), ty.clone());
                        ty
                    })
                    .collect();
                let ret_ty = self.check_expr(body);
                self.env.pop_scope();
                Ty::Fn(
                    param_tys,
                    Box::new(ret_ty),
                    EffectSet::new(),
                    ErrorSet::new(),
                )
            }

            Expr::If(cond, then_branch, else_branch) => {
                let cond_ty = self.check_expr(cond);
                if cond_ty != Ty::Bool && !cond_ty.is_error() {
                    self.err(
                        ErrorCode::E0001,
                        format!("if condition must be Bool, got `{cond_ty}`"),
                    );
                }
                let then_ty = self.check_expr(then_branch);
                if let Some(else_expr) = else_branch {
                    let else_ty = self.check_expr(else_expr);
                    // If one branch diverges (Never), the overall type is the other branch.
                    if matches!(then_ty, Ty::Never) {
                        else_ty
                    } else if matches!(else_ty, Ty::Never) {
                        then_ty
                    } else {
                        self.unify(&then_ty, &else_ty, "if/else branches");
                        then_ty
                    }
                } else {
                    // No else branch: the expression types as Unit.
                    // Unify then_ty with Unit so non-Unit then-branches are flagged.
                    self.unify(&Ty::Unit, &then_ty, "if without else must be Unit");
                    Ty::Unit
                }
            }

            Expr::Match(scrutinee, arms) => {
                let scrut_ty = self.check_expr(scrutinee);
                let scrut_ty = self.apply_subst(&scrut_ty);

                // Check exhaustiveness
                self.check_exhaustiveness(&scrut_ty, arms);

                let mut result_ty: Option<Ty> = None;
                for arm in arms {
                    // Check pattern against scrutinee type and get bindings
                    let bindings = self.check_pattern(&arm.pattern, &scrut_ty);

                    // Create a new scope with pattern bindings
                    self.env.push_scope();
                    for (name, ty) in bindings {
                        self.env.define(name, ty);
                    }

                    // Check guard if present
                    if let Some(guard) = &arm.guard {
                        let guard_ty = self.check_expr(guard);
                        if guard_ty != Ty::Bool && !guard_ty.is_error() {
                            self.err(
                                ErrorCode::E0017,
                                format!("match guard must be Bool, got `{guard_ty}`"),
                            );
                        }
                    }

                    let arm_ty = self.check_expr(&arm.body);
                    self.env.pop_scope();

                    if let Some(ref expected) = result_ty {
                        // If the accumulated result type is Never (all prior arms diverged),
                        // adopt this arm's type. If this arm diverges, keep the existing type.
                        if matches!(expected, Ty::Never) {
                            result_ty = Some(arm_ty);
                        } else if !matches!(arm_ty, Ty::Never) {
                            self.unify(expected, &arm_ty, "match arms");
                        }
                    } else {
                        result_ty = Some(arm_ty);
                    }
                }
                result_ty.unwrap_or(Ty::Unit)
            }

            Expr::Block(stmts, tail) => {
                self.env.push_scope();
                for stmt in stmts {
                    self.check_stmt(stmt);
                }
                let ty = if let Some(tail_expr) = tail {
                    self.check_expr(tail_expr)
                } else {
                    Ty::Unit
                };
                self.env.pop_scope();
                ty
            }

            Expr::Pipe(lhs, rhs) => {
                let arg_ty = self.check_expr(lhs);
                let fn_ty = self.check_expr(rhs);
                match fn_ty {
                    Ty::Fn(params, ret, caps, errors) => {
                        if params.len() != 1 {
                            self.err(
                                ErrorCode::E0009,
                                format!(
                                    "pipe target expects 1 argument, function takes {}",
                                    params.len()
                                ),
                            );
                        } else {
                            self.unify(&params[0], &arg_ty, "pipe argument");
                        }
                        self.check_effect_propagation(&caps);
                        self.check_error_propagation(&errors);
                        *ret
                    }
                    Ty::Error => Ty::Error,
                    _ => {
                        self.err(
                            ErrorCode::E0009,
                            format!("pipe target must be a function, got `{fn_ty}`"),
                        );
                        Ty::Error
                    }
                }
            }

            Expr::FieldAccess(expr, field) => {
                let ty = self.check_expr(expr);
                match &ty {
                    Ty::Named(name) | Ty::App(name, _) => {
                        if let Some(fields) = self.registry.structs.get(name).cloned() {
                            let (fields, _) = self.struct_fields_for_type(name, &fields, &ty);
                            if let Some((_, fty)) = fields.iter().find(|(n, _)| n == field) {
                                fty.clone()
                            } else {
                                self.err(
                                    ErrorCode::E0015,
                                    format!("struct `{name}` has no field `{field}`"),
                                );
                                Ty::Error
                            }
                        } else {
                            self.err(ErrorCode::E0016, format!("type `{name}` has no fields"));
                            Ty::Error
                        }
                    }
                    Ty::Error => Ty::Error,
                    _ => {
                        self.err(
                            ErrorCode::E0016,
                            format!("cannot access field `{field}` on type `{ty}`"),
                        );
                        Ty::Error
                    }
                }
            }

            Expr::StructLit(name, fields) => {
                if let Some(def_fields) = self.registry.structs.get(name).cloned() {
                    let (def_fields, struct_ty) = self.instantiate_struct_fields(name, &def_fields);
                    // Check for duplicate fields in the literal
                    let mut seen = HashSet::new();
                    for (fname, _) in fields.iter() {
                        if !seen.insert(fname.as_str()) {
                            self.err(
                                ErrorCode::E0015,
                                format!("duplicate field `{fname}` in struct `{name}`"),
                            );
                        }
                    }

                    for (fname, fexpr) in fields {
                        let fty = self.check_expr(fexpr);
                        if let Some((_, expected)) = def_fields.iter().find(|(n, _)| n == fname) {
                            self.unify(expected, &fty, &format!("struct field `{fname}`"));
                        } else {
                            self.err(
                                ErrorCode::E0015,
                                format!("struct `{name}` has no field `{fname}`"),
                            );
                        }
                    }

                    // Check for missing required fields
                    let provided_names: HashSet<&str> =
                        fields.iter().map(|(n, _)| n.as_str()).collect();
                    for (def_name, _) in &def_fields {
                        if !provided_names.contains(def_name.as_str()) {
                            self.err(
                                ErrorCode::E0015,
                                format!("missing field `{def_name}` in struct `{name}`"),
                            );
                        }
                    }

                    struct_ty
                } else {
                    self.err(ErrorCode::E0005, format!("undefined struct `{name}`"));
                    Ty::Error
                }
            }

            Expr::Try(expr) => self.check_expr(expr),

            Expr::Hole(name, ty_hint, allows, span) => {
                let hole_name = name
                    .clone()
                    .unwrap_or_else(|| self.fresh_unnamed_hole_name());
                let effective_allows = allows.clone().or_else(|| self.current_hole_allows.clone());
                let inferred_from_allows = effective_allows
                    .as_deref()
                    .and_then(|allowed| self.infer_hole_type_from_allows(allowed));
                let return_expected = self
                    .expected_return_type
                    .as_ref()
                    .map(|ret| self.apply_subst(ret));
                let (ty, type_inferred_from) = if let Some(te) = ty_hint {
                    (
                        self.resolve_type(te),
                        Some("hole type annotation".to_string()),
                    )
                } else if let Some(ret) = return_expected {
                    if matches!(ret, Ty::Var(_) | Ty::Hole(_)) {
                        if let Some(inferred) = inferred_from_allows {
                            (inferred, Some("`@allows[...]` candidates".to_string()))
                        } else {
                            (
                                ret,
                                Some(format!("return type of `{}`", self.current_function)),
                            )
                        }
                    } else {
                        (
                            ret,
                            Some(format!("return type of `{}`", self.current_function)),
                        )
                    }
                } else if let Some(inferred) = inferred_from_allows {
                    (inferred, Some("`@allows[...]` candidates".to_string()))
                } else {
                    (Ty::Hole(hole_name.clone()), None)
                };

                // Collect hole info for the report (v0.3)
                let bindings = self.env.all_bindings();
                let expected = self.apply_subst(&ty);
                let suggestions = self.find_suggestions(&expected, effective_allows.as_deref());

                // Build scored candidates from simple suggestions
                let candidates: Vec<crate::hole::CandidateScore> = suggestions
                    .into_iter()
                    .map(|s| crate::hole::CandidateScore {
                        name: s,
                        type_match: 1.0,
                        cost_fit: 0.5,
                        required_effects_fit: 1.0,
                        error_coverage: 0.5,
                    })
                    .collect();

                // Collect available effects and errors in scope
                let available_effects = self.current_effects.clone();
                let errors_to_handle: Vec<String> = self.current_errors.iter().cloned().collect();

                self.hole_report.holes.push(HoleInfo {
                    name: hole_name,
                    location: None,
                    span: *span,
                    expected_type: expected,
                    type_inferred_from,
                    function: self.current_function.clone(),
                    enclosing_signature: None,
                    bindings,
                    binding_dependencies: std::collections::BTreeMap::new(),
                    available_effects,
                    errors_to_handle,
                    cost_budget: None,
                    candidates,
                    dependent_holes: Vec::new(),
                    confidence: None,
                    error_clusters: Vec::new(),
                });

                ty
            }

            Expr::Spawn(expr) => {
                if !self.current_effects.contains("Spawn") {
                    self.err(
                        ErrorCode::C0001,
                        "spawn requires effect `Spawn`; add `uses [Spawn]`".to_string(),
                    );
                }
                if !self.concurrency.in_parallel_scope(&self.current_function) {
                    self.err(
                        ErrorCode::C0103,
                        "spawn is only allowed inside `parallel_scope { ... }`".to_string(),
                    );
                }
                let inner = self.check_expr(expr);
                self.concurrency.record_spawn(
                    &self.current_function,
                    &inner.to_string(),
                    self.current_effects.iter().cloned().collect(),
                );
                Ty::App("Task".into(), vec![inner])
            }

            Expr::Await(expr) => {
                let ty = self.check_expr(expr);
                let ty = self.apply_subst(&ty);
                match ty {
                    Ty::App(ref name, ref args) if name == "Task" && args.len() == 1 => {
                        args[0].clone()
                    }
                    Ty::Error => Ty::Error,
                    _ => {
                        self.err(
                            ErrorCode::E0001,
                            format!("await expects Task[T], got `{ty}`"),
                        );
                        Ty::Error
                    }
                }
            }

            Expr::ChannelNew { elem_type, buffer } => {
                let buffer_ty = self.check_expr(buffer);
                self.unify(&Ty::I32, &buffer_ty, "Channel.new buffer");
                let elem_ty = self.resolve_type(elem_type);
                Ty::Tuple(vec![
                    Ty::App("Sender".into(), vec![elem_ty.clone()]),
                    Ty::App("Receiver".into(), vec![elem_ty]),
                ])
            }

            Expr::Return(expr) => {
                if let Some(inner) = expr {
                    let ret_val_ty = self.check_expr(inner);
                    if let Some(expected) = self.expected_return_type.clone() {
                        self.unify(&expected, &ret_val_ty, "return");
                    }
                }
                Ty::Never
            }

            Expr::Throw(expr) => {
                let _ = self.check_expr(expr);
                self.check_throw_coverage(expr);
                Ty::Never
            }

            Expr::List(elems) => {
                if elems.is_empty() {
                    Ty::App("List".into(), vec![self.fresh_var()])
                } else {
                    let first_ty = self.check_expr(&elems[0]);
                    for elem in &elems[1..] {
                        let elem_ty = self.check_expr(elem);
                        self.unify(&first_ty, &elem_ty, "list elements");
                    }
                    Ty::App("List".into(), vec![first_ty])
                }
            }

            Expr::ParallelScope { lanes, body } => {
                if let Some(lanes_expr) = lanes {
                    let lanes_ty = self.check_expr(lanes_expr);
                    if lanes_ty != Ty::I32 && !lanes_ty.is_error() {
                        self.err(
                            ErrorCode::E0002,
                            format!("parallel_scope lanes must be I32, got `{lanes_ty}`"),
                        );
                    }
                    if let Expr::IntLit(n) = lanes_expr.as_ref()
                        && *n <= 0
                    {
                        self.err(
                            ErrorCode::E0002,
                            format!("parallel_scope lanes must be positive, got `{n}`"),
                        );
                    }
                    if let Expr::IntLit(n) = lanes_expr.as_ref() {
                        let spawn_sites = Self::count_spawns(body);
                        if spawn_sites > *n as usize {
                            self.err(
                            ErrorCode::C0103,
                            format!(
                                "parallel_scope(lanes: {n}) has {spawn_sites} spawn site(s) in body"
                            ),
                        );
                        }
                    }
                }
                self.concurrency
                    .enter_parallel_scope(&self.current_function);
                let body_ty = self.check_expr(body);
                self.concurrency
                    .leave_parallel_scope(&self.current_function);
                body_ty
            }

            Expr::Select(arms) => {
                let mut result_ty: Option<Ty> = None;
                for arm in arms {
                    let arm_ty = match arm {
                        SelectArm::Recv {
                            binding,
                            source,
                            body,
                        } => {
                            let source_raw_ty = self.check_expr(source);
                            let source_ty = self.apply_subst(&source_raw_ty);
                            self.env.push_scope();
                            let binding_ty = match source_ty {
                                Ty::App(ref name, ref args)
                                    if name == "Receiver" && args.len() == 1 =>
                                {
                                    args[0].clone()
                                }
                                Ty::Error => Ty::Error,
                                other => {
                                    self.err(
                                        ErrorCode::E0001,
                                        format!("select source must be Receiver[T], got `{other}`"),
                                    );
                                    Ty::Error
                                }
                            };
                            self.env.define(binding.clone(), binding_ty);
                            let arm_ty = self.check_expr(body);
                            self.env.pop_scope();
                            arm_ty
                        }
                        SelectArm::Timeout { duration, body } => {
                            let duration_ty = self.check_expr(duration);
                            self.unify(&Ty::I32, &duration_ty, "select timeout");
                            self.check_expr(body)
                        }
                    };
                    if let Some(ref expected) = result_ty {
                        self.unify(expected, &arm_ty, "select arms");
                    } else {
                        result_ty = Some(arm_ty);
                    }
                }
                result_ty.unwrap_or(Ty::Unit)
            }

            Expr::Placeholder => {
                unreachable!(
                    "`_` placeholder should have been desugared into a lambda by the parser"
                )
            }

            Expr::Perform {
                effect,
                operation,
                args,
            } => {
                // Verify the required effect is in the current function's uses set.
                if !self.current_effects.contains(effect) {
                    self.err(
                        ErrorCode::C0001,
                        format!(
                            "perform requires effect `{effect}` but current function does not declare it"
                        ),
                    );
                }
                if !self.registry.interfaces.contains_key(effect) {
                    for arg in args {
                        let _ = self.check_expr(arg);
                    }
                    self.err(ErrorCode::C0002, format!("unknown effect `{effect}`"));
                    return Ty::Error;
                }
                if let Some((param_tys, ret_ty)) =
                    self.lookup_registered_effect_operation(effect, operation)
                {
                    if param_tys.len() != args.len() {
                        self.err(
                        ErrorCode::E0007,
                        format!(
                            "effect operation `{effect}.{operation}` expects {} arguments, got {}",
                            param_tys.len(),
                            args.len()
                        ),
                    );
                        for arg in args {
                            let _ = self.check_expr(arg);
                        }
                        return self.apply_subst(&ret_ty);
                    }
                    for (i, (expected, arg_expr)) in param_tys.iter().zip(args).enumerate() {
                        let arg_ty = self.check_expr(arg_expr);
                        self.unify(
                            expected,
                            &arg_ty,
                            &format!("argument {} of `{effect}.{operation}`", i + 1),
                        );
                    }
                    return self.apply_subst(&ret_ty);
                }
                for arg in args {
                    let _ = self.check_expr(arg);
                }
                Ty::Error
            }

            Expr::Handle { body, handlers } => {
                let mut provided_effects = EffectSet::new();
                let mut seen_operations: HashSet<(String, String)> = HashSet::new();

                for binding in handlers {
                    match binding {
                        HandleBinding::On(arm) => {
                            if !self.registry.interfaces.contains_key(&arm.effect) {
                                self.err(
                                    ErrorCode::C0002,
                                    format!("unknown effect `{}`", arm.effect),
                                );
                                continue;
                            }
                            provided_effects.insert(arm.effect.clone());
                            let key = (arm.effect.clone(), arm.operation.clone());
                            if !seen_operations.insert(key.clone()) {
                                self.err(
                                    ErrorCode::E0014,
                                    format!(
                                        "duplicate handler binding for `{}.{}` in one `with` block",
                                        key.0, key.1
                                    ),
                                );
                            }
                        }
                        HandleBinding::Use(handler_use) => {
                            let Some(info) =
                                self.registry.handlers.get(&handler_use.handler).cloned()
                            else {
                                self.err(
                                    ErrorCode::C0002,
                                    format!("unknown handler `{}`", handler_use.handler),
                                );
                                continue;
                            };

                            provided_effects.insert(info.effect.clone());
                            for (operation, _, _) in &info.methods {
                                let key = (info.effect.clone(), operation.clone());
                                if !seen_operations.insert(key.clone()) {
                                    self.err(
                                    ErrorCode::E0014,
                                    format!(
                                        "duplicate handler binding for `{}.{}` in one `with` block",
                                        key.0, key.1
                                    ),
                                );
                                }
                            }
                        }
                    }
                }

                for binding in handlers {
                    if let HandleBinding::Use(handler_use) = binding {
                        let Some(info) = self.registry.handlers.get(&handler_use.handler).cloned()
                        else {
                            continue;
                        };

                        let mut seen_fields = HashSet::new();
                        for (field_name, value_expr) in &handler_use.payload {
                            if !seen_fields.insert(field_name.clone()) {
                                self.err(
                                    ErrorCode::E0015,
                                    format!(
                                        "duplicate payload field `{field_name}` in handler `{}`",
                                        handler_use.handler
                                    ),
                                );
                            }

                            let value_ty = self.check_expr(value_expr);
                            if let Some((_, expected_ty)) =
                                info.fields.iter().find(|(name, _)| name == field_name)
                            {
                                self.unify(
                                    expected_ty,
                                    &value_ty,
                                    &format!(
                                        "payload field `{field_name}` for handler `{}`",
                                        handler_use.handler
                                    ),
                                );
                            } else {
                                self.err(
                                    ErrorCode::E0015,
                                    format!(
                                        "handler `{}` has no payload field `{field_name}`",
                                        handler_use.handler
                                    ),
                                );
                            }
                        }

                        for (field_name, _) in &info.fields {
                            if !handler_use
                                .payload
                                .iter()
                                .any(|(name, _)| name == field_name)
                            {
                                self.err(
                                    ErrorCode::E0101,
                                    format!(
                                        "handler `{}` is missing payload field `{field_name}`",
                                        handler_use.handler
                                    ),
                                );
                            }
                        }
                    }
                }

                let prev_effects = self.current_effects.clone();
                self.current_effects = self.current_effects.union(&provided_effects);

                let body_ty = self.check_expr(body);

                for binding in handlers {
                    let HandleBinding::On(arm) = binding else {
                        continue;
                    };

                    self.env.push_scope();
                    if self.registry.interfaces.contains_key(&arm.effect) {
                        if let Some((param_tys, ret_ty)) =
                            self.lookup_registered_effect_operation(&arm.effect, &arm.operation)
                        {
                            if param_tys.len() != arm.params.len() {
                                self.err(
                                    ErrorCode::E0007,
                                    format!(
                                        "handler arm `{}.{}` expects {} parameters, got {}",
                                        arm.effect,
                                        arm.operation,
                                        param_tys.len(),
                                        arm.params.len()
                                    ),
                                );
                            }

                            for (param, expected_ty) in arm.params.iter().zip(param_tys.iter()) {
                                self.env.define(param.clone(), expected_ty.clone());
                            }
                            for param in arm.params.iter().skip(param_tys.len()) {
                                let var = self.fresh_var();
                                self.env.define(param.clone(), var);
                            }

                            let arm_ty = self.check_expr(&arm.body);
                            self.unify(
                                &ret_ty,
                                &arm_ty,
                                &format!("handler arm `{}.{}`", arm.effect, arm.operation),
                            );
                        } else {
                            for param in &arm.params {
                                let var = self.fresh_var();
                                self.env.define(param.clone(), var);
                            }
                            let _ = self.check_expr(&arm.body);
                        }
                    } else {
                        for param in &arm.params {
                            let var = self.fresh_var();
                            self.env.define(param.clone(), var);
                        }
                        let _ = self.check_expr(&arm.body);
                    }
                    self.env.pop_scope();
                }

                self.current_effects = prev_effects;
                body_ty
            }
        }
    }

    // ── Binary operators ────────────────────────────────────────────

    pub(super) fn check_binop(&mut self, lhs: &Expr, op: &BinOp, rhs: &Expr) -> Ty {
        let lt = self.check_expr(lhs);
        let rt = self.check_expr(rhs);

        if lt.is_error() || rt.is_error() {
            return Ty::Error;
        }

        match op {
            // Arithmetic: both operands must be same numeric type
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                // String concatenation with +
                if matches!(op, BinOp::Add) && lt == Ty::Str && rt == Ty::Str {
                    return Ty::Str;
                }
                if !lt.is_numeric() {
                    self.err(
                        ErrorCode::E0002,
                        format!("cannot apply `{op:?}` to type `{lt}`"),
                    );
                    return Ty::Error;
                }
                self.unify(&lt, &rt, "arithmetic operands");
                lt
            }
            // Comparison: both operands same type, returns Bool
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                self.unify(&lt, &rt, "comparison operands");
                Ty::Bool
            }
            // Logical: both Bool
            BinOp::And | BinOp::Or => {
                if lt != Ty::Bool {
                    self.err(
                        ErrorCode::E0002,
                        format!("logical `{op:?}` expects Bool, got `{lt}`"),
                    );
                }
                if rt != Ty::Bool {
                    self.err(
                        ErrorCode::E0002,
                        format!("logical `{op:?}` expects Bool, got `{rt}`"),
                    );
                }
                Ty::Bool
            }
            // Bitwise: both integer
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                if !lt.is_integer() && !lt.is_error() {
                    self.err(
                        ErrorCode::E0002,
                        format!("bitwise `{op:?}` expects integer type, got `{lt}`"),
                    );
                }
                if !rt.is_integer() && !rt.is_error() {
                    self.err(
                        ErrorCode::E0002,
                        format!("bitwise `{op:?}` expects integer type, got `{rt}`"),
                    );
                }
                lt
            }
        }
    }

    pub(super) fn count_spawns(expr: &Expr) -> usize {
        match expr {
            Expr::Spawn(inner) => 1 + Self::count_spawns(inner),
            Expr::Call(callee, args) => {
                Self::count_spawns(callee) + args.iter().map(Self::count_spawns).sum::<usize>()
            }
            Expr::Lambda(_, body)
            | Expr::Await(body)
            | Expr::Try(body)
            | Expr::Return(Some(body))
            | Expr::Throw(body)
            | Expr::UnaryOp(_, body) => Self::count_spawns(body),
            Expr::BinOp(lhs, _, rhs) | Expr::Pipe(lhs, rhs) => {
                Self::count_spawns(lhs) + Self::count_spawns(rhs)
            }
            Expr::FieldAccess(base, _) => Self::count_spawns(base),
            Expr::If(cond, then_branch, else_branch) => {
                Self::count_spawns(cond)
                    + Self::count_spawns(then_branch)
                    + else_branch
                        .as_ref()
                        .map_or(0, |else_expr| Self::count_spawns(else_expr))
            }
            Expr::Match(scrutinee, arms) => {
                Self::count_spawns(scrutinee)
                    + arms
                        .iter()
                        .map(|arm| {
                            arm.guard.as_ref().map_or(0, Self::count_spawns)
                                + Self::count_spawns(&arm.body)
                        })
                        .sum::<usize>()
            }
            Expr::Block(stmts, tail) => {
                let stmt_spawns: usize = stmts
                    .iter()
                    .map(|stmt| match stmt {
                        Stmt::Let(_, _, value) => Self::count_spawns(value),
                        Stmt::Expr(e) => Self::count_spawns(e),
                    })
                    .sum();
                stmt_spawns + tail.as_ref().map_or(0, |e| Self::count_spawns(e))
            }
            Expr::Hole(_, _, _, _)
            | Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::StrLit(_)
            | Expr::BoolLit(_)
            | Expr::Var(_)
            | Expr::Placeholder => 0,
            Expr::StructLit(_, fields) => fields.iter().map(|(_, e)| Self::count_spawns(e)).sum(),
            Expr::List(elems) => elems.iter().map(Self::count_spawns).sum(),
            Expr::TString(parts) => parts
                .iter()
                .map(|part| match part {
                    TStringPart::Literal(_) => 0,
                    TStringPart::Expr(e) => Self::count_spawns(e),
                })
                .sum(),
            Expr::FString(parts) => parts
                .iter()
                .map(|part| match part {
                    FStringPart::Literal(_) => 0,
                    FStringPart::Expr(e) => Self::count_spawns(e),
                })
                .sum(),
            Expr::ParallelScope { body, .. } => Self::count_spawns(body),
            Expr::Select(arms) => arms
                .iter()
                .map(|arm| match arm {
                    SelectArm::Recv { source, body, .. } => {
                        Self::count_spawns(source) + Self::count_spawns(body)
                    }
                    SelectArm::Timeout { duration, body } => {
                        Self::count_spawns(duration) + Self::count_spawns(body)
                    }
                })
                .sum(),
            Expr::Handle { body, handlers } => {
                Self::count_spawns(body)
                    + handlers
                        .iter()
                        .map(|binding| match binding {
                            HandleBinding::Use(handler_use) => handler_use
                                .payload
                                .iter()
                                .map(|(_, expr)| Self::count_spawns(expr))
                                .sum(),
                            HandleBinding::On(arm) => Self::count_spawns(&arm.body),
                        })
                        .sum::<usize>()
            }
            Expr::Perform { args, .. } => args.iter().map(|arg| Self::count_spawns(arg)).sum(),
            Expr::ChannelNew { buffer, .. } => Self::count_spawns(buffer),
            Expr::Return(None) => 0,
        }
    }

    // ── Function calls ──────────────────────────────────────────────

    pub(super) fn check_call(&mut self, callee: &Expr, args: &[Expr]) -> Ty {
        // Direct call by name: `foo(args)`
        if let Expr::Var(name) = callee
            && let Some((param_tys, ret_ty, callee_caps)) =
                self.registry.functions.get(name).cloned()
        {
            // Instantiate generic functions with fresh type variables
            let mut type_mapping = HashMap::new();
            let (param_tys, ret_ty) = match self.registry.fn_type_params.get(name).cloned() {
                Some(ref tp) if !tp.is_empty() => {
                    let (inst_params, inst_ret, mapping) =
                        self.instantiate_sig(tp, &param_tys, &ret_ty);
                    type_mapping = mapping;
                    (inst_params, inst_ret)
                }
                _ => (param_tys, ret_ty),
            };

            if param_tys.len() != args.len() {
                self.err(
                    ErrorCode::E0007,
                    format!(
                        "function `{name}` expects {} arguments, got {}",
                        param_tys.len(),
                        args.len()
                    ),
                );
                return self.apply_subst(&ret_ty);
            }
            for (i, (expected, arg_expr)) in param_tys.iter().zip(args).enumerate() {
                let arg_ty = self.check_expr(arg_expr);
                self.unify(
                    expected,
                    &arg_ty,
                    &format!("argument {} of `{name}`", i + 1),
                );
            }
            self.check_where_bounds(name, &type_mapping);
            self.check_effect_propagation(&callee_caps);
            if let Some(callee_errors) = self.registry.fn_errors.get(name).cloned() {
                self.check_error_propagation(&callee_errors);
            }
            return self.apply_subst(&ret_ty);
        }

        // Direct call by name: check module registry (prelude builtins)
        if let Expr::Var(name) = callee
            && let Some((param_tys, ret_ty, caps)) = self.lookup_module_function(name)
        {
            if param_tys.len() != args.len() {
                self.err(
                    ErrorCode::E0007,
                    format!(
                        "function `{name}` expects {} arguments, got {}",
                        param_tys.len(),
                        args.len()
                    ),
                );
                return ret_ty;
            }
            for (i, (expected, arg_expr)) in param_tys.iter().zip(args).enumerate() {
                let arg_ty = self.check_expr(arg_expr);
                self.unify(
                    expected,
                    &arg_ty,
                    &format!("argument {} of `{name}`", i + 1),
                );
            }
            self.check_effect_propagation(&caps);
            return ret_ty;
        }
        // Could be a variable holding a function

        // Method call: `obj.method(args)` — callee is FieldAccess
        // General case: check callee type
        let fn_ty = self.check_expr(callee);
        match fn_ty {
            Ty::Fn(param_tys, ret_ty, caps, errors) => {
                if param_tys.len() != args.len() {
                    self.err(
                        ErrorCode::E0007,
                        format!(
                            "function expects {} arguments, got {}",
                            param_tys.len(),
                            args.len()
                        ),
                    );
                } else {
                    for (i, (expected, arg_expr)) in param_tys.iter().zip(args).enumerate() {
                        let arg_ty = self.check_expr(arg_expr);
                        self.unify(expected, &arg_ty, &format!("argument {}", i + 1));
                    }
                }
                self.check_effect_propagation(&caps);
                self.check_error_propagation(&errors);
                *ret_ty
            }
            Ty::Error => Ty::Error,
            _ => {
                self.err(
                    ErrorCode::E0008,
                    format!("cannot call non-function type `{fn_ty}`"),
                );
                Ty::Error
            }
        }
    }

    // ── Statements ──────────────────────────────────────────────────

    pub(super) fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(name, ty_ann, init) => {
                let init_ty = self.check_expr(init);
                let ty = if let Some(te) = ty_ann {
                    let declared = self.resolve_type(te);
                    self.unify(&declared, &init_ty, &format!("let binding `{name}`"));
                    // Check refinement predicate on constant initializers
                    if let Ty::Refined(_, ref var_name, ref pred) = declared {
                        self.check_refinement_on_expr(init, var_name, pred, name);
                    }
                    declared
                } else {
                    init_ty
                };
                self.env.define(name.clone(), ty);
            }
            Stmt::Expr(expr) => {
                let _ = self.check_expr(expr);
            }
        }
    }

    // ── Module registry lookup ──────────────────────────────────────

    /// Look up an operation on an explicitly declared `effect`.
    ///
    /// This does not try to infer signatures for platform-provided runtime
    /// handlers whose effect interfaces have not been loaded as source yet.
    pub(super) fn lookup_registered_effect_operation(
        &mut self,
        effect: &str,
        operation: &str,
    ) -> Option<(Vec<Ty>, Ty)> {
        let (type_params, methods) = self.registry.interfaces.get(effect).cloned()?;
        let Some((_name, param_tys, ret_ty)) =
            methods.into_iter().find(|(name, _, _)| name == operation)
        else {
            self.err(
                ErrorCode::C0002,
                format!("effect `{effect}` has no operation `{operation}`"),
            );
            return None;
        };

        if type_params.is_empty() {
            Some((param_tys, ret_ty))
        } else {
            let (params, ret, _) = self.instantiate_sig(&type_params, &param_tys, &ret_ty);
            Some((params, ret))
        }
    }

    /// Look up a function in the module registry (e.g. prelude builtins).
    /// Instantiates fresh type variables for each `Ty::Var` in the signature
    /// to avoid collisions with the checker's own variable counter.
    pub(super) fn lookup_module_function(
        &mut self,
        name: &str,
    ) -> Option<(Vec<Ty>, Ty, EffectSet)> {
        // First pass: find the signature (immutable borrow of module_registry)
        let found = self.module_registry.all_interfaces().find_map(|module| {
            module.functions.get(name).map(|(params, ret)| {
                (
                    params.clone(),
                    ret.clone(),
                    module
                        .function_required_effects
                        .get(name)
                        .cloned()
                        .unwrap_or_default(),
                )
            })
        });

        let (params, ret, caps) = found?;

        // Collect all Var IDs used in the signature
        let mut var_ids = std::collections::BTreeSet::new();
        for p in &params {
            Self::collect_vars(p, &mut var_ids);
        }
        Self::collect_vars(&ret, &mut var_ids);
        if var_ids.is_empty() {
            return Some((params, ret, caps));
        }
        // Map old IDs → fresh variables (mutable borrow of self)
        let mapping: std::collections::BTreeMap<u32, Ty> = var_ids
            .into_iter()
            .map(|id| (id, self.fresh_var()))
            .collect();
        let params = params
            .iter()
            .map(|t| Self::replace_vars(t, &mapping))
            .collect();
        let ret = Self::replace_vars(&ret, &mapping);
        Some((params, ret, caps))
    }

    /// Collect all `Ty::Var` IDs from a type.
    pub(super) fn collect_vars(ty: &Ty, ids: &mut std::collections::BTreeSet<u32>) {
        ty.visit(&mut |t| {
            if let Ty::Var(id) = t {
                ids.insert(*id);
            }
        });
    }

    /// Replace `Ty::Var` IDs according to a mapping.
    pub(super) fn replace_vars(ty: &Ty, mapping: &std::collections::BTreeMap<u32, Ty>) -> Ty {
        ty.fold_ref(&mut |t| match t {
            Ty::Var(id) => Some(mapping.get(id).cloned().unwrap_or_else(|| t.clone())),
            _ => None,
        })
    }
}
