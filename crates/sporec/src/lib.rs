/// sporec — Spore language compiler
///
/// Stateless pure function: source → compiled output.
/// All IO is handled by the `spore` codebase manager.
pub mod compiler;

pub use compiler::{HoleSummary, check_verbose, compile, compile_files, hole_summary, holes, run};
