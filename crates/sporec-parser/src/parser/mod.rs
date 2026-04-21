//! Recursive-descent + Pratt parser for the Spore language.
//!
//! Produces AST nodes defined in [`crate::ast`].

mod expr;
mod item;
mod pattern;
mod ty;

use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::{Comment, Span, Spanned, Token};

/// Binding power for infix operators (left, right).
fn infix_bp(tok: &Token) -> Option<(u8, u8)> {
    Some(match tok {
        Token::PipeArrow => (2, 3),
        Token::OrOr => (4, 5),
        Token::AndAnd => (6, 7),
        Token::Pipe => (8, 9),
        Token::Caret => (10, 11),
        Token::Amp => (12, 13),
        Token::EqEq | Token::NotEq => (14, 15),
        Token::Lt | Token::Gt | Token::LtEq | Token::GtEq => (16, 17),
        Token::Shl | Token::Shr => (18, 19),
        Token::Plus | Token::Minus => (20, 21),
        Token::Star | Token::Slash | Token::Percent => (22, 23),
        Token::DotDot | Token::DotDotEq => (24, 25),
        _ => return None,
    })
}

fn prefix_bp(tok: &Token) -> Option<u8> {
    Some(match tok {
        Token::Minus | Token::Bang | Token::Tilde => 26,
        _ => return None,
    })
}

fn token_to_binop(tok: &Token) -> Option<BinOp> {
    Some(match tok {
        Token::Plus => BinOp::Add,
        Token::Minus => BinOp::Sub,
        Token::Star => BinOp::Mul,
        Token::Slash => BinOp::Div,
        Token::Percent => BinOp::Mod,
        Token::EqEq => BinOp::Eq,
        Token::NotEq => BinOp::Ne,
        Token::Lt => BinOp::Lt,
        Token::Gt => BinOp::Gt,
        Token::LtEq => BinOp::Le,
        Token::GtEq => BinOp::Ge,
        Token::AndAnd => BinOp::And,
        Token::OrOr => BinOp::Or,
        Token::Amp => BinOp::BitAnd,
        Token::Caret => BinOp::BitXor,
        Token::Shl => BinOp::Shl,
        Token::Shr => BinOp::Shr,
        _ => return None,
    })
}

fn token_to_unaryop(tok: &Token) -> Option<UnaryOp> {
    Some(match tok {
        Token::Minus => UnaryOp::Neg,
        Token::Bang => UnaryOp::Not,
        Token::Tilde => UnaryOp::BitNot,
        _ => return None,
    })
}

/// If any argument is `Expr::Placeholder`, rewrite the call into a lambda:
///
///   `f(a, _, c, _)` → `|_p0: _, _p1: _| f(a, _p0, c, _p1)`
///
/// Only inspects the immediate argument list (not nested calls).
fn desugar_placeholder_call(callee: Box<Expr>, args: Vec<Expr>) -> Expr {
    let has_placeholder = args.iter().any(|a| matches!(a, Expr::Placeholder));
    if !has_placeholder {
        return Expr::Call(callee, args);
    }

    let mut counter = 0usize;
    let mut params = Vec::new();
    let new_args: Vec<Expr> = args
        .into_iter()
        .map(|a| {
            if matches!(a, Expr::Placeholder) {
                let name = format!("_p{counter}");
                counter += 1;
                params.push(Param {
                    name: name.clone(),
                    ty: TypeExpr::Named("_".to_string()),
                });
                Expr::Var(name)
            } else {
                a
            }
        })
        .collect();

    Expr::Lambda(params, Box::new(Expr::Call(callee, new_args)))
}

pub struct Parser {
    tokens: Vec<Spanned<Token>>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Spanned<Token>>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos].node
    }

    fn peek_span(&self) -> Span {
        self.tokens[self.pos].span
    }

    fn previous_span(&self) -> Span {
        if self.pos > 0 {
            self.tokens[self.pos - 1].span
        } else {
            Span { start: 0, end: 0 }
        }
    }

    fn advance(&mut self) -> &Spanned<Token> {
        let t = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    fn at(&self, tok: &Token) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(tok)
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), Token::Eof)
    }

    fn expect(&mut self, expected: &Token) -> Result<Span, ParseError> {
        if self.at(expected) {
            let span = self.peek_span();
            self.advance();
            Ok(span)
        } else {
            Err(self.error(format!("expected {expected:?}, found {:?}", self.peek())))
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.peek().clone() {
            Token::Ident(s) => {
                self.advance();
                Ok(s)
            }
            Token::Self_ => {
                self.advance();
                Ok("self".into())
            }
            _ => Err(self.error(format!("expected identifier, found {:?}", self.peek()))),
        }
    }

    fn at_contextual_ident(&self, expected: &str) -> bool {
        matches!(self.peek(), Token::Ident(name) if name == expected)
    }

    fn parse_qualified_ident(&mut self) -> Result<String, ParseError> {
        let mut name = self.expect_ident()?;
        while self.at(&Token::Dot) {
            self.advance();
            let seg = self.expect_ident()?;
            name = format!("{name}.{seg}");
        }
        Ok(name)
    }

    fn error(&self, message: String) -> ParseError {
        ParseError {
            message,
            span: self.peek_span(),
        }
    }

    pub fn parse_module(&mut self) -> Result<Module, ParseError> {
        self.parse_module_with_comments(Vec::new())
    }

    /// Parse a module and attach pre-collected comments.
    pub fn parse_module_with_comments(
        &mut self,
        comments: Vec<Comment>,
    ) -> Result<Module, ParseError> {
        if self.at(&Token::Mod) {
            return Err(self.error(
                "module declarations are not supported; module names are derived from file paths"
                    .to_string(),
            ));
        }

        let mut items = Vec::new();
        while !self.at_eof() {
            items.push(self.parse_item()?);
        }
        Ok(Module {
            name: String::new(),
            items,
            comments,
        })
    }

    /// Parse a comma-separated list. `end` is the closing delimiter (not consumed).
    fn parse_comma_sep<T>(
        &mut self,
        mut parse_one: impl FnMut(&mut Self) -> Result<T, ParseError>,
        end: &Token,
    ) -> Result<Vec<T>, ParseError> {
        let mut items = Vec::new();
        while !self.at(end) && !self.at_eof() {
            items.push(parse_one(self)?);
            if !self.at(&Token::Comma) {
                break;
            }
            self.advance();
        }
        Ok(items)
    }
}
