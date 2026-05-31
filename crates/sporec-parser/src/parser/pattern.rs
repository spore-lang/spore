use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::Token;

use super::Parser;

impl Parser {
    pub(super) fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        let pat = self.parse_single_pattern()?;
        if self.at(&Token::Pipe) {
            let mut alternatives = vec![pat];
            while self.at(&Token::Pipe) {
                self.advance();
                alternatives.push(self.parse_single_pattern()?);
            }
            Ok(Pattern::Or(alternatives))
        } else {
            Ok(pat)
        }
    }

    fn parse_single_pattern(&mut self) -> Result<Pattern, ParseError> {
        match self.peek().clone() {
            Token::Ident(name) if name == "_" => {
                self.advance();
                Ok(Pattern::Wildcard)
            }
            Token::Ident(name) if name == "ok" => {
                self.advance();
                Ok(Pattern::OutcomeOk(Box::new(self.parse_single_pattern()?)))
            }
            Token::Fail => {
                self.advance();
                Ok(Pattern::OutcomeFail(Box::new(self.parse_single_pattern()?)))
            }
            Token::Ident(name) => {
                self.advance();
                if self.at(&Token::LParen) {
                    self.advance();
                    let fields = self.parse_comma_sep(|p| p.parse_pattern(), &Token::RParen)?;
                    self.expect(&Token::RParen)?;
                    Ok(Pattern::Constructor(name, fields))
                } else if self.at(&Token::LBrace) {
                    self.advance();
                    let fields = self.parse_comma_sep(
                        |p| {
                            let fname = p.expect_ident()?;
                            let pat = if p.at(&Token::Colon) {
                                p.advance();
                                p.parse_pattern()?
                            } else {
                                Pattern::Var(fname.clone())
                            };
                            Ok((fname, pat))
                        },
                        &Token::RBrace,
                    )?;
                    self.expect(&Token::RBrace)?;
                    Ok(Pattern::Struct(name, fields))
                } else {
                    Ok(Pattern::Var(name))
                }
            }
            Token::Int(n) => {
                self.advance();
                Ok(Pattern::IntLit(n))
            }
            Token::Str(s) => {
                self.advance();
                Ok(Pattern::StrLit(s))
            }
            Token::Bool(b) => {
                self.advance();
                Ok(Pattern::BoolLit(b))
            }
            Token::LBracket => {
                self.advance();
                let mut elements = Vec::new();
                let mut rest = None;
                while !self.at(&Token::RBracket) && !self.at_eof() {
                    if self.at(&Token::DotDot) {
                        self.advance();
                        rest = Some(self.expect_ident()?);
                        if self.at(&Token::Comma) {
                            self.advance();
                        }
                        break;
                    }
                    elements.push(self.parse_pattern()?);
                    if !self.at(&Token::Comma) {
                        break;
                    }
                    self.advance();
                }
                self.expect(&Token::RBracket)?;
                Ok(Pattern::List(elements, rest))
            }
            _ => Err(self.error(format!("expected pattern, found {:?}", self.peek()))),
        }
    }
}
