// Spore standard library — character utilities
// Characters are represented as single-character strings.

fn is_digit(c: Str) -> Bool cost <= 3
spec {
    example "digit": is_digit("5") == true
    example "letter": is_digit("a") == false
    example "zero": is_digit("0") == true
    example "nine": is_digit("9") == true
}
{
    let code = char_to_int(c);
    code >= 48 && code <= 57
}

fn is_letter(c: Str) -> Bool cost <= 3
spec {
    example "lower": is_letter("a") == true
    example "upper": is_letter("Z") == true
    example "digit": is_letter("5") == false
}
{
    is_uppercase(c) || is_lowercase(c)
}

fn is_whitespace(c: Str) -> Bool cost <= 3
spec {
    example "space": is_whitespace(" ") == true
    example "letter": is_whitespace("a") == false
}
{
    c == " " || c == "\t" || c == "\n" || c == "\r"
}

fn is_uppercase(c: Str) -> Bool cost <= 3
spec {
    example "upper": is_uppercase("A") == true
    example "lower": is_uppercase("a") == false
    example "digit": is_uppercase("5") == false
}
{
    let code = char_to_int(c);
    code >= 65 && code <= 90
}

fn is_lowercase(c: Str) -> Bool cost <= 3
spec {
    example "lower": is_lowercase("a") == true
    example "upper": is_lowercase("A") == false
    example "digit": is_lowercase("5") == false
}
{
    let code = char_to_int(c);
    code >= 97 && code <= 122
}

fn is_alphanumeric(c: Str) -> Bool cost <= 3
spec {
    example "letter": is_alphanumeric("a") == true
    example "digit": is_alphanumeric("5") == true
    example "space": is_alphanumeric(" ") == false
}
{
    is_letter(c) || is_digit(c)
}
