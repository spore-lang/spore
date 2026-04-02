// Spore standard library — string utilities
// Most string operations delegate to builtins.

fn is_empty(s: String) -> Bool { string_length(s) == 0 }
fn is_not_empty(s: String) -> Bool { string_length(s) > 0 }
