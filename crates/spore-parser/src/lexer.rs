//! Hand-written lexer for the Spore language.

use crate::error::LexError;

/// Byte-offset span in source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn point(pos: usize) -> Self {
        Self {
            start: pos,
            end: pos + 1,
        }
    }
}

/// A value annotated with its source span.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

/// Spore token types.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // ── Literals ──
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),

    // ── Identifier ──
    Ident(String),

    // ── Keywords ──
    Fn,
    Let,
    If,
    Else,
    Match,
    Return,
    Pub,
    Struct,
    Type,
    Capability,
    Import,
    As,
    Spawn,
    Await,
    Where,
    Cost,
    Uses,
    Throw,
    Select,
    Mod,
    Pkg,
    In,
    Self_,
    Impl,
    ParallelScope,

    // ── Operators ──
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    AndAnd,
    OrOr,
    Bang,
    Amp,
    Pipe,
    Caret,
    Tilde,
    Shl,
    Shr,
    Eq,
    PipeArrow,
    Question,
    Arrow,
    FatArrow,
    DotDot,
    DotDotEq,
    /// Unicode `≤` (for `cost ≤` expressions)
    Le2,

    // ── Delimiters ──
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,

    // ── Punctuation ──
    Comma,
    Colon,
    ColonColon,
    Semicolon,
    Dot,
    At,
    Hash,

    // ── Special ──
    Eof,
}

// ── Lexer ────────────────────────────────────────────────────────────────────

pub struct Lexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            pos: 0,
        }
    }

    /// Tokenise the entire source, returning all tokens ending with `Eof`.
    pub fn tokenize(&mut self) -> Result<Vec<Spanned<Token>>, Vec<LexError>> {
        let mut tokens = Vec::new();
        let mut errors = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.source.len() {
                tokens.push(Spanned::new(Token::Eof, Span::point(self.pos)));
                break;
            }
            match self.next_token() {
                Ok(tok) => tokens.push(tok),
                Err(e) => errors.push(e),
            }
        }

        if errors.is_empty() {
            Ok(tokens)
        } else {
            Err(errors)
        }
    }

    // ── helpers ──────────────────────────────────────────────────────────

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<u8> {
        self.bytes.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> u8 {
        let b = self.bytes[self.pos];
        self.pos += 1;
        b
    }

    fn eat(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn remaining(&self) -> &'a str {
        &self.source[self.pos..]
    }

    // ── whitespace / comments ───────────────────────────────────────────

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // whitespace
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
            // line comment
            if self.remaining().starts_with("//") {
                while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
                    self.pos += 1;
                }
                continue;
            }
            // block comment (nestable)
            if self.remaining().starts_with("/*") {
                self.pos += 2;
                let mut depth = 1u32;
                while self.pos + 1 < self.bytes.len() && depth > 0 {
                    if self.bytes[self.pos] == b'/' && self.bytes[self.pos + 1] == b'*' {
                        depth += 1;
                        self.pos += 2;
                    } else if self.bytes[self.pos] == b'*' && self.bytes[self.pos + 1] == b'/' {
                        depth -= 1;
                        self.pos += 2;
                    } else {
                        self.pos += 1;
                    }
                }
                continue;
            }
            break;
        }
    }

    // ── main dispatch ───────────────────────────────────────────────────

    fn next_token(&mut self) -> Result<Spanned<Token>, LexError> {
        let start = self.pos;

        // Check for Unicode ≤ (UTF-8: E2 89 A4)
        if self.remaining().starts_with('≤') {
            self.pos += '≤'.len_utf8();
            return Ok(Spanned::new(Token::Le2, Span::new(start, self.pos)));
        }

        let b = self.advance();

        match b {
            // ── single-char tokens ──
            b'(' => Ok(Spanned::new(Token::LParen, Span::new(start, self.pos))),
            b')' => Ok(Spanned::new(Token::RParen, Span::new(start, self.pos))),
            b'{' => Ok(Spanned::new(Token::LBrace, Span::new(start, self.pos))),
            b'}' => Ok(Spanned::new(Token::RBrace, Span::new(start, self.pos))),
            b'[' => Ok(Spanned::new(Token::LBracket, Span::new(start, self.pos))),
            b']' => Ok(Spanned::new(Token::RBracket, Span::new(start, self.pos))),
            b',' => Ok(Spanned::new(Token::Comma, Span::new(start, self.pos))),
            b';' => Ok(Spanned::new(Token::Semicolon, Span::new(start, self.pos))),
            b'@' => Ok(Spanned::new(Token::At, Span::new(start, self.pos))),
            b'#' => Ok(Spanned::new(Token::Hash, Span::new(start, self.pos))),
            b'~' => Ok(Spanned::new(Token::Tilde, Span::new(start, self.pos))),
            b'?' => Ok(Spanned::new(Token::Question, Span::new(start, self.pos))),
            b'+' => Ok(Spanned::new(Token::Plus, Span::new(start, self.pos))),
            b'*' => Ok(Spanned::new(Token::Star, Span::new(start, self.pos))),
            b'^' => Ok(Spanned::new(Token::Caret, Span::new(start, self.pos))),
            b'%' => Ok(Spanned::new(Token::Percent, Span::new(start, self.pos))),

            // ── multi-char starting with `.` ──
            b'.' => {
                if self.eat(b'.') {
                    if self.eat(b'=') {
                        Ok(Spanned::new(Token::DotDotEq, Span::new(start, self.pos)))
                    } else {
                        Ok(Spanned::new(Token::DotDot, Span::new(start, self.pos)))
                    }
                } else {
                    Ok(Spanned::new(Token::Dot, Span::new(start, self.pos)))
                }
            }

            // ── multi-char starting with `:` ──
            b':' => {
                if self.eat(b':') {
                    Ok(Spanned::new(Token::ColonColon, Span::new(start, self.pos)))
                } else {
                    Ok(Spanned::new(Token::Colon, Span::new(start, self.pos)))
                }
            }

            // ── multi-char starting with `-` ──
            b'-' => {
                if self.eat(b'>') {
                    Ok(Spanned::new(Token::Arrow, Span::new(start, self.pos)))
                } else {
                    Ok(Spanned::new(Token::Minus, Span::new(start, self.pos)))
                }
            }

            // ── multi-char starting with `=` ──
            b'=' => {
                if self.eat(b'=') {
                    Ok(Spanned::new(Token::EqEq, Span::new(start, self.pos)))
                } else if self.eat(b'>') {
                    Ok(Spanned::new(Token::FatArrow, Span::new(start, self.pos)))
                } else {
                    Ok(Spanned::new(Token::Eq, Span::new(start, self.pos)))
                }
            }

            // ── multi-char starting with `!` ──
            b'!' => {
                if self.eat(b'=') {
                    Ok(Spanned::new(Token::NotEq, Span::new(start, self.pos)))
                } else {
                    Ok(Spanned::new(Token::Bang, Span::new(start, self.pos)))
                }
            }

            // ── multi-char starting with `<` ──
            b'<' => {
                if self.eat(b'=') {
                    Ok(Spanned::new(Token::LtEq, Span::new(start, self.pos)))
                } else if self.eat(b'<') {
                    Ok(Spanned::new(Token::Shl, Span::new(start, self.pos)))
                } else {
                    Ok(Spanned::new(Token::Lt, Span::new(start, self.pos)))
                }
            }

            // ── multi-char starting with `>` ──
            b'>' => {
                if self.eat(b'=') {
                    Ok(Spanned::new(Token::GtEq, Span::new(start, self.pos)))
                } else if self.eat(b'>') {
                    Ok(Spanned::new(Token::Shr, Span::new(start, self.pos)))
                } else {
                    Ok(Spanned::new(Token::Gt, Span::new(start, self.pos)))
                }
            }

            // ── multi-char starting with `&` ──
            b'&' => {
                if self.eat(b'&') {
                    Ok(Spanned::new(Token::AndAnd, Span::new(start, self.pos)))
                } else {
                    Ok(Spanned::new(Token::Amp, Span::new(start, self.pos)))
                }
            }

            // ── multi-char starting with `|` ──
            b'|' => {
                if self.eat(b'|') {
                    Ok(Spanned::new(Token::OrOr, Span::new(start, self.pos)))
                } else if self.eat(b'>') {
                    Ok(Spanned::new(Token::PipeArrow, Span::new(start, self.pos)))
                } else {
                    Ok(Spanned::new(Token::Pipe, Span::new(start, self.pos)))
                }
            }

            // ── slash (already handled // and /* above, so this is division) ──
            b'/' => Ok(Spanned::new(Token::Slash, Span::new(start, self.pos))),

            // ── string literal ──
            b'"' => self.read_string(start),

            // ── number literal ──
            b'0'..=b'9' => self.read_number(start),

            // ── identifier or keyword ──
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.read_ident_or_keyword(start),

            _ => {
                // Try to skip unknown UTF-8 char
                let ch = self.source[start..].chars().next().unwrap();
                self.pos = start + ch.len_utf8();
                Err(LexError {
                    message: format!("unexpected character: '{ch}'"),
                    span: Span::new(start, self.pos),
                })
            }
        }
    }

    // ── number ──────────────────────────────────────────────────────────

    fn read_number(&mut self, start: usize) -> Result<Spanned<Token>, LexError> {
        // Check for 0x, 0b, 0o prefixes
        if self.bytes[start] == b'0'
            && let Some(prefix) = self.peek()
        {
            match prefix {
                b'x' | b'X' => {
                    self.pos += 1; // skip 'x'
                    while let Some(b) = self.peek() {
                        if b.is_ascii_hexdigit() || b == b'_' {
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                    let text = &self.source[start + 2..self.pos];
                    let text = text.replace('_', "");
                    let val = i64::from_str_radix(&text, 16).map_err(|e| LexError {
                        message: format!("invalid hex literal: {e}"),
                        span: Span::new(start, self.pos),
                    })?;
                    return Ok(Spanned::new(Token::Int(val), Span::new(start, self.pos)));
                }
                b'b' | b'B' => {
                    self.pos += 1;
                    while let Some(b) = self.peek() {
                        if b == b'0' || b == b'1' || b == b'_' {
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                    let text = &self.source[start + 2..self.pos];
                    let text = text.replace('_', "");
                    let val = i64::from_str_radix(&text, 2).map_err(|e| LexError {
                        message: format!("invalid binary literal: {e}"),
                        span: Span::new(start, self.pos),
                    })?;
                    return Ok(Spanned::new(Token::Int(val), Span::new(start, self.pos)));
                }
                b'o' | b'O' => {
                    self.pos += 1;
                    while let Some(b) = self.peek() {
                        if (b'0'..=b'7').contains(&b) || b == b'_' {
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                    let text = &self.source[start + 2..self.pos];
                    let text = text.replace('_', "");
                    let val = i64::from_str_radix(&text, 8).map_err(|e| LexError {
                        message: format!("invalid octal literal: {e}"),
                        span: Span::new(start, self.pos),
                    })?;
                    return Ok(Spanned::new(Token::Int(val), Span::new(start, self.pos)));
                }
                _ => {}
            }
        }

        // decimal integer / float
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }

        let mut is_float = false;
        // Check for `.` that is NOT `..`
        if self.peek() == Some(b'.') && self.peek2() != Some(b'.') {
            // also avoid treating method calls as floats: 42.to_string()
            if self.peek2().is_some_and(|b| b.is_ascii_digit()) {
                is_float = true;
                self.pos += 1; // skip '.'
                while let Some(b) = self.peek() {
                    if b.is_ascii_digit() || b == b'_' {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
            }
        }

        let text = &self.source[start..self.pos];
        let text_clean = text.replace('_', "");
        if is_float {
            let val: f64 = text_clean.parse().map_err(|e| LexError {
                message: format!("invalid float literal: {e}"),
                span: Span::new(start, self.pos),
            })?;
            Ok(Spanned::new(Token::Float(val), Span::new(start, self.pos)))
        } else {
            let val: i64 = text_clean.parse().map_err(|e| LexError {
                message: format!("invalid integer literal: {e}"),
                span: Span::new(start, self.pos),
            })?;
            Ok(Spanned::new(Token::Int(val), Span::new(start, self.pos)))
        }
    }

    // ── string ──────────────────────────────────────────────────────────

    fn read_string(&mut self, start: usize) -> Result<Spanned<Token>, LexError> {
        let mut buf = String::new();

        loop {
            match self.peek() {
                None => {
                    return Err(LexError {
                        message: "unterminated string literal".into(),
                        span: Span::new(start, self.pos),
                    });
                }
                Some(b'"') => {
                    self.pos += 1; // closing quote
                    break;
                }
                Some(b'\\') => {
                    self.pos += 1;
                    match self.peek() {
                        Some(b'n') => {
                            buf.push('\n');
                            self.pos += 1;
                        }
                        Some(b't') => {
                            buf.push('\t');
                            self.pos += 1;
                        }
                        Some(b'r') => {
                            buf.push('\r');
                            self.pos += 1;
                        }
                        Some(b'\\') => {
                            buf.push('\\');
                            self.pos += 1;
                        }
                        Some(b'"') => {
                            buf.push('"');
                            self.pos += 1;
                        }
                        Some(b'0') => {
                            buf.push('\0');
                            self.pos += 1;
                        }
                        _ => {
                            return Err(LexError {
                                message: "invalid escape sequence".into(),
                                span: Span::new(self.pos - 1, self.pos + 1),
                            });
                        }
                    }
                }
                Some(_) => {
                    // Handle arbitrary UTF-8 chars
                    let ch = self.source[self.pos..].chars().next().unwrap();
                    buf.push(ch);
                    self.pos += ch.len_utf8();
                }
            }
        }

        Ok(Spanned::new(Token::Str(buf), Span::new(start, self.pos)))
    }

    // ── ident / keyword ─────────────────────────────────────────────────

    fn read_ident_or_keyword(&mut self, start: usize) -> Result<Spanned<Token>, LexError> {
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }

        let text = &self.source[start..self.pos];
        let span = Span::new(start, self.pos);

        let tok = match text {
            "fn" => Token::Fn,
            "let" => Token::Let,
            "if" => Token::If,
            "else" => Token::Else,
            "match" => Token::Match,
            "return" => Token::Return,
            "pub" => Token::Pub,
            "struct" => Token::Struct,
            "type" => Token::Type,
            "capability" => Token::Capability,
            "import" => Token::Import,
            "as" => Token::As,
            "spawn" => Token::Spawn,
            "await" => Token::Await,
            "where" => Token::Where,
            "cost" => Token::Cost,
            "uses" => Token::Uses,
            "throw" => Token::Throw,
            "select" => Token::Select,
            "mod" => Token::Mod,
            "pkg" => Token::Pkg,
            "in" => Token::In,
            "self" => Token::Self_,
            "impl" => Token::Impl,
            "parallel_scope" => Token::ParallelScope,
            "true" => Token::Bool(true),
            "false" => Token::Bool(false),
            _ => Token::Ident(text.to_string()),
        };

        Ok(Spanned::new(tok, span))
    }
}
