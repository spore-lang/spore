use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::{TemplatePart, Token};

use super::{
    Parser, desugar_placeholder_call, infix_bp, prefix_bp, token_to_binop, token_to_unaryop,
};

impl Parser {
    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_expr_bp(0)
    }

    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            if self.at_expr_terminator() {
                break;
            }
            loop {
                if self.at_expr_terminator() {
                    break;
                }
                match self.peek() {
                    Token::Question => {
                        self.advance();
                        lhs = Expr::Try(Box::new(lhs));
                    }
                    Token::Dot => {
                        self.advance();
                        let field = if self.at(&Token::Await) {
                            self.advance();
                            "await".to_string()
                        } else {
                            self.expect_ident()?
                        };
                        if field == "await" && !self.at(&Token::LParen) {
                            lhs = Expr::Await(Box::new(lhs));
                            continue;
                        }
                        if field == "new"
                            && matches!(&lhs, Expr::Var(name) if name == "Channel")
                            && self.at(&Token::LBracket)
                        {
                            lhs = self.parse_channel_new_expr()?;
                            continue;
                        }
                        if self.at(&Token::LParen) {
                            self.advance();
                            let args = self.parse_comma_sep(|p| p.parse_expr(), &Token::RParen)?;
                            self.expect(&Token::RParen)?;
                            let callee = Expr::FieldAccess(Box::new(lhs), field);
                            lhs = desugar_placeholder_call(Box::new(callee), args);
                        } else {
                            lhs = Expr::FieldAccess(Box::new(lhs), field);
                        }
                    }
                    Token::LParen => {
                        self.advance();
                        let args = self.parse_comma_sep(|p| p.parse_expr(), &Token::RParen)?;
                        self.expect(&Token::RParen)?;
                        lhs = desugar_placeholder_call(Box::new(lhs), args);
                    }
                    Token::LBracket => {
                        break;
                    }
                    _ => break,
                }
            }

            if self.at_expr_terminator() {
                break;
            }

            if let Token::PipeArrow = self.peek() {
                let (l_bp, r_bp) = (2, 3);
                if l_bp < min_bp {
                    break;
                }
                self.advance();
                let rhs = self.parse_expr_bp(r_bp)?;
                lhs = Expr::Pipe(Box::new(lhs), Box::new(rhs));
                continue;
            }

            if let Some((l_bp, r_bp)) = infix_bp(self.peek()) {
                if l_bp < min_bp {
                    break;
                }
                let op_tok = self.peek().clone();

                if matches!(op_tok, Token::Pipe) {
                    self.advance();
                    let rhs = self.parse_expr_bp(r_bp)?;
                    lhs = Expr::BinOp(Box::new(lhs), BinOp::BitOr, Box::new(rhs));
                    continue;
                }

                if let Some(binop) = token_to_binop(&op_tok) {
                    self.advance();
                    let rhs = self.parse_expr_bp(r_bp)?;
                    lhs = Expr::BinOp(Box::new(lhs), binop, Box::new(rhs));
                    continue;
                }
                break;
            }

            break;
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr, ParseError> {
        match self.peek().clone() {
            tok if prefix_bp(&tok).is_some() => {
                let bp = prefix_bp(&tok).expect("prefix_bp: unreachable - token already matched");
                let op = token_to_unaryop(&tok)
                    .expect("token_to_unaryop: unreachable - token already matched");
                self.advance();
                let expr = self.parse_expr_bp(bp)?;
                Ok(Expr::UnaryOp(op, Box::new(expr)))
            }
            Token::Int(n) => {
                self.advance();
                Ok(Expr::IntLit(n))
            }
            Token::SuffixedInt(n, suffix) => {
                self.advance();
                Ok(Expr::SuffixedIntLit(n, suffix))
            }
            Token::Float(f) => {
                self.advance();
                Ok(Expr::FloatLit(f))
            }
            Token::Str(s) => {
                self.advance();
                Ok(Expr::StrLit(s))
            }
            Token::FStr(parts) => {
                self.advance();
                self.expand_template_parts(&parts, true)
            }
            Token::TStr(parts) => {
                self.advance();
                self.expand_template_parts(&parts, false)
            }
            Token::Bool(b) => {
                self.advance();
                Ok(Expr::BoolLit(b))
            }
            Token::LBrace => self.parse_block_expr(),
            Token::LParen => {
                self.advance();
                if self.at(&Token::RParen) {
                    self.advance();
                    return Ok(Expr::Unit);
                }
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Token::If => self.parse_if_expr(),
            Token::Match => self.parse_match_expr(),
            Token::Spawn => {
                self.advance();
                let expr = self.parse_expr()?;
                Ok(Expr::Spawn(Box::new(expr)))
            }
            Token::Return => {
                self.advance();
                if self.at_eof()
                    || self.at(&Token::RBrace)
                    || self.at(&Token::Semicolon)
                    || self.at(&Token::RParen)
                {
                    Ok(Expr::Return(None))
                } else {
                    let expr = self.parse_expr()?;
                    Ok(Expr::Return(Some(Box::new(expr))))
                }
            }
            Token::Throw => {
                Err(self.error(
                    "`throw` is not part of the current syntax; use `fail error` to construct an outcome failure"
                        .into(),
                ))
            }
            Token::Fail => {
                self.advance();
                let expr = self.parse_expr()?;
                Ok(Expr::Fail(Box::new(expr)))
            }
            Token::LBracket => {
                self.advance();
                let elems = self.parse_comma_sep(|p| p.parse_expr(), &Token::RBracket)?;
                self.expect(&Token::RBracket)?;
                Ok(Expr::List(elems))
            }
            Token::Question => {
                let question_span = self.peek_span();
                self.advance();
                let name = if matches!(self.peek(), Token::Ident(_)) {
                    Some(self.expect_ident()?)
                } else {
                    None
                };
                let ty = if self.at(&Token::Colon) {
                    self.advance();
                    Some(Box::new(self.parse_type_expr()?))
                } else {
                    None
                };
                if self.at(&Token::At) {
                    return Err(self.error(
                        "hole metadata annotations are not part of the current syntax".into(),
                    ));
                }
                let hole_end = self.previous_span().end;
                Ok(Expr::Hole(
                    name,
                    ty,
                    Some(Span::new(question_span.start, hole_end)),
                ))
            }
            Token::Pipe => self.parse_lambda(),
            Token::ParallelScope => {
                self.advance();
                let lanes = if self.at(&Token::LParen) {
                    self.advance();
                    let param_name = self.expect_ident()?;
                    if param_name != "lanes" {
                        return Err(
                            self.error(format!("expected `lanes` parameter, got `{param_name}`"))
                        );
                    }
                    self.expect(&Token::Colon)?;
                    let expr = self.parse_expr()?;
                    self.expect(&Token::RParen)?;
                    Some(Box::new(expr))
                } else {
                    None
                };
                let body = self.parse_block_expr()?;
                Ok(Expr::ParallelScope {
                    lanes,
                    body: Box::new(body),
                })
            }
            Token::Select => self.parse_select_expr(),
            Token::Perform => self.parse_perform_expr(),
            Token::Handle => self.parse_handle_expr(),
            Token::Ident(ref name) if name == "_" => {
                self.advance();
                Ok(Expr::Placeholder)
            }
            Token::Ident(name) => {
                self.advance();
                if self.at(&Token::LBrace) && name.chars().next().is_some_and(|c| c.is_uppercase())
                {
                    self.advance();
                    let fields = self.parse_comma_sep(
                        |p| {
                            let fname = p.expect_ident()?;
                            p.expect(&Token::Colon)?;
                            let val = p.parse_expr()?;
                            Ok((fname, val))
                        },
                        &Token::RBrace,
                    )?;
                    self.expect(&Token::RBrace)?;
                    Ok(Expr::StructLit(name, fields))
                } else {
                    Ok(Expr::Var(name))
                }
            }
            Token::Self_ => {
                self.advance();
                Ok(Expr::Var("self".into()))
            }
            _ => Err(self.error(format!("expected expression, found {:?}", self.peek()))),
        }
    }

    /// Convert lexer-level `TemplatePart`s into `Expr::FString` or
    /// `Expr::TString` by sub-parsing each expression source fragment.
    fn expand_template_parts(
        &self,
        parts: &[TemplatePart],
        is_fstr: bool,
    ) -> Result<Expr, ParseError> {
        if is_fstr {
            let mut ast_parts = Vec::new();
            for part in parts {
                match part {
                    TemplatePart::Lit(s) => ast_parts.push(FStringPart::Literal(s.clone())),
                    TemplatePart::Expr(src) => {
                        ast_parts.push(FStringPart::Expr(self.parse_sub_expr(src)?));
                    }
                }
            }
            Ok(Expr::FString(ast_parts))
        } else {
            let mut ast_parts = Vec::new();
            for part in parts {
                match part {
                    TemplatePart::Lit(s) => ast_parts.push(TStringPart::Literal(s.clone())),
                    TemplatePart::Expr(src) => {
                        ast_parts.push(TStringPart::Expr(self.parse_sub_expr(src)?));
                    }
                }
            }
            Ok(Expr::TString(ast_parts))
        }
    }

    /// Parse a standalone expression from a source fragment (used for
    /// interpolated expressions inside f/t-strings).
    fn parse_sub_expr(&self, src: &str) -> Result<Expr, ParseError> {
        use crate::lexer::Lexer;
        let tokens = Lexer::new(src).tokenize().map_err(|errs| {
            let e = &errs[0];
            ParseError {
                message: e.message.clone(),
                span: e.span,
            }
        })?;
        let mut sub = Parser::new(tokens);
        sub.parse_expr()
    }

    fn parse_lambda(&mut self) -> Result<Expr, ParseError> {
        self.expect(&Token::Pipe)?;
        let params = self.parse_comma_sep(
            |p| {
                let name = p.expect_ident()?;
                let ty = if p.at(&Token::Colon) {
                    p.advance();
                    p.with_expr_terminator(&Token::Pipe, |p| p.parse_type_expr())?
                } else {
                    TypeExpr::Named("_".into())
                };
                Ok(Param { name, ty })
            },
            &Token::Pipe,
        )?;
        self.expect(&Token::Pipe)?;
        let body = self.parse_expr()?;
        Ok(Expr::Lambda(params, Box::new(body)))
    }

    pub(super) fn parse_block_expr(&mut self) -> Result<Expr, ParseError> {
        self.expect(&Token::LBrace)?;
        let mut stmts = Vec::new();
        let mut tail: Option<Box<Expr>> = None;

        while !self.at(&Token::RBrace) && !self.at_eof() {
            if self.at(&Token::Let) {
                stmts.push(self.parse_let_stmt()?);
            } else {
                let expr = self.parse_expr()?;
                if self.at(&Token::Semicolon) {
                    self.advance();
                    stmts.push(Stmt::Expr(expr));
                } else if self.at(&Token::RBrace) {
                    tail = Some(Box::new(expr));
                } else {
                    return Err(self.error(
                        "expected ';' after expression statement (Spore uses Rust-style semicolons: \
                         add ';' to discard the value, or move the expression to the end of the \
                         block as the tail expression)".to_string(),
                    ));
                }
            }
        }

        self.expect(&Token::RBrace)?;
        Ok(Expr::Block(stmts, tail))
    }

    fn parse_let_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.expect(&Token::Let)?;
        let name = self.expect_ident()?;
        let ty = if self.at(&Token::Colon) {
            self.advance();
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        self.expect(&Token::Eq)?;
        let expr = self.parse_expr()?;
        self.expect(&Token::Semicolon)?;
        Ok(Stmt::Let(name, ty, expr))
    }

    fn parse_if_expr(&mut self) -> Result<Expr, ParseError> {
        self.expect(&Token::If)?;
        let cond = self.parse_expr()?;
        let then_branch = self.parse_block_expr()?;
        let else_branch = if self.at(&Token::Else) {
            self.advance();
            if self.at(&Token::If) {
                Some(Box::new(self.parse_if_expr()?))
            } else {
                Some(Box::new(self.parse_block_expr()?))
            }
        } else {
            None
        };
        Ok(Expr::If(Box::new(cond), Box::new(then_branch), else_branch))
    }

    fn parse_match_expr(&mut self) -> Result<Expr, ParseError> {
        self.expect(&Token::Match)?;
        let scrutinee = self.parse_expr()?;
        self.expect(&Token::LBrace)?;
        let mut arms = Vec::new();
        while !self.at(&Token::RBrace) && !self.at_eof() {
            let pattern = self.parse_pattern()?;
            let guard = if self.at(&Token::If) {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect(&Token::FatArrow)?;
            let body = self.parse_expr()?;
            arms.push(MatchArm {
                pattern,
                guard,
                body,
            });
            if self.at(&Token::Comma) {
                self.advance();
            }
        }
        self.expect(&Token::RBrace)?;
        Ok(Expr::Match(Box::new(scrutinee), arms))
    }

    fn parse_select_expr(&mut self) -> Result<Expr, ParseError> {
        self.expect(&Token::Select)?;
        self.expect(&Token::LBrace)?;
        let mut arms = Vec::new();
        while !self.at(&Token::RBrace) && !self.at_eof() {
            let binding = self.expect_ident()?;
            if binding == "timeout" {
                self.expect(&Token::LParen)?;
                let duration = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                self.expect(&Token::FatArrow)?;
                let body = self.parse_expr()?;
                arms.push(SelectArm::Timeout { duration, body });
            } else {
                self.expect(&Token::From)?;
                let source = self.parse_expr()?;
                self.expect(&Token::FatArrow)?;
                let body = self.parse_expr()?;
                arms.push(SelectArm::Recv {
                    binding,
                    source,
                    body,
                });
            }
            if self.at(&Token::Comma) {
                self.advance();
            }
        }
        self.expect(&Token::RBrace)?;
        Ok(Expr::Select(arms))
    }

    fn parse_channel_new_expr(&mut self) -> Result<Expr, ParseError> {
        self.expect(&Token::LBracket)?;
        let elem_type = self.parse_type_expr()?;
        self.expect(&Token::RBracket)?;
        self.expect(&Token::LParen)?;
        let label = self.expect_ident()?;
        if label != "buffer" {
            return Err(self.error(format!(
                "expected named argument `buffer` in Channel.new, found `{label}`"
            )));
        }
        self.expect(&Token::Colon)?;
        let buffer = self.parse_expr()?;
        if self.at(&Token::Comma) {
            self.advance();
        }
        self.expect(&Token::RParen)?;
        Ok(Expr::ChannelNew {
            elem_type,
            buffer: Box::new(buffer),
        })
    }

    fn parse_perform_expr(&mut self) -> Result<Expr, ParseError> {
        self.expect(&Token::Perform)?;
        let effect = self.expect_ident()?;
        self.expect(&Token::Dot)?;
        let operation = self.expect_ident()?;
        self.expect(&Token::LParen)?;
        let args = self.parse_comma_sep(|p| Ok(Box::new(p.parse_expr()?)), &Token::RParen)?;
        self.expect(&Token::RParen)?;
        Ok(Expr::Perform {
            effect,
            operation,
            args,
        })
    }

    fn parse_handle_expr(&mut self) -> Result<Expr, ParseError> {
        self.expect(&Token::Handle)?;
        let body = self.parse_block_expr()?;
        self.expect(&Token::With)?;
        self.expect(&Token::LBrace)?;
        let mut handlers = Vec::new();
        while !self.at(&Token::RBrace) && !self.at_eof() {
            handlers.push(self.parse_handle_binding()?);
            if self.at(&Token::Comma) {
                self.advance();
            }
        }
        self.expect(&Token::RBrace)?;
        Ok(Expr::Handle {
            body: Box::new(body),
            handlers,
        })
    }

    fn parse_handle_binding(&mut self) -> Result<HandleBinding, ParseError> {
        if self.at_contextual_ident("use") {
            self.advance();
            let handler = self.parse_qualified_ident()?;
            self.expect(&Token::LBrace)?;
            let payload = self.parse_comma_sep(
                |p| {
                    let field = p.expect_ident()?;
                    p.expect(&Token::Colon)?;
                    let value = p.parse_expr()?;
                    Ok((field, value))
                },
                &Token::RBrace,
            )?;
            self.expect(&Token::RBrace)?;
            Ok(HandleBinding::Use(HandlerUse { handler, payload }))
        } else {
            if self.at_contextual_ident("on") {
                self.advance();
            }
            Ok(HandleBinding::On(self.parse_effect_arm()?))
        }
    }

    fn parse_effect_arm(&mut self) -> Result<EffectArm, ParseError> {
        let effect = self.expect_ident()?;
        self.expect(&Token::Dot)?;
        let operation = self.expect_ident()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_comma_sep(|p| p.expect_ident(), &Token::RParen)?;
        self.expect(&Token::RParen)?;
        self.expect(&Token::FatArrow)?;
        let arm_body = self.parse_expr()?;
        Ok(EffectArm {
            effect,
            operation,
            params,
            body: Box::new(arm_body),
        })
    }
}
