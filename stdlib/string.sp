// Spore standard library — string utilities
// Most string operations (trim, to_upper, to_lower, starts_with,
// ends_with, split, replace, string_length, char_at, substring)
// are runtime builtins — do NOT redefine them here.

fn is_empty(s: String) -> Bool cost <= 2 { string_length(s) == 0 }
fn is_not_empty(s: String) -> Bool cost <= 2 { string_length(s) > 0 }

fn is_blank(s: String) -> Bool cost <= 3 { string_length(trim(s)) == 0 }

fn char_at_safe(s: String, i: Int) -> Option[String] cost <= 2 {
    if i < 0 { None }
    else { if i >= string_length(s) { None } else { char_at(s, i) } }
}

@unbounded
fn repeat_string(s: String, n: Int) -> String {
    if n <= 0 { "" }
    else { s + repeat_string(s, n - 1) }
}

@unbounded
fn pad_left(s: String, width: Int, fill: String) -> String {
    if string_length(s) >= width { s }
    else { pad_left(fill + s, width, fill) }
}

@unbounded
fn pad_right(s: String, width: Int, fill: String) -> String {
    if string_length(s) >= width { s }
    else { pad_right(s + fill, width, fill) }
}
