// Spore standard library — string utilities
// Most string operations (trim, to_upper, to_lower, starts_with,
// ends_with, split, replace, string_length, char_at, substring)
// are runtime builtins — do NOT redefine them here.

fn is_empty(s: Str) -> Bool cost <= 2
spec {
    example "empty": is_empty("") == true
    example "nonempty": is_empty("hi") == false
}
{ string_length(s) == 0 }

fn is_not_empty(s: Str) -> Bool cost <= 2
spec {
    example "empty": is_not_empty("") == false
    example "nonempty": is_not_empty("hi") == true
}
{ string_length(s) > 0 }

fn is_blank(s: Str) -> Bool cost <= 3
spec {
    example "empty": is_blank("") == true
    example "spaces": is_blank("   ") == true
    example "content": is_blank("hi") == false
}
{ string_length(trim(s)) == 0 }

fn char_at_safe(s: Str, i: I32) -> Option[Str] cost <= 2 {
    if i < 0 { None }
    else { if i >= string_length(s) { None } else { char_at(s, i) } }
}

@unbounded
fn repeat_string(s: Str, n: I32) -> Str
spec {
    example "basic": repeat_string("ab", 3) == "ababab"
    example "zero": repeat_string("x", 0) == ""
    example "one": repeat_string("hi", 1) == "hi"
}
{
    if n <= 0 { "" }
    else { s + repeat_string(s, n - 1) }
}

@unbounded
fn pad_left(s: Str, width: I32, fill: Str) -> Str
spec {
    example "pad": pad_left("hi", 5, " ") == "   hi"
    example "no_pad": pad_left("hello", 3, " ") == "hello"
}
{
    if string_length(s) >= width { s }
    else { pad_left(fill + s, width, fill) }
}

@unbounded
fn pad_right(s: Str, width: I32, fill: Str) -> Str
spec {
    example "pad": pad_right("hi", 5, " ") == "hi   "
    example "no_pad": pad_right("hello", 3, " ") == "hello"
}
{
    if string_length(s) >= width { s }
    else { pad_right(s + fill, width, fill) }
}
