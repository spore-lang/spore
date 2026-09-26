use super::*;

impl Checker {
    pub(super) fn check_expr(&mut self, expr: &Expr) -> Ty {
        match expr {
            Expr::IntLit(_) => Ty::I64,
            Expr::SuffixedIntLit(n, suffix) => self.check_suffixed_int_literal(*n, suffix),
            Expr::FloatLit(_) => Ty::F64,
            Expr::StrLit(_) => Ty::Str,
            Expr::BoolLit(_) => Ty::Bool,
            Expr::Unit => Ty::Unit,
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
                        Ty::Fn(params, Box::new(ret), caps)
                    }
                } else if let Some((params, ret, caps)) = self.lookup_module_function(name) {
                    Ty::Fn(params, Box::new(ret), caps)
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
                self.push_effect_observer();
                let ret_ty = self.check_expr(body);
                let caps = self.pop_effect_observer();
                self.env.pop_scope();
                Ty::Fn(param_tys, Box::new(ret_ty), caps)
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
                    self.merge_branch_types(then_ty, else_ty, "if/else branches")
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

                    if let Some(expected) = result_ty.take() {
                        result_ty = Some(self.merge_branch_types(expected, arm_ty, "match arms"));
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
                    Ty::Fn(params, ret, caps) => {
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
                self.check_field_access_on_type(&ty, field)
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

            Expr::Try(expr) => {
                let inner_ty = self.check_expr(expr);
                let ty = self.apply_subst(&inner_ty);
                match ty {
                    Ty::Outcome(success, failure) => {
                        if let Some(enclosing_failure) = self.current_outcome_failure.clone() {
                            self.unify(
                                &enclosing_failure,
                                &failure,
                                "outcome propagation with `?`",
                            );
                        } else {
                            self.err(
                                ErrorCode::E0012,
                                "`?` requires an enclosing outcome return type such as `A ! E`"
                                    .into(),
                            );
                        }
                        *success
                    }
                    Ty::Error => Ty::Error,
                    other => {
                        self.err(
                            ErrorCode::E0012,
                            format!("`?` expects an outcome value, got `{other}`"),
                        );
                        Ty::Error
                    }
                }
            }

            Expr::Hole(name, ty_hint, span) => {
                let hole_name = name
                    .clone()
                    .unwrap_or_else(|| self.fresh_unnamed_hole_name());
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
                    (
                        ret,
                        Some(format!("return type of `{}`", self.current_function)),
                    )
                } else {
                    (Ty::Hole(hole_name.clone()), None)
                };

                // Collect hole info for the report (v0.3)
                let bindings = self.env.all_bindings();
                let expected = self.apply_subst(&ty);
                let suggestions = self.find_suggestions(&expected);

                // Build scored candidates from simple suggestions
                let candidates: Vec<crate::hole::CandidateScore> = suggestions
                    .into_iter()
                    .map(|name| {
                        let missing_effects = self
                            .registry
                            .functions
                            .get(&name)
                            .map(|(_, _, effects)| {
                                effects
                                    .iter()
                                    .filter(|effect| !self.current_effects.contains(effect))
                                    .cloned()
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();
                        let required_effects_fit =
                            if missing_effects.is_empty() { 1.0 } else { 0.0 };
                        let mut rejection_reasons = Vec::new();
                        if !missing_effects.is_empty() {
                            rejection_reasons
                                .push(format!("requires effects [{}]", missing_effects.join(", ")));
                        }

                        crate::hole::CandidateScore {
                            name,
                            type_match: 1.0,
                            budget_fit: 0.5,
                            required_effects_fit,
                            error_coverage: 1.0,
                            rejection_reasons: rejection_reasons.clone(),
                            explanation: rejection_reasons.first().cloned(),
                            adjustments: rejection_reasons,
                        }
                    })
                    .collect();

                // Collect available effects and the enclosing outcome failure type.
                let available_effects = self.current_effects.clone();
                let errors_to_handle = self
                    .current_outcome_failure
                    .as_ref()
                    .map(|failure| vec![failure.to_string()])
                    .unwrap_or_default();
                let effect_context = self.hole_effect_context_stack.last().map(|context| {
                    crate::hole::EffectContext {
                        discharged_effects: context.discharged_effects.clone(),
                        surviving_effects: context.surviving_effects.clone(),
                    }
                });

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
                    effect_context,
                    budget_context: None,
                    property_context: None,
                    candidates,
                    dependent_holes: Vec::new(),
                    confidence: None,
                    error_clusters: Vec::new(),
                });

                ty
            }

            Expr::Spawn(expr) => {
                self.observe_effect("Spawn");
                if !self.current_effects.contains("Spawn") {
                    self.err(
                        ErrorCode::F0001,
                        "spawn requires effect `Spawn`; add `uses [Spawn]`".to_string(),
                    );
                }
                if !self.concurrency.in_parallel_scope(&self.current_function) {
                    self.err(
                        ErrorCode::F0103,
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
                let _ = self.check_expr_against(&Ty::I64, buffer, "Channel.new buffer");
                let elem_ty = self.resolve_type(elem_type);
                Ty::Tuple(vec![
                    Ty::App("Sender".into(), vec![elem_ty.clone()]),
                    Ty::App("Receiver".into(), vec![elem_ty]),
                ])
            }

            Expr::Return(expr) => {
                if let Some(inner) = expr {
                    if let Some(expected) = self.expected_return_type.clone() {
                        let _ = self.check_expr_against(&expected, inner, "return");
                    } else {
                        let _ = self.check_expr(inner);
                    }
                }
                Ty::Never
            }

            Expr::Fail(expr) => {
                let failure = self.check_expr(expr);
                Ty::Outcome(Box::new(self.fresh_var()), Box::new(failure))
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
                    if lanes_ty != Ty::I64 && !lanes_ty.is_error() {
                        self.err(
                            ErrorCode::E0002,
                            format!("parallel_scope lanes must be I64, got `{lanes_ty}`"),
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
                            ErrorCode::F0103,
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
                            let _ = self.check_expr_against(&Ty::I64, duration, "select timeout");
                            self.check_expr(body)
                        }
                    };
                    if let Some(expected) = result_ty.take() {
                        result_ty = Some(self.merge_branch_types(expected, arm_ty, "select arms"));
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
                self.observe_effect(effect.clone());
                // Verify the required effect is in the current function's uses set.
                if !self.current_effects.contains(effect) {
                    self.err(
                        ErrorCode::F0001,
                        format!(
                            "perform requires effect `{effect}` but current function does not declare it"
                        ),
                    );
                }
                if !self.registry.effects.contains(effect) {
                    for arg in args {
                        let _ = self.check_expr(arg);
                    }
                    self.err(ErrorCode::F0002, format!("unknown effect `{effect}`"));
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
                        let _ = self.check_expr_against(
                            expected,
                            arg_expr,
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
                let mut handled_effects = EffectSet::new();
                let mut seen_operations: HashSet<(String, String)> = HashSet::new();
                let mut named_handler_effects = EffectSet::new();
                let mut inline_effects: HashMap<String, HashSet<String>> = HashMap::new();

                for binding in handlers {
                    match binding {
                        HandleBinding::On(arm) => {
                            if !self.registry.effects.contains(&arm.effect) {
                                self.err(
                                    ErrorCode::F0002,
                                    format!("unknown effect `{}`", arm.effect),
                                );
                                continue;
                            }
                            handled_effects.insert(arm.effect.clone());
                            inline_effects
                                .entry(arm.effect.clone())
                                .or_default()
                                .insert(arm.operation.clone());
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
                                    ErrorCode::F0002,
                                    format!("unknown handler `{}`", handler_use.handler),
                                );
                                continue;
                            };

                            handled_effects = handled_effects.union(&info.handled_effects);
                            named_handler_effects = named_handler_effects.union(&info.uses_effects);
                            for (effect, methods) in &info.methods {
                                for (operation, _, _) in methods {
                                    let key = (effect.clone(), operation.clone());
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
                }

                for (effect, operations) in &inline_effects {
                    if let Some((_type_params, expected_operations)) =
                        self.registry.interfaces.get(effect).cloned()
                    {
                        for (operation, _, _) in &expected_operations {
                            if !operations.contains(operation) {
                                self.err(
                                    ErrorCode::E0013,
                                    format!(
                                        "handle block is missing handler arm `{}.{}`",
                                        effect, operation
                                    ),
                                );
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
                self.current_effects = prev_effects.union(&handled_effects);
                let enclosing_effect_context = if let Some(parent) =
                    self.hole_effect_context_stack.last()
                {
                    super::EnclosingHandlerEffectContext {
                        surviving_effects: parent.surviving_effects.difference(&handled_effects),
                        discharged_effects: parent.discharged_effects.union(&handled_effects),
                    }
                } else {
                    super::EnclosingHandlerEffectContext {
                        surviving_effects: prev_effects.difference(&handled_effects),
                        discharged_effects: handled_effects.clone(),
                    }
                };
                self.hole_effect_context_stack
                    .push(enclosing_effect_context);
                self.push_effect_observer();
                let body_ty = self.check_expr(body);
                let body_effects = self.pop_effect_observer();
                self.hole_effect_context_stack.pop();
                let mut handler_effects = named_handler_effects;

                self.current_effects = prev_effects.clone();

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

                            self.push_effect_observer();
                            let arm_ty = self.check_expr(&arm.body);
                            let arm_effects = self.pop_effect_observer();
                            handler_effects = handler_effects.union(&arm_effects);
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
                            self.push_effect_observer();
                            let _ = self.check_expr(&arm.body);
                            let arm_effects = self.pop_effect_observer();
                            handler_effects = handler_effects.union(&arm_effects);
                        }
                    } else {
                        for param in &arm.params {
                            let var = self.fresh_var();
                            self.env.define(param.clone(), var);
                        }
                        self.push_effect_observer();
                        let _ = self.check_expr(&arm.body);
                        let arm_effects = self.pop_effect_observer();
                        handler_effects = handler_effects.union(&arm_effects);
                    }
                    self.env.pop_scope();
                }

                let discharged_effects = body_effects
                    .difference(&handled_effects)
                    .union(&handler_effects);
                self.observe_effects(&discharged_effects);
                let leaked_outer_effects = discharged_effects.difference(&prev_effects);
                if !leaked_outer_effects.is_empty() {
                    self.err(
                        ErrorCode::F0001,
                        format!(
                            "handle block leaks outer effects {}; add them to the surrounding `uses [...]` or discharge them with another handler",
                            leaked_outer_effects
                        ),
                    );
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
                // Str concatenation with +
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
            | Expr::Fail(body)
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
            Expr::Hole(_, _, _)
            | Expr::IntLit(_)
            | Expr::SuffixedIntLit(_, _)
            | Expr::FloatLit(_)
            | Expr::StrLit(_)
            | Expr::BoolLit(_)
            | Expr::Unit
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

    pub(super) fn check_expr_against(&mut self, expected: &Ty, expr: &Expr, context: &str) -> Ty {
        let expected = self.apply_subst(expected);
        if let Expr::Block(stmts, tail) = expr {
            self.env.push_scope();
            for stmt in stmts {
                self.check_stmt(stmt);
            }
            let actual = if let Some(tail_expr) = tail {
                self.check_expr_against(&expected, tail_expr, context)
            } else {
                self.unify(&expected, &Ty::Unit, context);
                Ty::Unit
            };
            self.env.pop_scope();
            return actual;
        }

        if let Ty::Outcome(success, _) = &expected {
            if let Expr::IntLit(n) = expr
                && let Some(fits) = Self::integer_literal_fits(success, *n)
            {
                if fits {
                    return (**success).clone();
                }
                self.err(
                    ErrorCode::E0001,
                    format!("integer literal `{n}` does not fit `{success}` in {context}"),
                );
                return Ty::Error;
            }
            let actual = self.check_expr(expr);
            if matches!(actual, Ty::Outcome(_, _)) {
                self.unify(&expected, &actual, context);
            } else {
                self.unify(success, &actual, context);
            }
            return actual;
        }

        if let Expr::IntLit(n) = expr
            && let Some(fits) = Self::integer_literal_fits(&expected, *n)
        {
            if fits {
                return expected;
            }
            self.err(
                ErrorCode::E0001,
                format!("integer literal `{n}` does not fit `{expected}` in {context}"),
            );
            return Ty::Error;
        }

        let actual = self.check_expr(expr);
        self.unify(&expected, &actual, context);
        actual
    }

    fn merge_branch_types(&mut self, left: Ty, right: Ty, context: &str) -> Ty {
        let left = self.apply_subst(&left);
        let right = self.apply_subst(&right);
        match (&left, &right) {
            (Ty::Never, _) => right,
            (_, Ty::Never) => left,
            (Ty::Outcome(_, _), Ty::Outcome(_, _)) => {
                self.unify(&left, &right, context);
                left
            }
            (Ty::Outcome(success, _), _) => {
                self.unify(success, &right, context);
                left
            }
            (_, Ty::Outcome(success, _)) => {
                self.unify(&left, success, context);
                right
            }
            _ => {
                self.unify(&left, &right, context);
                left
            }
        }
    }

    fn integer_literal_fits(expected: &Ty, n: i64) -> Option<bool> {
        match expected.base_type() {
            Ty::I8 => Some(i8::try_from(n).is_ok()),
            Ty::I16 => Some(i16::try_from(n).is_ok()),
            Ty::I32 => Some(i32::try_from(n).is_ok()),
            Ty::I64 => Some(true),
            Ty::U8 => Some(u8::try_from(n).is_ok()),
            Ty::U16 => Some(u16::try_from(n).is_ok()),
            Ty::U32 => Some(u32::try_from(n).is_ok()),
            Ty::U64 => Some(n >= 0),
            _ => None,
        }
    }

    fn check_suffixed_int_literal(&mut self, n: i64, suffix: &str) -> Ty {
        let Some(ty) = Self::integer_suffix_ty(suffix) else {
            self.err(
                ErrorCode::E0001,
                format!("unknown integer literal suffix `{suffix}`"),
            );
            return Ty::Error;
        };
        if Self::integer_literal_fits(&ty, n).unwrap_or(false) {
            ty
        } else {
            self.err(
                ErrorCode::E0001,
                format!("integer literal `{n}{suffix}` does not fit `{ty}`"),
            );
            Ty::Error
        }
    }

    fn integer_suffix_ty(suffix: &str) -> Option<Ty> {
        match suffix {
            "i8" => Some(Ty::I8),
            "i16" => Some(Ty::I16),
            "i32" => Some(Ty::I32),
            "i64" => Some(Ty::I64),
            "u8" => Some(Ty::U8),
            "u16" => Some(Ty::U16),
            "u32" => Some(Ty::U32),
            "u64" => Some(Ty::U64),
            _ => None,
        }
    }

    // ── Function calls ──────────────────────────────────────────────

    fn check_field_access_on_type(&mut self, ty: &Ty, field: &str) -> Ty {
        match ty {
            Ty::Named(name) | Ty::App(name, _) => {
                if let Some(fields) = self.registry.structs.get(name).cloned() {
                    let (fields, _) = self.struct_fields_for_type(name, &fields, ty);
                    if let Some((_, fty)) = fields.iter().find(|(name, _)| name == field) {
                        fty.clone()
                    } else if name.starts_with("__handler::") {
                        let inferred = self.fresh_var();
                        self.registry
                            .structs
                            .get_mut(name)
                            .expect("handler self type must be registered")
                            .push((field.to_string(), inferred.clone()));
                        inferred
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

    fn check_instantiated_method_call(
        &mut self,
        method_name: &str,
        method: InstantiatedMethod,
        receiver_ty: Option<&Ty>,
        args: &[Expr],
    ) -> Ty {
        let explicit_params = if let Some(receiver_ty) = receiver_ty {
            let Some((receiver_param, explicit_params)) = method.params.split_first() else {
                self.err(
                    ErrorCode::E0007,
                    format!("method `{method_name}` is missing its receiver parameter"),
                );
                return Ty::Error;
            };
            self.unify(
                receiver_param,
                receiver_ty,
                &format!("receiver of method `{method_name}`"),
            );
            explicit_params
        } else {
            method.params.as_slice()
        };

        if explicit_params.len() != args.len() {
            self.err(
                ErrorCode::E0007,
                format!(
                    "method `{method_name}` expects {} arguments, got {}",
                    explicit_params.len(),
                    args.len()
                ),
            );
            return self.apply_subst(&method.return_type);
        }
        for (index, (expected, arg_expr)) in explicit_params.iter().zip(args).enumerate() {
            let _ = self.check_expr_against(
                expected,
                arg_expr,
                &format!("argument {} of method `{method_name}`", index + 1),
            );
        }
        self.check_instantiated_bounds(
            &format!("method `{method_name}`"),
            &method.generic_bounds,
            &method.type_mapping,
        );
        self.check_effect_propagation(&method.required_effects);
        self.apply_subst(&method.return_type)
    }

    fn check_function_type_call(&mut self, fn_ty: Ty, args: &[Expr]) -> Ty {
        match fn_ty {
            Ty::Fn(param_tys, ret_ty, caps) => {
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
                        let _ = self.check_expr_against(
                            expected,
                            arg_expr,
                            &format!("argument {}", i + 1),
                        );
                    }
                }
                self.check_effect_propagation(&caps);
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
                let _ = self.check_expr_against(
                    expected,
                    arg_expr,
                    &format!("argument {} of `{name}`", i + 1),
                );
            }
            self.check_generic_bounds(name, &type_mapping);
            self.check_effect_propagation(&callee_caps);
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
                let _ = self.check_expr_against(
                    expected,
                    arg_expr,
                    &format!("argument {} of `{name}`", i + 1),
                );
            }
            self.check_effect_propagation(&caps);
            return ret_ty;
        }
        // Could be a variable holding a function

        if let Expr::FieldAccess(receiver, method_name) = callee {
            if let Expr::Var(owner_name) = receiver.as_ref()
                && let Some(method) = self.lookup_static_method(owner_name, method_name)
            {
                return self.check_instantiated_method_call(method_name, method, None, args);
            }

            let receiver_ty = self.check_expr(receiver);
            if let Some(method) = self
                .lookup_receiver_method(&receiver_ty, method_name)
                .or_else(|| self.lookup_generic_bound_method(&receiver_ty, method_name))
            {
                return self.check_instantiated_method_call(
                    method_name,
                    method,
                    Some(&receiver_ty),
                    args,
                );
            }

            let fn_ty = self.check_field_access_on_type(&receiver_ty, method_name);
            return self.check_function_type_call(fn_ty, args);
        }

        let fn_ty = self.check_expr(callee);
        self.check_function_type_call(fn_ty, args)
    }

    // ── Statements ──────────────────────────────────────────────────

    pub(super) fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(name, ty_ann, init) => {
                let ty = if let Some(te) = ty_ann {
                    let declared = self.resolve_type(te);
                    let _ =
                        self.check_expr_against(&declared, init, &format!("let binding `{name}`"));
                    // Check refinement predicate on constant initializers
                    if let Ty::Refined(_, ref var_name, ref pred) = declared {
                        self.check_refinement_on_expr(init, var_name, pred, name);
                    }
                    declared
                } else {
                    self.check_expr(init)
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
        let Some((type_params, methods)) = self.registry.interfaces.get(effect).cloned() else {
            self.err(
                ErrorCode::F0002,
                format!("effect `{effect}` has no visible operation protocol"),
            );
            return None;
        };
        let Some((_name, param_tys, ret_ty)) =
            methods.into_iter().find(|(name, _, _)| name == operation)
        else {
            self.err(
                ErrorCode::F0002,
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
