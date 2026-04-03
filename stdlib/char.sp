// Spore standard library — character utilities
// Characters are represented as single-character strings.

fn is_digit(c: String) -> Bool cost <= 3 {
    let code = char_to_int(c);
    code >= 48 && code <= 57
}

fn is_letter(c: String) -> Bool cost <= 3 {
    is_uppercase(c) || is_lowercase(c)
}

fn is_whitespace(c: String) -> Bool cost <= 3 {
    c == " " || c == "\t" || c == "\n" || c == "\r"
}

fn is_uppercase(c: String) -> Bool cost <= 3 {
    let code = char_to_int(c);
    code >= 65 && code <= 90
}

fn is_lowercase(c: String) -> Bool cost <= 3 {
    let code = char_to_int(c);
    code >= 97 && code <= 122
}

fn is_alphanumeric(c: String) -> Bool cost <= 3 {
    is_letter(c) || is_digit(c)
}
