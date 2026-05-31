// Spore standard library — string utilities
// Most string operations (trim, to_upper, to_lower, starts_with,
// ends_with, split, replace, string_length, char_at, substring)
// are runtime builtins — do NOT redefine them here.
fn is_empty(s: Str) -> Bool
properties {
    empty(): is_empty("") == true
    nonempty(): is_empty("hi") == false
}
{ string_length(s) == 0 }

fn is_not_empty(s: Str) -> Bool
properties {
    empty(): is_not_empty("") == false
    nonempty(): is_not_empty("hi") == true
}
{ string_length(s) > 0 }

fn is_blank(s: Str) -> Bool
properties {
    empty(): is_blank("") == true
    spaces(): is_blank("   ") == true
    content(): is_blank("hi") == false
}
{ string_length(trim(s)) == 0 }

fn char_at_safe(s: Str, i: I64) -> Option[Str] {
    if i < 0 { None } else {
        if i >= string_length(s) { None } else { char_at(s, i) }
    }
}

fn repeat_string(s: Str, n: I64) -> Str
properties {
    basic(): repeat_string("ab", 3) == "ababab"
    zero(): repeat_string("x", 0) == ""
    one(): repeat_string("hi", 1) == "hi"
}
{
    if n <= 0 { "" } else { s + repeat_string(s, n - 1) }
}

fn pad_left(s: Str, width: I64, fill: Str) -> Str
properties {
    pad(): pad_left("hi", 5, " ") == "   hi"
    no_pad(): pad_left("hello", 3, " ") == "hello"
}
{
    if string_length(s) >= width { s } else { pad_left(fill + s, width, fill) }
}

fn pad_right(s: Str, width: I64, fill: Str) -> Str
properties {
    pad(): pad_right("hi", 5, " ") == "hi   "
    no_pad(): pad_right("hello", 3, " ") == "hello"
}
{
    if string_length(s) >= width { s } else { pad_right(s + fill, width, fill) }
}
