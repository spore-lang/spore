use proptest::prelude::*;
use sporec_parser::{
    formatter::format_module,
    lexer::{Lexer, Token},
    parse,
};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn arbitrary_utf8_is_rejected_or_tokenized_without_panicking(source in ".{0,256}") {
        if let Ok(tokens) = Lexer::new(&source).tokenize() {
            prop_assert_eq!(&tokens.last().unwrap().node, &Token::Eof);
            for token in tokens.iter().filter(|token| token.node != Token::Eof) {
                prop_assert!(token.span.start <= token.span.end);
                prop_assert!(token.span.end <= source.len());
                prop_assert!(source.is_char_boundary(token.span.start));
                prop_assert!(source.is_char_boundary(token.span.end));
            }
        }
        let _ = parse(&source);
    }

    #[test]
    fn formatting_preserves_raw_string_values(value in "[^\"]{0,128}") {
        let source = format!("fn text() -> Str {{ r\"{value}\" }}");
        let module = parse(&source).unwrap();
        let formatted = format_module(&module);
        let tokens = Lexer::new(&formatted).tokenize().unwrap();
        prop_assert!(tokens.iter().any(|token| token.node == Token::Str(value.clone())));
        let reparsed = parse(&formatted).unwrap();
        prop_assert_eq!(format_module(&reparsed), formatted);
    }

    #[test]
    fn formatting_integer_functions_is_idempotent(value in 0_i64..=i64::MAX) {
        let source = format!("fn value( )->I64{{{value}}}");
        let formatted = format_module(&parse(&source).unwrap());
        let reparsed = parse(&formatted).unwrap();
        prop_assert_eq!(format_module(&reparsed), formatted);
    }
}
