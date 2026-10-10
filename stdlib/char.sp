// Spore standard library — character utilities
// Characters are represented as single-character strings.
fn is_digit(c: Str) -> Bool
properties {
    digit(): is_digit("5") == true
    letter(): is_digit("a") == false
    zero(): is_digit("0") == true
    nine(): is_digit("9") == true
}
{
    let code = char_to_int(c);
    code >= 48 && code <= 57
}

fn is_letter(c: Str) -> Bool
properties {
    lower(): is_letter("a") == true
    upper(): is_letter("Z") == true
    digit(): is_letter("5") == false
}
{ is_uppercase(c) || is_lowercase(c) }

fn is_whitespace(c: Str) -> Bool
properties {
    space(): is_whitespace(" ") == true
    letter(): is_whitespace("a") == false
}
{ c == " " || c == "\t" || c == "\n" || c == "\r" }

fn is_uppercase(c: Str) -> Bool
properties {
    upper(): is_uppercase("A") == true
    lower(): is_uppercase("a") == false
    digit(): is_uppercase("5") == false
}
{
    let code = char_to_int(c);
    code >= 65 && code <= 90
}

fn is_lowercase(c: Str) -> Bool
properties {
    lower(): is_lowercase("a") == true
    upper(): is_lowercase("A") == false
    digit(): is_lowercase("5") == false
}
{
    let code = char_to_int(c);
    code >= 97 && code <= 122
}

fn is_alphanumeric(c: Str) -> Bool
properties {
    letter(): is_alphanumeric("a") == true
    digit(): is_alphanumeric("5") == true
    space(): is_alphanumeric(" ") == false
}
{ is_letter(c) || is_digit(c) }
