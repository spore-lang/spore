//! Recursive-descent + Pratt parser for the Spore language.
//!
//! Produces AST nodes defined in [`crate::ast`].

use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::{Span, Spanned, Token};

// ── Precedence table (Pratt parsing) ─────────────────────────────────────

/// Binding power for infix operators (left, right).
fn infix_bp(tok: &Token) -> Option<(u8, u8)> {
    Some(match tok {
        // Pipe is lowest
        Token::PipeArrow => (2, 3),
        // Logical OR
        Token::OrOr => (4, 5),
        // Logical AND
        Token::AndAnd => (6, 7),
        // Bitwise OR
        Token::Pipe => (8, 9),
        // Bitwise XOR
        Token::Caret => (10, 11),
        // Bitwise AND
        Token::Amp => (12, 13),
        // Equality
        Token::EqEq | Token::NotEq => (14, 15),
        // Comparison
        Token::Lt | Token::Gt | Token::LtEq | Token::GtEq => (16, 17),
        // Shift
        Token::Shl | Token::Shr => (18, 19),
        // Additive
        Token::Plus | Token::Minus => (20, 21),
        // Multiplicative
        Token::Star | Token::Slash | Token::Percent => (22, 23),
        // Range
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

// ── Parser ───────────────────────────────────────────────────────────────

pub struct Parser {
    tokens: Vec<Spanned<Token>>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Spanned<Token>>) -> Self {
        Self { tokens, pos: 0 }
    }

    // ── Token access ────────────────────────────────────────────────

    fn peek(&self) -> &Token {
        &self.tokens[self.pos].node
    }

    fn peek_span(&self) -> Span {
        self.tokens[self.pos].span
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
            // Allow `self` as an identifier in parameter position
            Token::Self_ => {
                self.advance();
                Ok("self".into())
            }
            _ => Err(self.error(format!("expected identifier, found {:?}", self.peek()))),
        }
    }

    fn error(&self, message: String) -> ParseError {
        ParseError {
            message,
            span: self.peek_span(),
        }
    }

    // ── Top-level: Module ───────────────────────────────────────────

    pub fn parse_module(&mut self) -> Result<Module, ParseError> {
        let mut items = Vec::new();
        while !self.at_eof() {
            items.push(self.parse_item()?);
        }
        Ok(Module {
            name: String::new(),
            items,
        })
    }

    // ── Items ───────────────────────────────────────────────────────

    fn parse_item(&mut self) -> Result<Item, ParseError> {
        match self.peek() {
            Token::Fn | Token::Pub => self.parse_fn_item(),
            Token::Struct => self.parse_struct_item(),
            Token::Type => self.parse_type_item(),
            Token::Capability => self.parse_capability_item(),
            Token::Import => self.parse_import_item(),
            _ => Err(self.error(format!(
                "expected item (fn, pub, struct, type, capability, import), found {:?}",
                self.peek()
            ))),
        }
    }

    fn parse_fn_item(&mut self) -> Result<Item, ParseError> {
        Ok(Item::Function(self.parse_fn_def()?))
    }

    fn parse_fn_def(&mut self) -> Result<FnDef, ParseError> {
        // optional visibility
        let visibility = self.parse_visibility()?;

        self.expect(&Token::Fn)?;
        let name = self.expect_ident()?;

        // params
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;

        // optional return type
        let return_type = if self.at(&Token::Arrow) {
            self.advance();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        // optional errors clause: `throws [E1, E2]`
        let errors = if self.at(&Token::Throw) {
            self.advance();
            self.expect(&Token::LBracket)?;
            let errs = self.parse_comma_sep(|p| p.parse_type_expr(), &Token::RBracket)?;
            self.expect(&Token::RBracket)?;
            errs
        } else {
            vec![]
        };

        // optional where clause
        let where_clause = if self.at(&Token::Where) {
            Some(self.parse_where_clause()?)
        } else {
            None
        };

        // optional cost clause
        let cost_clause = if self.at(&Token::Cost) {
            Some(self.parse_cost_clause()?)
        } else {
            None
        };

        // optional uses clause
        let uses_clause = if self.at(&Token::Uses) {
            Some(self.parse_uses_clause()?)
        } else {
            None
        };

        // body: block or hole
        let body = if self.at(&Token::LBrace) {
            Some(self.parse_block_expr()?)
        } else {
            None
        };

        Ok(FnDef {
            name,
            visibility,
            params,
            return_type,
            errors,
            where_clause,
            cost_clause,
            uses_clause,
            body,
        })
    }

    fn parse_visibility(&mut self) -> Result<Visibility, ParseError> {
        if self.at(&Token::Pub) {
            self.advance();
            // Check for `pub(pkg)`
            if self.at(&Token::LParen) {
                self.advance();
                if self.at(&Token::Pkg) {
                    self.advance();
                    self.expect(&Token::RParen)?;
                    Ok(Visibility::PubPkg)
                } else {
                    Err(self.error("expected `pkg` after `pub(`".into()))
                }
            } else {
                Ok(Visibility::Pub)
            }
        } else {
            Ok(Visibility::Private)
        }
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        self.parse_comma_sep(|p| p.parse_param(), &Token::RParen)
    }

    fn parse_param(&mut self) -> Result<Param, ParseError> {
        let name = self.expect_ident()?;
        self.expect(&Token::Colon)?;
        let ty = self.parse_type_expr()?;
        Ok(Param { name, ty })
    }

    // ── Clauses ─────────────────────────────────────────────────────

    fn parse_where_clause(&mut self) -> Result<WhereClause, ParseError> {
        self.expect(&Token::Where)?;
        let mut constraints = Vec::new();
        loop {
            let type_var = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let bound = self.expect_ident()?;
            constraints.push(TypeConstraint { type_var, bound });
            if !self.at(&Token::Comma) {
                break;
            }
            self.advance(); // eat comma
            // Don't continue if we hit a non-ident (next clause or body)
            if !matches!(self.peek(), Token::Ident(_)) {
                break;
            }
        }
        Ok(WhereClause { constraints })
    }

    fn parse_cost_clause(&mut self) -> Result<CostClause, ParseError> {
        self.expect(&Token::Cost)?;
        // expect `≤` or `<=`
        if self.at(&Token::Le2) || self.at(&Token::LtEq) {
            self.advance();
        } else {
            return Err(self.error("expected `≤` or `<=` after `cost`".into()));
        }
        let bound = self.parse_cost_expr()?;
        Ok(CostClause { bound })
    }

    fn parse_cost_expr(&mut self) -> Result<CostExpr, ParseError> {
        let left = self.parse_cost_atom()?;
        self.parse_cost_expr_rest(left)
    }

    fn parse_cost_expr_rest(&mut self, left: CostExpr) -> Result<CostExpr, ParseError> {
        match self.peek() {
            Token::Plus => {
                self.advance();
                let right = self.parse_cost_atom()?;
                let node = CostExpr::Add(Box::new(left), Box::new(right));
                self.parse_cost_expr_rest(node)
            }
            Token::Star => {
                self.advance();
                let right = self.parse_cost_atom()?;
                let node = CostExpr::Mul(Box::new(left), Box::new(right));
                self.parse_cost_expr_rest(node)
            }
            _ => Ok(left),
        }
    }

    fn parse_cost_atom(&mut self) -> Result<CostExpr, ParseError> {
        match self.peek().clone() {
            Token::Int(n) => {
                self.advance();
                Ok(CostExpr::Literal(n as u64))
            }
            Token::Ident(s) => {
                self.advance();
                Ok(CostExpr::Var(s))
            }
            Token::LParen => {
                self.advance();
                let inner = self.parse_cost_expr()?;
                self.expect(&Token::RParen)?;
                Ok(inner)
            }
            _ => Err(self.error(format!(
                "expected cost expression, found {:?}",
                self.peek()
            ))),
        }
    }

    fn parse_uses_clause(&mut self) -> Result<UsesClause, ParseError> {
        self.expect(&Token::Uses)?;
        self.expect(&Token::LBracket)?;
        let resources = self.parse_comma_sep(|p| p.expect_ident(), &Token::RBracket)?;
        self.expect(&Token::RBracket)?;
        Ok(UsesClause { resources })
    }

    // ── Type expressions ────────────────────────────────────────────

    fn parse_type_expr(&mut self) -> Result<TypeExpr, ParseError> {
        match self.peek().clone() {
            Token::Ident(name) => {
                self.advance();
                // Check for generic params: `List[Int]`
                if self.at(&Token::LBracket) {
                    self.advance();
                    let args =
                        self.parse_comma_sep(|p| p.parse_type_expr(), &Token::RBracket)?;
                    self.expect(&Token::RBracket)?;
                    Ok(TypeExpr::Generic(name, args))
                } else {
                    Ok(TypeExpr::Named(name))
                }
            }
            Token::LParen => {
                self.advance();
                // Tuple or function type
                let types =
                    self.parse_comma_sep(|p| p.parse_type_expr(), &Token::RParen)?;
                self.expect(&Token::RParen)?;
                if self.at(&Token::Arrow) {
                    self.advance();
                    let ret = self.parse_type_expr()?;
                    Ok(TypeExpr::Function(types, Box::new(ret)))
                } else {
                    Ok(TypeExpr::Tuple(types))
                }
            }
            _ => Err(self.error(format!("expected type, found {:?}", self.peek()))),
        }
    }

    // ── Struct definition ───────────────────────────────────────────

    fn parse_struct_item(&mut self) -> Result<Item, ParseError> {
        self.expect(&Token::Struct)?;
        let name = self.expect_ident()?;

        // optional type params
        let type_params = if self.at(&Token::LBracket) {
            self.advance();
            let ps = self.parse_comma_sep(|p| p.expect_ident(), &Token::RBracket)?;
            self.expect(&Token::RBracket)?;
            ps
        } else {
            vec![]
        };

        self.expect(&Token::LBrace)?;
        let fields = self.parse_comma_sep(
            |p| {
                let fname = p.expect_ident()?;
                p.expect(&Token::Colon)?;
                let ty = p.parse_type_expr()?;
                Ok(FieldDef { name: fname, ty })
            },
            &Token::RBrace,
        )?;
        self.expect(&Token::RBrace)?;

        Ok(Item::StructDef(StructDef {
            name,
            visibility: Visibility::Private,
            type_params,
            fields,
            implements: vec![],
        }))
    }

    // ── Type (enum) definition ──────────────────────────────────────

    fn parse_type_item(&mut self) -> Result<Item, ParseError> {
        self.expect(&Token::Type)?;
        let name = self.expect_ident()?;

        let type_params = if self.at(&Token::LBracket) {
            self.advance();
            let ps = self.parse_comma_sep(|p| p.expect_ident(), &Token::RBracket)?;
            self.expect(&Token::RBracket)?;
            ps
        } else {
            vec![]
        };

        self.expect(&Token::LBrace)?;
        let mut variants = Vec::new();
        while !self.at(&Token::RBrace) && !self.at_eof() {
            let vname = self.expect_ident()?;
            let fields = if self.at(&Token::LParen) {
                self.advance();
                let fs = self.parse_comma_sep(|p| p.parse_type_expr(), &Token::RParen)?;
                self.expect(&Token::RParen)?;
                fs
            } else {
                vec![]
            };
            variants.push(Variant {
                name: vname,
                fields,
            });
            if self.at(&Token::Comma) {
                self.advance();
            }
        }
        self.expect(&Token::RBrace)?;

        Ok(Item::TypeDef(TypeDef {
            name,
            visibility: Visibility::Private,
            type_params,
            variants,
            implements: vec![],
        }))
    }

    // ── Capability definition ───────────────────────────────────────

    fn parse_capability_item(&mut self) -> Result<Item, ParseError> {
        self.expect(&Token::Capability)?;
        let name = self.expect_ident()?;

        let type_params = if self.at(&Token::LBracket) {
            self.advance();
            let ps = self.parse_comma_sep(|p| p.expect_ident(), &Token::RBracket)?;
            self.expect(&Token::RBracket)?;
            ps
        } else {
            vec![]
        };

        self.expect(&Token::LBrace)?;
        let mut methods = Vec::new();
        while !self.at(&Token::RBrace) && !self.at_eof() {
            methods.push(self.parse_fn_def()?);
        }
        self.expect(&Token::RBrace)?;

        Ok(Item::CapabilityDef(CapabilityDef {
            name,
            visibility: Visibility::Private,
            type_params,
            methods,
        }))
    }

    // ── Import declaration ──────────────────────────────────────────

    fn parse_import_item(&mut self) -> Result<Item, ParseError> {
        self.expect(&Token::Import)?;
        let path = self.expect_ident()?;

        // Collect path segments: `import std::io::File`
        let mut full_path = path;
        while self.at(&Token::ColonColon) {
            self.advance();
            let seg = self.expect_ident()?;
            full_path = format!("{full_path}::{seg}");
        }

        let alias = if self.at(&Token::As) {
            self.advance();
            self.expect_ident()?
        } else {
            // Default alias is the last segment
            full_path
                .rsplit("::")
                .next()
                .unwrap_or(&full_path)
                .to_string()
        };

        Ok(Item::Import(ImportDecl::Import {
            path: full_path,
            alias,
        }))
    }

    // ── Expressions ─────────────────────────────────────────────────

    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_expr_bp(0)
    }

    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            // Postfix: `?` (try), `.field`, `(args)` (call)
            loop {
                match self.peek() {
                    Token::Question => {
                        self.advance();
                        lhs = Expr::Try(Box::new(lhs));
                    }
                    Token::Dot => {
                        self.advance();
                        let field = self.expect_ident()?;
                        // Check for method call: `obj.method(args)`
                        if self.at(&Token::LParen) {
                            self.advance();
                            let args = self.parse_comma_sep(|p| p.parse_expr(), &Token::RParen)?;
                            self.expect(&Token::RParen)?;
                            let callee = Expr::FieldAccess(Box::new(lhs), field);
                            lhs = Expr::Call(Box::new(callee), args);
                        } else {
                            lhs = Expr::FieldAccess(Box::new(lhs), field);
                        }
                    }
                    Token::LParen => {
                        // function call
                        self.advance();
                        let args = self.parse_comma_sep(|p| p.parse_expr(), &Token::RParen)?;
                        self.expect(&Token::RParen)?;
                        lhs = Expr::Call(Box::new(lhs), args);
                    }
                    Token::LBracket => {
                        // generic instantiation call: `foo[T](args)` — parse [T] as generic
                        // For now, skip this — it'd need lookahead to disambiguate from indexing
                        break;
                    }
                    _ => break,
                }
            }

            // Infix: check for pipe specially since it has Pipe token and PipeArrow
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

                // For `|` (bitwise OR) vs lambda: we only get here if already in expr
                if matches!(op_tok, Token::Pipe) {
                    // BitOr in infix position
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
            // Unary operators
            tok if prefix_bp(&tok).is_some() => {
                let bp = prefix_bp(&tok).unwrap();
                let op = token_to_unaryop(&tok).unwrap();
                self.advance();
                let expr = self.parse_expr_bp(bp)?;
                Ok(Expr::UnaryOp(op, Box::new(expr)))
            }

            // Integer literal
            Token::Int(n) => {
                self.advance();
                Ok(Expr::IntLit(n))
            }
            // Float literal
            Token::Float(f) => {
                self.advance();
                Ok(Expr::FloatLit(f))
            }
            // String literal
            Token::Str(s) => {
                self.advance();
                Ok(Expr::StrLit(s))
            }
            // Bool literal
            Token::Bool(b) => {
                self.advance();
                Ok(Expr::BoolLit(b))
            }

            // Block
            Token::LBrace => self.parse_block_expr(),

            // Parenthesized expression
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }

            // If expression
            Token::If => self.parse_if_expr(),

            // Match expression
            Token::Match => self.parse_match_expr(),

            // Spawn expression
            Token::Spawn => {
                self.advance();
                let expr = self.parse_expr()?;
                Ok(Expr::Spawn(Box::new(expr)))
            }

            // Await expression
            Token::Await => {
                self.advance();
                let expr = self.parse_expr()?;
                Ok(Expr::Await(Box::new(expr)))
            }

            // Return expression
            Token::Return => {
                self.advance();
                let expr = self.parse_expr()?;
                Ok(expr) // TODO: add Return to AST if needed
            }

            // Hole: `?name` or `?name: Type`
            Token::Question => {
                self.advance();
                let name = self.expect_ident()?;
                let ty = if self.at(&Token::Colon) {
                    self.advance();
                    Some(Box::new(self.parse_type_expr()?))
                } else {
                    None
                };
                Ok(Expr::Hole(name, ty))
            }

            // Lambda: `|params| body`
            Token::Pipe => self.parse_lambda(),

            // Identifier (variable or struct literal or call)
            Token::Ident(name) => {
                self.advance();
                // Check for struct literal: `Name { field: val, ... }`
                // Only if the name starts with uppercase
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

            _ => Err(self.error(format!("expected expression, found {:?}", self.peek()))),
        }
    }

    // ── Lambda ──────────────────────────────────────────────────────

    fn parse_lambda(&mut self) -> Result<Expr, ParseError> {
        self.expect(&Token::Pipe)?;
        let params = self.parse_comma_sep(
            |p| {
                let name = p.expect_ident()?;
                let ty = if p.at(&Token::Colon) {
                    p.advance();
                    p.parse_type_expr()?
                } else {
                    TypeExpr::Named("_".into()) // inferred
                };
                Ok(Param { name, ty })
            },
            &Token::Pipe,
        )?;
        self.expect(&Token::Pipe)?;
        let body = self.parse_expr()?;
        Ok(Expr::Lambda(params, Box::new(body)))
    }

    // ── Block expression ────────────────────────────────────────────

    fn parse_block_expr(&mut self) -> Result<Expr, ParseError> {
        self.expect(&Token::LBrace)?;
        let mut stmts = Vec::new();
        let mut tail: Option<Box<Expr>> = None;

        while !self.at(&Token::RBrace) && !self.at_eof() {
            // Check for `let` statement
            if self.at(&Token::Let) {
                stmts.push(self.parse_let_stmt()?);
                // Optional semicolon
                if self.at(&Token::Semicolon) {
                    self.advance();
                }
            } else {
                let expr = self.parse_expr()?;
                if self.at(&Token::Semicolon) {
                    self.advance();
                    stmts.push(Stmt::Expr(expr));
                } else if self.at(&Token::RBrace) {
                    // This is the tail expression
                    tail = Some(Box::new(expr));
                } else {
                    // Expression without semicolon not at end — treat as statement
                    stmts.push(Stmt::Expr(expr));
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
        Ok(Stmt::Let(name, ty, expr))
    }

    // ── If expression ───────────────────────────────────────────────

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
        Ok(Expr::If(
            Box::new(cond),
            Box::new(then_branch),
            else_branch,
        ))
    }

    // ── Match expression ────────────────────────────────────────────

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

    // ── Patterns ────────────────────────────────────────────────────

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        let pat = self.parse_single_pattern()?;
        // Check for `|` (or-pattern)
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
            Token::Ident(name) => {
                self.advance();
                // Constructor: `Some(x)` or struct: `Point { x, y }`
                if self.at(&Token::LParen) {
                    self.advance();
                    let fields =
                        self.parse_comma_sep(|p| p.parse_pattern(), &Token::RParen)?;
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
            _ => Err(self.error(format!("expected pattern, found {:?}", self.peek()))),
        }
    }

    // ── Helpers ─────────────────────────────────────────────────────

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
            self.advance(); // eat comma
        }
        Ok(items)
    }
}
