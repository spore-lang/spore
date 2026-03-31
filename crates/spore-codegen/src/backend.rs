//! Code generation backend trait and types.
//!
//! Defines the interface for code generation backends.
//! The interpreter is one backend; Cranelift will be another.

/// Compilation target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Tree-walking interpreter (current default)
    Interpreter,
    /// Native code via Cranelift (future)
    Native { arch: Arch, os: Os },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
    Wasm32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    MacOS,
    Windows,
    Wasi,
}

impl Target {
    /// Get the current host target.
    pub fn host() -> Self {
        Target::Native {
            arch: if cfg!(target_arch = "aarch64") {
                Arch::Aarch64
            } else {
                Arch::X86_64
            },
            os: if cfg!(target_os = "macos") {
                Os::MacOS
            } else if cfg!(target_os = "windows") {
                Os::Windows
            } else {
                Os::Linux
            },
        }
    }

    pub fn interpreter() -> Self {
        Target::Interpreter
    }

    /// Get the target triple string.
    pub fn triple(&self) -> String {
        match self {
            Target::Interpreter => "interpreter".to_string(),
            Target::Native { arch, os } => {
                let arch_str = match arch {
                    Arch::X86_64 => "x86_64",
                    Arch::Aarch64 => "aarch64",
                    Arch::Wasm32 => "wasm32",
                };
                let os_str = match os {
                    Os::Linux => "unknown-linux-gnu",
                    Os::MacOS => "apple-darwin",
                    Os::Windows => "pc-windows-msvc",
                    Os::Wasi => "wasi",
                };
                format!("{arch_str}-{os_str}")
            }
        }
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.triple())
    }
}

/// Compiled output from a backend.
#[derive(Debug)]
pub enum CompiledOutput {
    /// Interpreter result (a value)
    InterpreterResult(String),
    /// Object file bytes
    ObjectFile(Vec<u8>),
}

/// Configuration for code generation.
#[derive(Debug, Clone)]
pub struct CodegenConfig {
    pub target: Target,
    pub opt_level: OptLevel,
    pub debug_info: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    None,
    Speed,
    Size,
}

impl Default for CodegenConfig {
    fn default() -> Self {
        Self {
            target: Target::interpreter(),
            opt_level: OptLevel::None,
            debug_info: false,
        }
    }
}

/// Trait for code generation backends.
pub trait CodegenBackend {
    /// Compile a module to the target.
    fn compile(
        &mut self,
        module: &spore_parser::ast::Module,
        config: &CodegenConfig,
    ) -> Result<CompiledOutput, CodegenError>;

    /// Get the name of this backend.
    fn name(&self) -> &str;

    /// Get the supported targets.
    fn supported_targets(&self) -> Vec<Target>;
}

/// Code generation errors.
#[derive(Debug, Clone)]
pub struct CodegenError {
    pub message: String,
}

impl CodegenError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "codegen error: {}", self.message)
    }
}

impl std::error::Error for CodegenError {}

/// Function-level compilation info for future Cranelift backend.
#[derive(Debug, Clone)]
pub struct FunctionCompilation {
    pub name: String,
    pub params: Vec<CompileType>,
    pub return_type: CompileType,
    pub body_ir: Vec<IrInst>,
}

/// Simplified type for codegen (not the full Ty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileType {
    I64,
    F64,
    Bool,
    /// Pointer to heap-allocated data (strings, structs)
    Ptr,
    Unit,
}

/// Simplified IR instructions for code generation.
#[derive(Debug, Clone)]
pub enum IrInst {
    /// Load integer constant
    ConstI64(i64),
    /// Load float constant
    ConstF64(f64),
    /// Load boolean constant
    ConstBool(bool),
    /// Load a local variable by index
    LoadLocal(usize),
    /// Store to a local variable by index
    StoreLocal(usize),
    /// Binary arithmetic operation
    BinOp(IrBinOp),
    /// Call a function by name, with argument count
    Call(String, usize),
    /// Return
    Return,
    /// Branch if zero to target instruction index
    BranchIfZero(usize),
    /// Unconditional branch to target instruction index
    Branch(usize),
}

#[derive(Debug, Clone, Copy)]
pub enum IrBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_target() {
        let target = Target::host();
        let triple = target.triple();
        assert!(!triple.is_empty());
        // On macOS ARM
        if cfg!(target_arch = "aarch64") && cfg!(target_os = "macos") {
            assert_eq!(triple, "aarch64-apple-darwin");
        }
    }

    #[test]
    fn interpreter_target() {
        let target = Target::interpreter();
        assert_eq!(target.triple(), "interpreter");
    }

    #[test]
    fn default_config() {
        let config = CodegenConfig::default();
        assert_eq!(config.target, Target::Interpreter);
        assert_eq!(config.opt_level, OptLevel::None);
        assert!(!config.debug_info);
    }

    #[test]
    fn target_display() {
        let t = Target::Native {
            arch: Arch::X86_64,
            os: Os::Linux,
        };
        assert_eq!(t.to_string(), "x86_64-unknown-linux-gnu");
    }

    #[test]
    fn compile_type_equality() {
        assert_eq!(CompileType::I64, CompileType::I64);
        assert_ne!(CompileType::I64, CompileType::F64);
    }
}
