use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::Token;

use super::Parser;

impl Parser {
    pub(super) fn parse_type_expr(&mut self) -> Result<TypeExpr, ParseError> {
        let base = self.parse_type_expr_base()?;
        if self.at(&Token::When) {
            self.advance();
            let pred = self.parse_expr()?;
            Ok(TypeExpr::Refinement(
                Box::new(base),
                "self".into(),
                Box::new(pred),
            ))
        } else {
            Ok(base)
        }
    }

    fn parse_type_expr_base(&mut self) -> Result<TypeExpr, ParseError> {
        match self.peek().clone() {
            Token::Self_ => {
                self.advance();
                Ok(TypeExpr::Named("Self".into()))
            }
            Token::Question => {
                self.advance();
                let name = if matches!(self.peek(), Token::Ident(_)) {
                    Some(self.expect_ident()?)
                } else {
                    None
                };
                Ok(TypeExpr::Hole(name))
            }
            Token::Ident(name) => {
                self.advance();
                if self.at(&Token::LBracket) {
                    self.advance();
                    let args = self.parse_comma_sep(|p| p.parse_type_expr(), &Token::RBracket)?;
                    self.expect(&Token::RBracket)?;
                    Ok(TypeExpr::Generic(name, args))
                } else {
                    Ok(TypeExpr::Named(name))
                }
            }
            Token::LParen => {
                self.advance();
                let types = self.parse_comma_sep(|p| p.parse_type_expr(), &Token::RParen)?;
                self.expect(&Token::RParen)?;
                if self.at(&Token::Arrow) {
                    self.advance();
                    let ret = self.parse_type_expr()?;
                    let errors = if self.at(&Token::Bang) {
                        self.advance();
                        let mut errs = vec![self.parse_type_expr()?];
                        while self.at(&Token::Pipe) {
                            self.advance();
                            errs.push(self.parse_type_expr()?);
                        }
                        errs
                    } else {
                        vec![]
                    };
                    Ok(TypeExpr::Function(types, Box::new(ret), errors))
                } else {
                    Ok(TypeExpr::Tuple(types))
                }
            }
            Token::LBrace => {
                self.advance();
                let fields = self.parse_comma_sep(
                    |p| {
                        let name = p.expect_ident()?;
                        p.expect(&Token::Colon)?;
                        let ty = p.parse_type_expr()?;
                        Ok((name, ty))
                    },
                    &Token::RBrace,
                )?;
                self.expect(&Token::RBrace)?;
                Ok(TypeExpr::Record(fields))
            }
            _ => Err(self.error(format!("expected type, found {:?}", self.peek()))),
        }
    }
}
