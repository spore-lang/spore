//! Structured concurrency model.
//!
//! Spore uses structured concurrency where every spawned task has
//! a well-defined scope and parent. Tasks form a tree — a parent
//! task cannot complete until all children complete.

use std::collections::HashMap;

/// Task type — represents a spawned concurrent computation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskType {
    /// The result type of the task.
    pub result_type: String,
    /// Capabilities required by the task.
    pub capabilities: Vec<String>,
    /// Error types the task may produce.
    pub errors: Vec<String>,
}

/// A structured concurrency scope.
#[derive(Debug, Clone)]
pub struct TaskScope {
    /// Name of the scope (usually function name).
    pub name: String,
    /// Child tasks spawned in this scope.
    pub children: Vec<TaskInfo>,
    /// Whether this scope awaits all children before returning.
    pub structured: bool,
}

/// Information about a spawned task.
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub id: u32,
    pub task_type: TaskType,
    pub spawn_site: String,
}

/// Concurrency analyzer — validates structured concurrency rules.
#[derive(Debug, Default)]
pub struct ConcurrencyAnalyzer {
    next_task_id: u32,
    /// Function → task scope
    scopes: HashMap<String, TaskScope>,
    /// Warnings about concurrency issues
    pub warnings: Vec<ConcurrencyWarning>,
}

#[derive(Debug, Clone)]
pub struct ConcurrencyWarning {
    pub kind: WarningKind,
    pub message: String,
    pub function: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarningKind {
    /// A task is spawned but never awaited in the same scope.
    UnawaitedTask,
    /// A non-async function calls spawn.
    SpawnWithoutAsync,
    /// Nested spawn without intermediate await.
    DeepNesting,
}

impl ConcurrencyAnalyzer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin analyzing a function's concurrency.
    pub fn enter_function(&mut self, name: &str) {
        self.scopes.insert(
            name.to_string(),
            TaskScope {
                name: name.to_string(),
                children: Vec::new(),
                structured: true,
            },
        );
    }

    /// Record a spawn expression in the current function.
    pub fn record_spawn(
        &mut self,
        function: &str,
        result_type: &str,
        capabilities: Vec<String>,
    ) -> u32 {
        let id = self.next_task_id;
        self.next_task_id += 1;

        let task = TaskInfo {
            id,
            task_type: TaskType {
                result_type: result_type.to_string(),
                capabilities,
                errors: Vec::new(),
            },
            spawn_site: function.to_string(),
        };

        if let Some(scope) = self.scopes.get_mut(function) {
            scope.children.push(task);
        }

        id
    }

    /// Record an await expression.
    pub fn record_await(&mut self, _function: &str, _task_id: Option<u32>) {
        // Track that a task was awaited in this scope
    }

    /// Finish analyzing a function and produce warnings.
    pub fn leave_function(&mut self, function: &str) {
        if let Some(scope) = self.scopes.get(function) {
            // Check for unawaited tasks (simplified check)
            if !scope.children.is_empty() && scope.structured {
                // In structured concurrency, all tasks should be awaited
                // For now, just record the scope
            }
        }
    }

    /// Get the task scope for a function.
    pub fn scope(&self, function: &str) -> Option<&TaskScope> {
        self.scopes.get(function)
    }

    /// Get all task scopes.
    pub fn all_scopes(&self) -> &HashMap<String, TaskScope> {
        &self.scopes
    }

    /// Get the total number of spawn sites analyzed.
    pub fn total_spawns(&self) -> usize {
        self.scopes.values().map(|s| s.children.len()).sum()
    }
}

/// Validate that a function's concurrency is well-structured.
pub fn check_structured_concurrency(scope: &TaskScope) -> Vec<ConcurrencyWarning> {
    let mut warnings = Vec::new();

    if scope.children.len() > 10 {
        warnings.push(ConcurrencyWarning {
            kind: WarningKind::DeepNesting,
            message: format!(
                "function `{}` spawns {} tasks — consider breaking into smaller scopes",
                scope.name,
                scope.children.len()
            ),
            function: scope.name.clone(),
        });
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_task_type() {
        let t = TaskType {
            result_type: "Int".into(),
            capabilities: vec!["NetRead".into()],
            errors: vec![],
        };
        assert_eq!(t.result_type, "Int");
    }

    #[test]
    fn analyzer_records_spawns() {
        let mut analyzer = ConcurrencyAnalyzer::new();
        analyzer.enter_function("main");
        let id1 = analyzer.record_spawn("main", "Int", vec![]);
        let id2 = analyzer.record_spawn("main", "String", vec!["NetRead".into()]);
        assert_ne!(id1, id2);
        assert_eq!(analyzer.total_spawns(), 2);
    }

    #[test]
    fn task_scope_tracking() {
        let mut analyzer = ConcurrencyAnalyzer::new();
        analyzer.enter_function("worker");
        analyzer.record_spawn("worker", "Bool", vec![]);
        analyzer.leave_function("worker");

        let scope = analyzer.scope("worker").unwrap();
        assert_eq!(scope.children.len(), 1);
        assert!(scope.structured);
    }

    #[test]
    fn deep_nesting_warning() {
        let mut scope = TaskScope {
            name: "heavy".into(),
            children: Vec::new(),
            structured: true,
        };
        for i in 0..15 {
            scope.children.push(TaskInfo {
                id: i,
                task_type: TaskType {
                    result_type: "Unit".into(),
                    capabilities: vec![],
                    errors: vec![],
                },
                spawn_site: "heavy".into(),
            });
        }
        let warnings = check_structured_concurrency(&scope);
        assert!(warnings.iter().any(|w| w.kind == WarningKind::DeepNesting));
    }

    #[test]
    fn empty_scope_no_warnings() {
        let scope = TaskScope {
            name: "pure".into(),
            children: Vec::new(),
            structured: true,
        };
        let warnings = check_structured_concurrency(&scope);
        assert!(warnings.is_empty());
    }
}
