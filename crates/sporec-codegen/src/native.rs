use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use cranelift_codegen::ir::{AbiParam, InstBuilder, condcodes::IntCC, types};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};
use sporec_parser::ast::{BinOp, Expr, FnDef, Item, Module as AstModule, Stmt, TypeExpr, UnaryOp};

use crate::value::Value;

const MAX_ENTRY_ARGS: usize = 8;

// Trap codes communicated from JIT code to Rust via a thread-local.
const TRAP_OVERFLOW: i64 = 1;
const TRAP_DIV_ZERO: i64 = 2;
const TRAP_MOD_ZERO: i64 = 3;
const TRAP_DIV_OVERFLOW: i64 = 4;

thread_local! {
    /// Set by JIT code when a recoverable arithmetic error occurs.
    static ARITH_TRAP: Cell<i64> = const { Cell::new(0) };
}

/// C-callable function that JIT code invokes to signal an arithmetic error.
/// The `code` is one of the `TRAP_*` constants above.
unsafe extern "C" fn spore_arith_trap(code: i64) {
    ARITH_TRAP.with(|cell| cell.set(code));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeType {
    I64,
    Bool,
    Unit,
}

impl NativeType {
    fn from_type_expr(ty: Option<&TypeExpr>) -> Result<Self, NativeError> {
        match ty {
            None => Ok(Self::Unit),
            Some(TypeExpr::Named(name)) => match name.as_str() {
                "I64" | "Int" => Ok(Self::I64),
                "Bool" => Ok(Self::Bool),
                "Unit" => Ok(Self::Unit),
                other => Err(NativeError::unsupported(format!(
                    "unsupported scalar type `{other}`"
                ))),
            },
            Some(TypeExpr::Tuple(items)) if items.is_empty() => Ok(Self::Unit),
            Some(other) => Err(NativeError::unsupported(format!(
                "unsupported scalar type `{other:?}`"
            ))),
        }
    }

    fn from_value(value: &Value) -> Result<Self, NativeError> {
        match value {
            Value::Int(_) => Ok(Self::I64),
            Value::Bool(_) => Ok(Self::Bool),
            Value::Unit => Ok(Self::Unit),
            other => Err(NativeError::unsupported(format!(
                "unsupported runtime value `{}`",
                other.type_name()
            ))),
        }
    }

    fn encode(self, value: &Value) -> Result<i64, NativeError> {
        match (self, value) {
            (Self::I64, Value::Int(v)) => Ok(*v),
            (Self::Bool, Value::Bool(v)) => Ok(i64::from(*v)),
            (Self::Unit, Value::Unit) => Ok(0),
            _ => Err(NativeError::new(format!(
                "expected {self}, got {}",
                value.type_name()
            ))),
        }
    }

    fn decode(self, value: i64) -> Value {
        match self {
            Self::I64 => Value::Int(value),
            Self::Bool => Value::Bool(value != 0),
            Self::Unit => Value::Unit,
        }
    }
}

impl fmt::Display for NativeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I64 => write!(f, "I64"),
            Self::Bool => write!(f, "Bool"),
            Self::Unit => write!(f, "()"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NativeError {
    message: String,
}

impl NativeError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn unsupported(reason: impl Into<String>) -> Self {
        Self::new(format!(
            "unsupported native backend feature: {}",
            reason.into()
        ))
    }

    fn for_function(function: &str, reason: impl Into<String>) -> Self {
        Self::unsupported(format!("function `{function}`: {}", reason.into()))
    }
}

impl fmt::Display for NativeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for NativeError {}

#[derive(Debug, Clone)]
struct FunctionSignature {
    params: Vec<NativeType>,
    return_ty: NativeType,
}

struct FunctionInfo<'a> {
    def: &'a FnDef,
    params: Vec<NativeType>,
    return_ty: NativeType,
    calls: BTreeSet<String>,
}

struct ModulePlan<'a> {
    functions: BTreeMap<String, FunctionInfo<'a>>,
}

#[derive(Clone, Copy)]
struct BoundValue {
    value: cranelift_codegen::ir::Value,
    ty: NativeType,
}

pub struct NativeProgram {
    _module: JITModule,
    function_ptrs: BTreeMap<String, *const u8>,
    signatures: BTreeMap<String, FunctionSignature>,
}

impl NativeProgram {
    pub fn call_function(&self, name: &str, args: Vec<Value>) -> Result<Value, NativeError> {
        let signature = self
            .signatures
            .get(name)
            .cloned()
            .ok_or_else(|| NativeError::new(format!("unknown function `{name}`")))?;
        if args.len() != signature.params.len() {
            return Err(NativeError::new(format!(
                "function `{name}` expects {} args, got {}",
                signature.params.len(),
                args.len()
            )));
        }
        let ptr = *self.function_ptrs.get(name).ok_or_else(|| {
            NativeError::new(format!("native function `{name}` was not finalized"))
        })?;
        let encoded_args = args
            .iter()
            .zip(signature.params.iter().copied())
            .map(|(value, expected)| {
                let actual = NativeType::from_value(value)?;
                if actual != expected {
                    return Err(NativeError::new(format!(
                        "function `{name}` expects `{expected}` arguments, got `{actual}`"
                    )));
                }
                expected.encode(value)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let raw = invoke_compiled_function(ptr, &encoded_args)?;
        Ok(signature.return_ty.decode(raw))
    }
}

pub fn compile_native(module: &AstModule) -> Result<NativeProgram, NativeError> {
    let plan = analyze_module(module)?;
    let isa_builder = cranelift_native::builder()
        .map_err(|error| NativeError::new(format!("failed to create host ISA builder: {error}")))?;
    let mut flags = settings::builder();
    flags.set("is_pic", "false").map_err(|error| {
        NativeError::new(format!("failed to configure Cranelift flags: {error}"))
    })?;
    let isa = isa_builder
        .finish(settings::Flags::new(flags))
        .map_err(|error| NativeError::new(format!("failed to finish host ISA: {error}")))?;
    let mut jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    // Register the Rust-side trap callback so JIT code can signal arithmetic errors.
    jit_builder.symbol("__spore_arith_trap", spore_arith_trap as *const u8);
    let mut jit = JITModule::new(jit_builder);

    // Declare the trap callback as an imported function.
    let mut trap_sig = jit.make_signature();
    trap_sig.params.push(AbiParam::new(types::I64));
    let trap_func_id = jit
        .declare_function("__spore_arith_trap", Linkage::Import, &trap_sig)
        .map_err(|error| NativeError::new(format!("failed to declare trap callback: {error}")))?;

    let mut func_ids = BTreeMap::new();
    for (name, info) in &plan.functions {
        let sig = native_signature(&jit, info.params.len());
        let func_id = jit
            .declare_function(name, Linkage::Local, &sig)
            .map_err(|error| NativeError::new(format!("failed to declare `{name}`: {error}")))?;
        func_ids.insert(name.clone(), func_id);
    }

    for (name, info) in &plan.functions {
        define_function(&mut jit, name, info, &plan, &func_ids, trap_func_id)?;
    }
    jit.finalize_definitions()
        .map_err(|error| NativeError::new(format!("failed to finalize native module: {error}")))?;

    let function_ptrs = func_ids
        .iter()
        .map(|(name, func_id)| (name.clone(), jit.get_finalized_function(*func_id)))
        .collect();
    let signatures = plan
        .functions
        .iter()
        .map(|(name, info)| {
            (
                name.clone(),
                FunctionSignature {
                    params: info.params.clone(),
                    return_ty: info.return_ty,
                },
            )
        })
        .collect();

    Ok(NativeProgram {
        _module: jit,
        function_ptrs,
        signatures,
    })
}

pub fn run_native(module: &AstModule) -> Result<Value, NativeError> {
    call_native(module, "main", vec![])
}

pub fn call_native(module: &AstModule, name: &str, args: Vec<Value>) -> Result<Value, NativeError> {
    compile_native(module)?.call_function(name, args)
}

fn analyze_module(module: &AstModule) -> Result<ModulePlan<'_>, NativeError> {
    let mut functions = BTreeMap::new();
    for item in &module.items {
        match item {
            Item::Function(def) => {
                if !def.type_params.is_empty() {
                    return Err(NativeError::for_function(
                        &def.name,
                        "generic functions are not supported",
                    ));
                }
                if def.where_clause.is_some() {
                    return Err(NativeError::for_function(
                        &def.name,
                        "`where` clauses are not supported",
                    ));
                }
                if def.is_foreign {
                    return Err(NativeError::for_function(
                        &def.name,
                        "`foreign fn` is not supported",
                    ));
                }
                if !def.errors.is_empty() {
                    return Err(NativeError::for_function(
                        &def.name,
                        "checked errors are not supported",
                    ));
                }
                if def
                    .uses_clause
                    .as_ref()
                    .is_some_and(|uses| !uses.resources.is_empty())
                {
                    return Err(NativeError::for_function(
                        &def.name,
                        "effectful `uses` clauses are not supported",
                    ));
                }
                let body = def.body.as_ref().ok_or_else(|| {
                    NativeError::for_function(&def.name, "hole-backed bodies are not supported")
                })?;
                let params = def
                    .params
                    .iter()
                    .map(|param| NativeType::from_type_expr(Some(&param.ty)))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| NativeError::for_function(&def.name, error.message))?;
                let return_ty = NativeType::from_type_expr(def.return_type.as_ref())
                    .map_err(|error| NativeError::for_function(&def.name, error.message))?;
                functions.insert(
                    def.name.clone(),
                    FunctionInfo {
                        def,
                        params,
                        return_ty,
                        calls: BTreeSet::new(),
                    },
                );
                let _ = body;
            }
            Item::Import(_) => {
                return Err(NativeError::unsupported(
                    "imports are not supported by the experimental scalar backend",
                ));
            }
            Item::Const(_) => {
                return Err(NativeError::unsupported(
                    "top-level constants are not supported by the experimental scalar backend",
                ));
            }
            Item::StructDef(_) | Item::TypeDef(_) | Item::Alias(_) => {
                return Err(NativeError::unsupported(
                    "aggregate and alias declarations are not supported by the experimental scalar backend",
                ));
            }
            _ => {
                return Err(NativeError::unsupported(
                    "non-function module items are not supported by the experimental scalar backend",
                ));
            }
        }
    }

    let signatures = functions
        .iter()
        .map(|(name, info)| (name.clone(), (info.params.clone(), info.return_ty)))
        .collect::<BTreeMap<_, _>>();

    for info in functions.values_mut() {
        let body =
            info.def.body.as_ref().ok_or_else(|| {
                NativeError::for_function(&info.def.name, "missing function body")
            })?;
        let mut scopes = vec![BTreeMap::new()];
        for (param, ty) in info.def.params.iter().zip(info.params.iter().copied()) {
            scopes.last_mut().unwrap().insert(param.name.clone(), ty);
        }
        let mut calls = BTreeSet::new();
        let body_ty = validate_expr(body, &info.def.name, &signatures, &mut scopes, &mut calls)?;
        if body_ty != info.return_ty {
            return Err(NativeError::for_function(
                &info.def.name,
                format!(
                    "declared return type `{}` does not match native-lowerable body type `{body_ty}`",
                    info.return_ty
                ),
            ));
        }
        info.calls = calls;
    }

    detect_recursive_cycles(&functions)?;
    Ok(ModulePlan { functions })
}

fn validate_expr(
    expr: &Expr,
    function: &str,
    signatures: &BTreeMap<String, (Vec<NativeType>, NativeType)>,
    scopes: &mut Vec<BTreeMap<String, NativeType>>,
    calls: &mut BTreeSet<String>,
) -> Result<NativeType, NativeError> {
    match expr {
        Expr::IntLit(_) => Ok(NativeType::I64),
        Expr::BoolLit(_) => Ok(NativeType::Bool),
        Expr::Var(name) => scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
            .ok_or_else(|| {
                if signatures.contains_key(name) {
                    NativeError::for_function(
                        function,
                        format!("first-class function value `{name}` is not supported"),
                    )
                } else {
                    NativeError::for_function(function, format!("unknown variable `{name}`"))
                }
            }),
        Expr::Block(stmts, tail) => {
            scopes.push(BTreeMap::new());
            for stmt in stmts {
                match stmt {
                    Stmt::Let(name, annotation, value) => {
                        let value_ty = validate_expr(value, function, signatures, scopes, calls)?;
                        if let Some(annotation) = annotation {
                            let annotated =
                                NativeType::from_type_expr(Some(annotation)).map_err(|error| {
                                    NativeError::for_function(function, error.message)
                                })?;
                            if annotated != value_ty {
                                return Err(NativeError::for_function(
                                    function,
                                    format!(
                                        "let binding `{name}` annotates `{annotated}` but native-lowerable value is `{value_ty}`"
                                    ),
                                ));
                            }
                        }
                        scopes.last_mut().unwrap().insert(name.clone(), value_ty);
                    }
                    Stmt::Expr(value) => {
                        let _ = validate_expr(value, function, signatures, scopes, calls)?;
                    }
                }
            }
            let result = match tail {
                Some(value) => validate_expr(value, function, signatures, scopes, calls)?,
                None => NativeType::Unit,
            };
            scopes.pop();
            Ok(result)
        }
        Expr::If(condition, then_branch, else_branch) => {
            let condition_ty = validate_expr(condition, function, signatures, scopes, calls)?;
            if condition_ty != NativeType::Bool {
                return Err(NativeError::for_function(
                    function,
                    format!("if condition must be Bool, found `{condition_ty}`"),
                ));
            }
            let then_ty = validate_expr(then_branch, function, signatures, scopes, calls)?;
            let else_ty = match else_branch {
                Some(else_branch) => {
                    validate_expr(else_branch, function, signatures, scopes, calls)?
                }
                None => NativeType::Unit,
            };
            if then_ty != else_ty {
                return Err(NativeError::for_function(
                    function,
                    format!("if branches must agree, found `{then_ty}` and `{else_ty}`"),
                ));
            }
            Ok(then_ty)
        }
        Expr::UnaryOp(UnaryOp::Neg, value) => {
            let value_ty = validate_expr(value, function, signatures, scopes, calls)?;
            if value_ty == NativeType::I64 {
                Ok(NativeType::I64)
            } else {
                Err(NativeError::for_function(
                    function,
                    format!("`-` expects I64, found `{value_ty}`"),
                ))
            }
        }
        Expr::UnaryOp(UnaryOp::Not, value) => {
            let value_ty = validate_expr(value, function, signatures, scopes, calls)?;
            if value_ty == NativeType::Bool {
                Ok(NativeType::Bool)
            } else {
                Err(NativeError::for_function(
                    function,
                    format!("`!` expects Bool, found `{value_ty}`"),
                ))
            }
        }
        Expr::UnaryOp(other, _) => Err(NativeError::for_function(
            function,
            format!("unary operator `{other:?}` is not supported"),
        )),
        Expr::BinOp(lhs, op, rhs) => {
            let lhs_ty = validate_expr(lhs, function, signatures, scopes, calls)?;
            let rhs_ty = validate_expr(rhs, function, signatures, scopes, calls)?;
            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                    if lhs_ty == NativeType::I64 && rhs_ty == NativeType::I64 {
                        Ok(NativeType::I64)
                    } else {
                        Err(NativeError::for_function(
                            function,
                            format!(
                                "arithmetic expects I64 operands, found `{lhs_ty}` and `{rhs_ty}`"
                            ),
                        ))
                    }
                }
                BinOp::Eq | BinOp::Ne => {
                    if lhs_ty == rhs_ty && lhs_ty != NativeType::Unit {
                        Ok(NativeType::Bool)
                    } else {
                        Err(NativeError::for_function(
                            function,
                            format!(
                                "comparison expects matching scalar operands, found `{lhs_ty}` and `{rhs_ty}`"
                            ),
                        ))
                    }
                }
                BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                    if lhs_ty == NativeType::I64 && rhs_ty == NativeType::I64 {
                        Ok(NativeType::Bool)
                    } else {
                        Err(NativeError::for_function(
                            function,
                            format!(
                                "ordering comparison expects I64 operands, found `{lhs_ty}` and `{rhs_ty}`"
                            ),
                        ))
                    }
                }
                BinOp::And | BinOp::Or => {
                    if lhs_ty == NativeType::Bool && rhs_ty == NativeType::Bool {
                        Ok(NativeType::Bool)
                    } else {
                        Err(NativeError::for_function(
                            function,
                            format!(
                                "logical operators expect Bool operands, found `{lhs_ty}` and `{rhs_ty}`"
                            ),
                        ))
                    }
                }
                other => Err(NativeError::for_function(
                    function,
                    format!("binary operator `{other:?}` is not supported"),
                )),
            }
        }
        Expr::Call(callee, args) => {
            let Expr::Var(name) = callee.as_ref() else {
                return Err(NativeError::for_function(
                    function,
                    "indirect calls are not supported",
                ));
            };
            let (params, return_ty) = signatures.get(name).ok_or_else(|| {
                NativeError::for_function(function, format!("unknown function `{name}`"))
            })?;
            if args.len() != params.len() {
                return Err(NativeError::for_function(
                    function,
                    format!(
                        "call to `{name}` expects {} args, found {}",
                        params.len(),
                        args.len()
                    ),
                ));
            }
            for (index, (arg, expected)) in args.iter().zip(params.iter()).enumerate() {
                let arg_ty = validate_expr(arg, function, signatures, scopes, calls)?;
                if &arg_ty != expected {
                    return Err(NativeError::for_function(
                        function,
                        format!(
                            "call to `{name}` argument {index} expects `{expected}`, found `{arg_ty}`"
                        ),
                    ));
                }
            }
            calls.insert(name.clone());
            Ok(*return_ty)
        }
        Expr::Return(_) => Err(NativeError::for_function(
            function,
            "`return` expressions are not supported yet",
        )),
        other => Err(NativeError::for_function(
            function,
            format!("expression `{other:?}` is not supported"),
        )),
    }
}

fn detect_recursive_cycles(
    functions: &BTreeMap<String, FunctionInfo<'_>>,
) -> Result<(), NativeError> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum VisitState {
        Visiting,
        Done,
    }

    fn dfs(
        name: &str,
        functions: &BTreeMap<String, FunctionInfo<'_>>,
        states: &mut BTreeMap<String, VisitState>,
        stack: &mut Vec<String>,
    ) -> Result<(), NativeError> {
        if states.get(name) == Some(&VisitState::Done) {
            return Ok(());
        }
        if states.get(name) == Some(&VisitState::Visiting) {
            let cycle_start = stack.iter().position(|entry| entry == name).unwrap_or(0);
            let mut cycle = stack[cycle_start..].to_vec();
            cycle.push(name.to_string());
            return Err(NativeError::unsupported(format!(
                "recursive calls are not supported: {}",
                cycle.join(" -> ")
            )));
        }

        states.insert(name.to_string(), VisitState::Visiting);
        stack.push(name.to_string());
        for callee in &functions[name].calls {
            dfs(callee, functions, states, stack)?;
        }
        stack.pop();
        states.insert(name.to_string(), VisitState::Done);
        Ok(())
    }

    let mut states = BTreeMap::new();
    let mut stack = Vec::new();
    for name in functions.keys() {
        dfs(name, functions, &mut states, &mut stack)?;
    }
    Ok(())
}

fn native_signature(module: &JITModule, arity: usize) -> cranelift_codegen::ir::Signature {
    let mut sig = module.make_signature();
    sig.params = vec![AbiParam::new(types::I64); arity];
    sig.returns.push(AbiParam::new(types::I64));
    sig
}

fn define_function(
    module: &mut JITModule,
    name: &str,
    info: &FunctionInfo<'_>,
    plan: &ModulePlan<'_>,
    func_ids: &BTreeMap<String, FuncId>,
    trap_func_id: FuncId,
) -> Result<(), NativeError> {
    let func_id = *func_ids
        .get(name)
        .ok_or_else(|| NativeError::new(format!("missing function id for `{name}`")))?;
    let mut ctx = module.make_context();
    ctx.func.signature = native_signature(module, info.params.len());
    let mut builder_ctx = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
    let entry = builder.create_block();
    for _ in &info.params {
        builder.append_block_param(entry, types::I64);
    }
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let mut scopes = vec![BTreeMap::new()];
    for ((param, ty), value) in info
        .def
        .params
        .iter()
        .zip(info.params.iter().copied())
        .zip(builder.block_params(entry).iter().copied())
    {
        scopes
            .last_mut()
            .unwrap()
            .insert(param.name.clone(), BoundValue { value, ty });
    }

    let result = compile_expr(
        &mut builder,
        module,
        plan,
        func_ids,
        trap_func_id,
        name,
        info.def.body.as_ref().unwrap(),
        &mut scopes,
    )?;
    builder.ins().return_(&[result.value]);
    builder.finalize();
    module
        .define_function(func_id, &mut ctx)
        .map_err(|error| NativeError::new(format!("failed to define `{name}`: {error}")))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn compile_expr(
    builder: &mut FunctionBuilder<'_>,
    module: &mut JITModule,
    plan: &ModulePlan<'_>,
    func_ids: &BTreeMap<String, FuncId>,
    trap_func_id: FuncId,
    current_function: &str,
    expr: &Expr,
    scopes: &mut Vec<BTreeMap<String, BoundValue>>,
) -> Result<BoundValue, NativeError> {
    match expr {
        Expr::IntLit(value) => Ok(BoundValue {
            value: builder.ins().iconst(types::I64, *value),
            ty: NativeType::I64,
        }),
        Expr::BoolLit(value) => Ok(BoundValue {
            value: builder.ins().iconst(types::I64, i64::from(*value)),
            ty: NativeType::Bool,
        }),
        Expr::Var(name) => scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
            .ok_or_else(|| {
                NativeError::for_function(current_function, format!("unknown variable `{name}`"))
            }),
        Expr::Block(stmts, tail) => {
            scopes.push(BTreeMap::new());
            for stmt in stmts {
                match stmt {
                    Stmt::Let(name, _, value) => {
                        let value = compile_expr(
                            builder,
                            module,
                            plan,
                            func_ids,
                            trap_func_id,
                            current_function,
                            value,
                            scopes,
                        )?;
                        scopes.last_mut().unwrap().insert(name.clone(), value);
                    }
                    Stmt::Expr(value) => {
                        let _ = compile_expr(
                            builder,
                            module,
                            plan,
                            func_ids,
                            trap_func_id,
                            current_function,
                            value,
                            scopes,
                        )?;
                    }
                }
            }
            let result = match tail {
                Some(value) => compile_expr(
                    builder,
                    module,
                    plan,
                    func_ids,
                    trap_func_id,
                    current_function,
                    value,
                    scopes,
                )?,
                None => unit_value(builder),
            };
            scopes.pop();
            Ok(result)
        }
        Expr::If(condition, then_branch, else_branch) => compile_if(
            builder,
            module,
            plan,
            func_ids,
            trap_func_id,
            current_function,
            condition,
            then_branch,
            else_branch.as_deref(),
            scopes,
        ),
        Expr::UnaryOp(UnaryOp::Neg, value) => {
            let value = compile_expr(
                builder,
                module,
                plan,
                func_ids,
                trap_func_id,
                current_function,
                value,
                scopes,
            )?;
            // Negating i64::MIN overflows; surface as a recoverable error.
            let min_val = builder.ins().iconst(types::I64, i64::MIN);
            let is_min = builder.ins().icmp(IntCC::Equal, value.value, min_val);
            let neg_result = builder.ins().ineg(value.value);
            Ok(emit_arith_guard(
                builder,
                module,
                trap_func_id,
                is_min,
                TRAP_OVERFLOW,
                neg_result,
                NativeType::I64,
            ))
        }
        Expr::UnaryOp(UnaryOp::Not, value) => {
            let value = compile_expr(
                builder,
                module,
                plan,
                func_ids,
                trap_func_id,
                current_function,
                value,
                scopes,
            )?;
            let cond = builder.ins().icmp_imm(IntCC::Equal, value.value, 0);
            Ok(BoundValue {
                value: bool_value(builder, cond),
                ty: NativeType::Bool,
            })
        }
        Expr::BinOp(lhs, BinOp::And, rhs) => compile_logical(
            builder,
            module,
            plan,
            func_ids,
            trap_func_id,
            current_function,
            lhs,
            rhs,
            scopes,
            false,
        ),
        Expr::BinOp(lhs, BinOp::Or, rhs) => compile_logical(
            builder,
            module,
            plan,
            func_ids,
            trap_func_id,
            current_function,
            lhs,
            rhs,
            scopes,
            true,
        ),
        Expr::BinOp(lhs, op, rhs) => {
            let lhs = compile_expr(
                builder,
                module,
                plan,
                func_ids,
                trap_func_id,
                current_function,
                lhs,
                scopes,
            )?;
            let rhs = compile_expr(
                builder,
                module,
                plan,
                func_ids,
                trap_func_id,
                current_function,
                rhs,
                scopes,
            )?;
            compile_binop(
                builder,
                module,
                trap_func_id,
                lhs,
                op,
                rhs,
                current_function,
            )
        }
        Expr::Call(callee, args) => {
            let Expr::Var(name) = callee.as_ref() else {
                return Err(NativeError::for_function(
                    current_function,
                    "indirect calls are not supported",
                ));
            };
            let func_id = *func_ids.get(name).ok_or_else(|| {
                NativeError::for_function(current_function, format!("unknown function `{name}`"))
            })?;
            let call_args = args
                .iter()
                .map(|arg| {
                    compile_expr(
                        builder,
                        module,
                        plan,
                        func_ids,
                        trap_func_id,
                        current_function,
                        arg,
                        scopes,
                    )
                    .map(|value| value.value)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let callee = module.declare_func_in_func(func_id, builder.func);
            let call = builder.ins().call(callee, &call_args);
            Ok(BoundValue {
                value: builder.inst_results(call)[0],
                ty: plan.functions[name].return_ty,
            })
        }
        other => Err(NativeError::for_function(
            current_function,
            format!("expression `{other:?}` is not supported"),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn compile_if(
    builder: &mut FunctionBuilder<'_>,
    module: &mut JITModule,
    plan: &ModulePlan<'_>,
    func_ids: &BTreeMap<String, FuncId>,
    trap_func_id: FuncId,
    current_function: &str,
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    scopes: &mut Vec<BTreeMap<String, BoundValue>>,
) -> Result<BoundValue, NativeError> {
    let condition = compile_expr(
        builder,
        module,
        plan,
        func_ids,
        trap_func_id,
        current_function,
        condition,
        scopes,
    )?;
    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, types::I64);
    let cond = builder.ins().icmp_imm(IntCC::NotEqual, condition.value, 0);
    builder.ins().brif(cond, then_block, &[], else_block, &[]);

    builder.switch_to_block(then_block);
    let then_value = compile_expr(
        builder,
        module,
        plan,
        func_ids,
        trap_func_id,
        current_function,
        then_branch,
        scopes,
    )?;
    builder.ins().jump(merge_block, &[then_value.value]);
    builder.seal_block(then_block);

    builder.switch_to_block(else_block);
    let else_value = match else_branch {
        Some(value) => compile_expr(
            builder,
            module,
            plan,
            func_ids,
            trap_func_id,
            current_function,
            value,
            scopes,
        )?,
        None => unit_value(builder),
    };
    builder.ins().jump(merge_block, &[else_value.value]);
    builder.seal_block(else_block);

    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);
    Ok(BoundValue {
        value: builder.block_params(merge_block)[0],
        ty: then_value.ty,
    })
}

#[allow(clippy::too_many_arguments)]
fn compile_logical(
    builder: &mut FunctionBuilder<'_>,
    module: &mut JITModule,
    plan: &ModulePlan<'_>,
    func_ids: &BTreeMap<String, FuncId>,
    trap_func_id: FuncId,
    current_function: &str,
    lhs: &Expr,
    rhs: &Expr,
    scopes: &mut Vec<BTreeMap<String, BoundValue>>,
    short_circuit_true: bool,
) -> Result<BoundValue, NativeError> {
    let lhs = compile_expr(
        builder,
        module,
        plan,
        func_ids,
        trap_func_id,
        current_function,
        lhs,
        scopes,
    )?;
    let rhs_block = builder.create_block();
    let short_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, types::I64);
    let lhs_cond = builder.ins().icmp_imm(IntCC::NotEqual, lhs.value, 0);
    if short_circuit_true {
        builder
            .ins()
            .brif(lhs_cond, short_block, &[], rhs_block, &[]);
    } else {
        builder
            .ins()
            .brif(lhs_cond, rhs_block, &[], short_block, &[]);
    }

    builder.switch_to_block(short_block);
    let short_value = builder
        .ins()
        .iconst(types::I64, i64::from(short_circuit_true));
    builder.ins().jump(merge_block, &[short_value]);
    builder.seal_block(short_block);

    builder.switch_to_block(rhs_block);
    let rhs = compile_expr(
        builder,
        module,
        plan,
        func_ids,
        trap_func_id,
        current_function,
        rhs,
        scopes,
    )?;
    builder.ins().jump(merge_block, &[rhs.value]);
    builder.seal_block(rhs_block);

    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);
    Ok(BoundValue {
        value: builder.block_params(merge_block)[0],
        ty: NativeType::Bool,
    })
}

fn compile_binop(
    builder: &mut FunctionBuilder<'_>,
    module: &mut JITModule,
    trap_func_id: FuncId,
    lhs: BoundValue,
    op: &BinOp,
    rhs: BoundValue,
    current_function: &str,
) -> Result<BoundValue, NativeError> {
    let value = match op {
        BinOp::Add => {
            let (result, overflow) = builder.ins().sadd_overflow(lhs.value, rhs.value);
            emit_arith_guard(
                builder,
                module,
                trap_func_id,
                overflow,
                TRAP_OVERFLOW,
                result,
                NativeType::I64,
            )
        }
        BinOp::Sub => {
            let (result, overflow) = builder.ins().ssub_overflow(lhs.value, rhs.value);
            emit_arith_guard(
                builder,
                module,
                trap_func_id,
                overflow,
                TRAP_OVERFLOW,
                result,
                NativeType::I64,
            )
        }
        BinOp::Mul => {
            let (result, overflow) = builder.ins().smul_overflow(lhs.value, rhs.value);
            emit_arith_guard(
                builder,
                module,
                trap_func_id,
                overflow,
                TRAP_OVERFLOW,
                result,
                NativeType::I64,
            )
        }
        BinOp::Div => {
            // Check divisor == 0.
            let is_zero = builder.ins().icmp_imm(IntCC::Equal, rhs.value, 0);
            let safe_block = builder.create_block();
            let div_block = builder.create_block();
            let merge_block = builder.create_block();
            builder.append_block_param(merge_block, types::I64);

            builder.ins().brif(is_zero, safe_block, &[], div_block, &[]);

            // div_zero error path.
            builder.switch_to_block(safe_block);
            builder.seal_block(safe_block);
            let trap_ref = module.declare_func_in_func(trap_func_id, builder.func);
            let code = builder.ins().iconst(types::I64, TRAP_DIV_ZERO);
            builder.ins().call(trap_ref, &[code]);
            let zero = builder.ins().iconst(types::I64, 0);
            builder.ins().jump(merge_block, &[zero]);

            // Check i64::MIN / -1 overflow.
            builder.switch_to_block(div_block);
            builder.seal_block(div_block);
            let i64_min = builder.ins().iconst(types::I64, i64::MIN);
            let lhs_is_min = builder.ins().icmp(IntCC::Equal, lhs.value, i64_min);
            let rhs_is_neg_one = builder.ins().icmp_imm(IntCC::Equal, rhs.value, -1);
            let is_div_overflow = builder.ins().band(lhs_is_min, rhs_is_neg_one);
            let ok_block = builder.create_block();
            let overflow_block = builder.create_block();
            builder
                .ins()
                .brif(is_div_overflow, overflow_block, &[], ok_block, &[]);

            // div overflow error path.
            builder.switch_to_block(overflow_block);
            builder.seal_block(overflow_block);
            let trap_ref2 = module.declare_func_in_func(trap_func_id, builder.func);
            let code2 = builder.ins().iconst(types::I64, TRAP_DIV_OVERFLOW);
            builder.ins().call(trap_ref2, &[code2]);
            let zero2 = builder.ins().iconst(types::I64, 0);
            builder.ins().jump(merge_block, &[zero2]);

            // Safe division.
            builder.switch_to_block(ok_block);
            builder.seal_block(ok_block);
            let result = builder.ins().sdiv(lhs.value, rhs.value);
            builder.ins().jump(merge_block, &[result]);

            builder.switch_to_block(merge_block);
            builder.seal_block(merge_block);
            BoundValue {
                value: builder.block_params(merge_block)[0],
                ty: NativeType::I64,
            }
        }
        BinOp::Mod => {
            // Check divisor == 0.
            let is_zero = builder.ins().icmp_imm(IntCC::Equal, rhs.value, 0);
            let error_block = builder.create_block();
            let ok_block = builder.create_block();
            let merge_block = builder.create_block();
            builder.append_block_param(merge_block, types::I64);

            builder.ins().brif(is_zero, error_block, &[], ok_block, &[]);

            // mod_zero error path.
            builder.switch_to_block(error_block);
            builder.seal_block(error_block);
            let trap_ref = module.declare_func_in_func(trap_func_id, builder.func);
            let code = builder.ins().iconst(types::I64, TRAP_MOD_ZERO);
            builder.ins().call(trap_ref, &[code]);
            let zero = builder.ins().iconst(types::I64, 0);
            builder.ins().jump(merge_block, &[zero]);

            // Safe modulo.
            builder.switch_to_block(ok_block);
            builder.seal_block(ok_block);
            let result = builder.ins().srem(lhs.value, rhs.value);
            builder.ins().jump(merge_block, &[result]);

            builder.switch_to_block(merge_block);
            builder.seal_block(merge_block);
            BoundValue {
                value: builder.block_params(merge_block)[0],
                ty: NativeType::I64,
            }
        }
        BinOp::Eq => {
            let cond = builder.ins().icmp(IntCC::Equal, lhs.value, rhs.value);
            BoundValue {
                value: bool_value(builder, cond),
                ty: NativeType::Bool,
            }
        }
        BinOp::Ne => {
            let cond = builder.ins().icmp(IntCC::NotEqual, lhs.value, rhs.value);
            BoundValue {
                value: bool_value(builder, cond),
                ty: NativeType::Bool,
            }
        }
        BinOp::Lt => {
            let cond = builder
                .ins()
                .icmp(IntCC::SignedLessThan, lhs.value, rhs.value);
            BoundValue {
                value: bool_value(builder, cond),
                ty: NativeType::Bool,
            }
        }
        BinOp::Gt => {
            let cond = builder
                .ins()
                .icmp(IntCC::SignedGreaterThan, lhs.value, rhs.value);
            BoundValue {
                value: bool_value(builder, cond),
                ty: NativeType::Bool,
            }
        }
        BinOp::Le => {
            let cond = builder
                .ins()
                .icmp(IntCC::SignedLessThanOrEqual, lhs.value, rhs.value);
            BoundValue {
                value: bool_value(builder, cond),
                ty: NativeType::Bool,
            }
        }
        BinOp::Ge => {
            let cond = builder
                .ins()
                .icmp(IntCC::SignedGreaterThanOrEqual, lhs.value, rhs.value);
            BoundValue {
                value: bool_value(builder, cond),
                ty: NativeType::Bool,
            }
        }
        other => {
            return Err(NativeError::for_function(
                current_function,
                format!("binary operator `{other:?}` is not supported"),
            ));
        }
    };
    Ok(value)
}

fn bool_value(
    builder: &mut FunctionBuilder<'_>,
    condition: cranelift_codegen::ir::Value,
) -> cranelift_codegen::ir::Value {
    let one = builder.ins().iconst(types::I64, 1);
    let zero = builder.ins().iconst(types::I64, 0);
    builder.ins().select(condition, one, zero)
}

fn unit_value(builder: &mut FunctionBuilder<'_>) -> BoundValue {
    BoundValue {
        value: builder.ins().iconst(types::I64, 0),
        ty: NativeType::Unit,
    }
}

/// Emit a conditional arithmetic error guard in JIT IR.
///
/// If `condition` is non-zero, calls `spore_arith_trap(trap_code)` and
/// continues with 0 as the placeholder result. Otherwise continues with
/// `ok_result`. The caller checks the thread-local after the JIT call
/// returns and converts any set trap code into `Err(NativeError)`.
fn emit_arith_guard(
    builder: &mut FunctionBuilder<'_>,
    module: &mut JITModule,
    trap_func_id: FuncId,
    condition: cranelift_codegen::ir::Value,
    trap_code: i64,
    ok_result: cranelift_codegen::ir::Value,
    ty: NativeType,
) -> BoundValue {
    let trap_block = builder.create_block();
    let ok_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, types::I64);

    builder
        .ins()
        .brif(condition, trap_block, &[], ok_block, &[]);

    builder.switch_to_block(trap_block);
    builder.seal_block(trap_block);
    let trap_ref = module.declare_func_in_func(trap_func_id, builder.func);
    let code_val = builder.ins().iconst(types::I64, trap_code);
    builder.ins().call(trap_ref, &[code_val]);
    let zero = builder.ins().iconst(types::I64, 0);
    builder.ins().jump(merge_block, &[zero]);

    builder.switch_to_block(ok_block);
    builder.seal_block(ok_block);
    builder.ins().jump(merge_block, &[ok_result]);

    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);
    BoundValue {
        value: builder.block_params(merge_block)[0],
        ty,
    }
}

fn invoke_compiled_function(ptr: *const u8, args: &[i64]) -> Result<i64, NativeError> {
    macro_rules! call {
        ($ptr:expr, []) => {{
            let function: unsafe extern "C" fn() -> i64 = unsafe { std::mem::transmute($ptr) };
            unsafe { function() }
        }};
        ($ptr:expr, [$a0:expr]) => {{
            let function: unsafe extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute($ptr) };
            unsafe { function($a0) }
        }};
        ($ptr:expr, [$a0:expr, $a1:expr]) => {{
            let function: unsafe extern "C" fn(i64, i64) -> i64 =
                unsafe { std::mem::transmute($ptr) };
            unsafe { function($a0, $a1) }
        }};
        ($ptr:expr, [$a0:expr, $a1:expr, $a2:expr]) => {{
            let function: unsafe extern "C" fn(i64, i64, i64) -> i64 =
                unsafe { std::mem::transmute($ptr) };
            unsafe { function($a0, $a1, $a2) }
        }};
        ($ptr:expr, [$a0:expr, $a1:expr, $a2:expr, $a3:expr]) => {{
            let function: unsafe extern "C" fn(i64, i64, i64, i64) -> i64 =
                unsafe { std::mem::transmute($ptr) };
            unsafe { function($a0, $a1, $a2, $a3) }
        }};
        ($ptr:expr, [$a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr]) => {{
            let function: unsafe extern "C" fn(i64, i64, i64, i64, i64) -> i64 =
                unsafe { std::mem::transmute($ptr) };
            unsafe { function($a0, $a1, $a2, $a3, $a4) }
        }};
        ($ptr:expr, [$a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr]) => {{
            let function: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64) -> i64 =
                unsafe { std::mem::transmute($ptr) };
            unsafe { function($a0, $a1, $a2, $a3, $a4, $a5) }
        }};
        ($ptr:expr, [$a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr, $a6:expr]) => {{
            let function: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64) -> i64 =
                unsafe { std::mem::transmute($ptr) };
            unsafe { function($a0, $a1, $a2, $a3, $a4, $a5, $a6) }
        }};
        ($ptr:expr, [$a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr, $a6:expr, $a7:expr]) => {{
            let function: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64) -> i64 =
                unsafe { std::mem::transmute($ptr) };
            unsafe { function($a0, $a1, $a2, $a3, $a4, $a5, $a6, $a7) }
        }};
    }

    // Clear any trap flag left by a previous call on this thread.
    ARITH_TRAP.with(|cell| cell.set(0));

    let raw = match args {
        [] => call!(ptr, []),
        [a0] => call!(ptr, [*a0]),
        [a0, a1] => call!(ptr, [*a0, *a1]),
        [a0, a1, a2] => call!(ptr, [*a0, *a1, *a2]),
        [a0, a1, a2, a3] => call!(ptr, [*a0, *a1, *a2, *a3]),
        [a0, a1, a2, a3, a4] => call!(ptr, [*a0, *a1, *a2, *a3, *a4]),
        [a0, a1, a2, a3, a4, a5] => call!(ptr, [*a0, *a1, *a2, *a3, *a4, *a5]),
        [a0, a1, a2, a3, a4, a5, a6] => call!(ptr, [*a0, *a1, *a2, *a3, *a4, *a5, *a6]),
        [a0, a1, a2, a3, a4, a5, a6, a7] => {
            call!(ptr, [*a0, *a1, *a2, *a3, *a4, *a5, *a6, *a7])
        }
        _ => {
            return Err(NativeError::unsupported(format!(
                "entry invocation supports at most {MAX_ENTRY_ARGS} scalar arguments"
            )));
        }
    };

    // Check whether JIT code signalled a recoverable arithmetic error.
    let trap_code = ARITH_TRAP.with(|cell| cell.get());
    if trap_code != 0 {
        ARITH_TRAP.with(|cell| cell.set(0));
        return Err(NativeError::new(match trap_code {
            TRAP_OVERFLOW => "integer overflow".to_string(),
            TRAP_DIV_ZERO => "division by zero".to_string(),
            TRAP_MOD_ZERO => "modulo by zero".to_string(),
            TRAP_DIV_OVERFLOW => "integer overflow in division (i64::MIN / -1)".to_string(),
            other => format!("arithmetic error (code {other})"),
        }));
    }

    Ok(raw)
}
