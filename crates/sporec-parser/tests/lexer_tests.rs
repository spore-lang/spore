use sporec_parser::lexer::{Lexer, Span, TemplatePart, Token};

fn toks(src: &str) -> Vec<Token> {
    Lexer::new(src)
        .tokenize()
        .expect("lex error")
        .into_iter()
        .map(|s| s.node)
        .collect()
}

fn toks_no_eof(src: &str) -> Vec<Token> {
    let mut v = toks(src);
    assert_eq!(v.last(), Some(&Token::Eof));
    v.pop();
    v
}

// ── Basic ────────────────────────────────────────────────────────────────

#[test]
fn test_empty_input() {
    assert_eq!(toks(""), vec![Token::Eof]);
}

#[test]
fn test_whitespace_only() {
    assert_eq!(toks("   \n\t  "), vec![Token::Eof]);
}

// ── Literals ─────────────────────────────────────────────────────────────

#[test]
fn test_integer_literals() {
    assert_eq!(
        toks_no_eof("42 0xFF 0b1010 0o77"),
        vec![
            Token::Int(42),
            Token::Int(0xFF),
            Token::Int(0b1010),
            Token::Int(0o77),
        ]
    );
}

#[test]
fn test_integer_underscores() {
    assert_eq!(toks_no_eof("1_000_000"), vec![Token::Int(1_000_000)]);
}

#[test]
#[allow(clippy::approx_constant)]
fn test_float_literal() {
    assert_eq!(toks_no_eof("3.14"), vec![Token::Float(3.14)]);
}

#[test]
fn test_string_literal() {
    assert_eq!(toks_no_eof(r#""hello""#), vec![Token::Str("hello".into())]);
}

#[test]
fn test_string_escapes() {
    assert_eq!(
        toks_no_eof(r#""line\n\ttab\\end""#),
        vec![Token::Str("line\n\ttab\\end".into())]
    );
}

#[test]
fn test_bool_literals() {
    assert_eq!(
        toks_no_eof("true false"),
        vec![Token::Bool(true), Token::Bool(false)]
    );
}

// ── Identifiers ──────────────────────────────────────────────────────────

#[test]
fn test_identifiers() {
    assert_eq!(
        toks_no_eof("foo bar_baz _x A1"),
        vec![
            Token::Ident("foo".into()),
            Token::Ident("bar_baz".into()),
            Token::Ident("_x".into()),
            Token::Ident("A1".into()),
        ]
    );
}

// ── Keywords ─────────────────────────────────────────────────────────────

#[test]
fn test_keywords() {
    assert_eq!(
        toks_no_eof("fn let if when else match return"),
        vec![
            Token::Fn,
            Token::Let,
            Token::If,
            Token::When,
            Token::Else,
            Token::Match,
            Token::Return
        ]
    );
}

#[test]
fn test_more_keywords() {
    assert_eq!(
        toks_no_eof("pub struct type capability import as"),
        vec![
            Token::Pub,
            Token::Struct,
            Token::Type,
            Token::Capability,
            Token::Import,
            Token::As,
        ]
    );
}

#[test]
fn test_effect_keywords() {
    assert_eq!(
        toks_no_eof("spawn await where cost uses throw select"),
        vec![
            Token::Spawn,
            Token::Await,
            Token::Where,
            Token::Cost,
            Token::Uses,
            Token::Throw,
            Token::Select,
        ]
    );
}

#[test]
fn test_trait_effect_handler_spec_keywords() {
    assert_eq!(
        toks_no_eof("trait effect handler spec"),
        vec![Token::Trait, Token::Effect, Token::Handler, Token::Spec]
    );
}

#[test]
fn test_module_keywords() {
    assert_eq!(
        toks_no_eof("mod pkg in self impl parallel_scope"),
        vec![
            Token::Mod,
            Token::Pkg,
            Token::In,
            Token::Self_,
            Token::Impl,
            Token::ParallelScope,
        ]
    );
}

// ── Operators ────────────────────────────────────────────────────────────

#[test]
fn test_arithmetic_operators() {
    assert_eq!(
        toks_no_eof("+ - * / %"),
        vec![
            Token::Plus,
            Token::Minus,
            Token::Star,
            Token::Slash,
            Token::Percent
        ]
    );
}

#[test]
fn test_comparison_operators() {
    assert_eq!(
        toks_no_eof("== != < > <= >="),
        vec![
            Token::EqEq,
            Token::NotEq,
            Token::Lt,
            Token::Gt,
            Token::LtEq,
            Token::GtEq,
        ]
    );
}

#[test]
fn test_logical_operators() {
    assert_eq!(
        toks_no_eof("&& || !"),
        vec![Token::AndAnd, Token::OrOr, Token::Bang]
    );
}

#[test]
fn test_bitwise_operators() {
    assert_eq!(
        toks_no_eof("& | ^ ~ << >>"),
        vec![
            Token::Amp,
            Token::Pipe,
            Token::Caret,
            Token::Tilde,
            Token::Shl,
            Token::Shr
        ]
    );
}

#[test]
fn test_two_char_operators() {
    assert_eq!(
        toks_no_eof("|> -> => :: ..="),
        vec![
            Token::PipeArrow,
            Token::Arrow,
            Token::FatArrow,
            Token::ColonColon,
            Token::DotDotEq,
        ]
    );
}

#[test]
fn test_pipe_arrow() {
    assert_eq!(
        toks_no_eof("x |> f"),
        vec![
            Token::Ident("x".into()),
            Token::PipeArrow,
            Token::Ident("f".into())
        ]
    );
}

#[test]
fn test_dot_dot() {
    assert_eq!(
        toks_no_eof("0..10"),
        vec![Token::Int(0), Token::DotDot, Token::Int(10)]
    );
}

// ── Delimiters ───────────────────────────────────────────────────────────

#[test]
fn test_delimiters() {
    assert_eq!(
        toks_no_eof("( ) { } [ ]"),
        vec![
            Token::LParen,
            Token::RParen,
            Token::LBrace,
            Token::RBrace,
            Token::LBracket,
            Token::RBracket,
        ]
    );
}

// ── Punctuation ──────────────────────────────────────────────────────────

#[test]
fn test_punctuation() {
    assert_eq!(
        toks_no_eof(", : ; . @ #"),
        vec![
            Token::Comma,
            Token::Colon,
            Token::Semicolon,
            Token::Dot,
            Token::At,
            Token::Hash,
        ]
    );
}

// ── Comments ─────────────────────────────────────────────────────────────

#[test]
fn test_line_comment() {
    assert_eq!(
        toks_no_eof("42 // this is a comment\n43"),
        vec![Token::Int(42), Token::Int(43)]
    );
}

#[test]
fn test_block_comment() {
    assert_eq!(
        toks_no_eof("42 /* block comment */ 43"),
        vec![Token::Int(42), Token::Int(43)]
    );
}

#[test]
fn test_nested_block_comment() {
    assert_eq!(
        toks_no_eof("42 /* outer /* inner */ */ 43"),
        vec![Token::Int(42), Token::Int(43)]
    );
}

// ── Special tokens ───────────────────────────────────────────────────────

#[test]
fn test_unicode_le() {
    assert_eq!(
        toks_no_eof("x ≤ 100"),
        vec![Token::Ident("x".into()), Token::Le2, Token::Int(100)]
    );
}

#[test]
fn test_question_mark() {
    assert_eq!(
        toks_no_eof("x?"),
        vec![Token::Ident("x".into()), Token::Question]
    );
}

#[test]
fn test_eq_assign() {
    assert_eq!(
        toks_no_eof("x = 5"),
        vec![Token::Ident("x".into()), Token::Eq, Token::Int(5)]
    );
}

// ── Spans ────────────────────────────────────────────────────────────────

#[test]
fn test_spans() {
    let tokens = Lexer::new("fn add").tokenize().unwrap();
    assert_eq!(tokens[0].span, Span::new(0, 2));
    assert_eq!(tokens[0].node, Token::Fn);
    assert_eq!(tokens[1].span, Span::new(3, 6));
    assert_eq!(tokens[1].node, Token::Ident("add".into()));
}

// ── Integration ──────────────────────────────────────────────────────────

#[test]
fn test_complete_function() {
    let src = "fn add(a: Int, b: Int) -> Int { a + b }";
    assert_eq!(
        toks_no_eof(src),
        vec![
            Token::Fn,
            Token::Ident("add".into()),
            Token::LParen,
            Token::Ident("a".into()),
            Token::Colon,
            Token::Ident("Int".into()),
            Token::Comma,
            Token::Ident("b".into()),
            Token::Colon,
            Token::Ident("Int".into()),
            Token::RParen,
            Token::Arrow,
            Token::Ident("Int".into()),
            Token::LBrace,
            Token::Ident("a".into()),
            Token::Plus,
            Token::Ident("b".into()),
            Token::RBrace,
        ]
    );
}

#[test]
fn test_function_with_uses() {
    let src = "fn fetch(url: String) -> Result[String] uses [NetRead] { ?body }";
    assert_eq!(
        toks_no_eof(src),
        vec![
            Token::Fn,
            Token::Ident("fetch".into()),
            Token::LParen,
            Token::Ident("url".into()),
            Token::Colon,
            Token::Ident("String".into()),
            Token::RParen,
            Token::Arrow,
            Token::Ident("Result".into()),
            Token::LBracket,
            Token::Ident("String".into()),
            Token::RBracket,
            Token::Uses,
            Token::LBracket,
            Token::Ident("NetRead".into()),
            Token::RBracket,
            Token::LBrace,
            Token::Question,
            Token::Ident("body".into()),
            Token::RBrace,
        ]
    );
}

#[test]
fn test_match_expression() {
    let src = "match x { 0 => true, _ => false }";
    assert_eq!(
        toks_no_eof(src),
        vec![
            Token::Match,
            Token::Ident("x".into()),
            Token::LBrace,
            Token::Int(0),
            Token::FatArrow,
            Token::Bool(true),
            Token::Comma,
            Token::Ident("_".into()),
            Token::FatArrow,
            Token::Bool(false),
            Token::RBrace,
        ]
    );
}

#[test]
fn test_pipe_chain() {
    let src = "data |> map(f) |> filter(g)";
    assert_eq!(
        toks_no_eof(src),
        vec![
            Token::Ident("data".into()),
            Token::PipeArrow,
            Token::Ident("map".into()),
            Token::LParen,
            Token::Ident("f".into()),
            Token::RParen,
            Token::PipeArrow,
            Token::Ident("filter".into()),
            Token::LParen,
            Token::Ident("g".into()),
            Token::RParen,
        ]
    );
}

#[test]
fn test_lambda() {
    let src = "|x| x + 1";
    assert_eq!(
        toks_no_eof(src),
        vec![
            Token::Pipe,
            Token::Ident("x".into()),
            Token::Pipe,
            Token::Ident("x".into()),
            Token::Plus,
            Token::Int(1),
        ]
    );
}

// ── Raw strings ──────────────────────────────────────────────────────────

#[test]
fn test_raw_string_no_escapes() {
    assert_eq!(
        toks_no_eof(r#"r"C:\Users\path\to\file""#),
        vec![Token::Str(r"C:\Users\path\to\file".into())]
    );
}

#[test]
fn test_raw_string_preserves_backslash_n() {
    assert_eq!(
        toks_no_eof(r#"r"hello\nworld""#),
        vec![Token::Str(r"hello\nworld".into())]
    );
}

#[test]
fn test_raw_string_empty() {
    assert_eq!(toks_no_eof(r#"r"""#), vec![Token::Str("".into())]);
}

// ── F-strings ────────────────────────────────────────────────────────────

#[test]
fn test_fstring_no_interpolation() {
    assert_eq!(
        toks_no_eof(r#"f"hello world""#),
        vec![Token::FStr(vec![TemplatePart::Lit("hello world".into())])]
    );
}

#[test]
fn test_fstring_simple_interpolation() {
    assert_eq!(
        toks_no_eof(r#"f"Hello {name}!""#),
        vec![Token::FStr(vec![
            TemplatePart::Lit("Hello ".into()),
            TemplatePart::Expr("name".into()),
            TemplatePart::Lit("!".into()),
        ])]
    );
}

#[test]
fn test_fstring_multiple_exprs() {
    assert_eq!(
        toks_no_eof(r#"f"{a} + {b} = {c}""#),
        vec![Token::FStr(vec![
            TemplatePart::Expr("a".into()),
            TemplatePart::Lit(" + ".into()),
            TemplatePart::Expr("b".into()),
            TemplatePart::Lit(" = ".into()),
            TemplatePart::Expr("c".into()),
        ])]
    );
}

// ── T-strings ────────────────────────────────────────────────────────────

#[test]
fn test_tstring_simple() {
    assert_eq!(
        toks_no_eof(r#"t"Dear {customer}, order #{id}""#),
        vec![Token::TStr(vec![
            TemplatePart::Lit("Dear ".into()),
            TemplatePart::Expr("customer".into()),
            TemplatePart::Lit(", order #".into()),
            TemplatePart::Expr("id".into()),
        ])]
    );
}

#[test]
fn test_tstring_no_interpolation() {
    assert_eq!(
        toks_no_eof(r#"t"plain text""#),
        vec![Token::TStr(vec![TemplatePart::Lit("plain text".into())])]
    );
}

// ── Foreign keyword ──────────────────────────────────────────────────────

#[test]
fn test_foreign_keyword() {
    assert_eq!(
        toks_no_eof("foreign fn malloc"),
        vec![Token::Foreign, Token::Fn, Token::Ident("malloc".into())]
    );
}
