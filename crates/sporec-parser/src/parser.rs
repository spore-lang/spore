//! Recursive-descent + Pratt parser for the Spore language.
//!
//! Produces AST nodes defined in [`crate::ast`].

use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::{Comment, Span, Spanned, TemplatePart, Token};

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

// ── Placeholder desugaring ───────────────────────────────────────────────

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
            // Allow `self` as an identifier in parameter position
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

    // ── Top-level: Module ───────────────────────────────────────────

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

    // ── Items ───────────────────────────────────────────────────────

    fn parse_item(&mut self) -> Result<Item, ParseError> {
        match self.peek() {
            Token::Fn | Token::Pub | Token::Foreign => self.parse_fn_or_const_or_alias_item(),
            Token::Const => self.parse_const_item(),
            Token::Struct => self.parse_struct_item(),
            Token::Type => self.parse_type_item(),
            Token::Capability => self.parse_capability_item(),
            Token::Trait => self.parse_trait_item(),
            Token::Effect => self.parse_effect_item(),
            Token::Handler => self.parse_handler_item(),
            Token::Impl => self.parse_impl_item(),
            Token::Import => self.parse_import_item(),
            Token::Alias => self.parse_alias_item(),
            Token::At => self.parse_annotated_item(),
            _ => Err(self.error(format!(
                "expected item (fn, pub, const, struct, type, trait, effect, handler, impl, import, alias, @annotation), found {:?}",
                self.peek()
            ))),
        }
    }

    fn parse_annotated_item(&mut self) -> Result<Item, ParseError> {
        let start = self.peek_span().start;
        let mut is_unbounded = false;
        let mut hole_allows = None;

        while self.at(&Token::At) {
            self.expect(&Token::At)?;
            let annotation = self.expect_ident()?;
            match annotation.as_str() {
                "unbounded" => {
                    is_unbounded = true;
                }
                "allows" => {
                    if hole_allows.is_some() {
                        return Err(self.error("duplicate `@allows[...]` annotation".into()));
                    }
                    self.expect(&Token::LBracket)?;
                    let allows = self.parse_comma_sep(|p| p.expect_ident(), &Token::RBracket)?;
                    self.expect(&Token::RBracket)?;
                    hole_allows = Some(allows);
                }
                _ => return Err(self.error(format!("unknown annotation `@{annotation}`"))),
            }
        }

        let mut fn_def = self.parse_fn_def()?;
        fn_def.is_unbounded = is_unbounded;
        fn_def.hole_allows = hole_allows;
        // Extend span to include annotations
        fn_def.span = fn_def.span.map(|s| Span::new(start, s.end));
        Ok(Item::Function(fn_def))
    }

    fn parse_fn_or_const_or_alias_item(&mut self) -> Result<Item, ParseError> {
        // Peek ahead past optional visibility to decide fn vs const vs alias
        let mut lookahead = self.pos;
        if matches!(self.tokens[lookahead].node, Token::Pub) {
            lookahead += 1;
            // Skip optional `(pkg)`
            if matches!(
                self.tokens.get(lookahead).map(|t| &t.node),
                Some(Token::LParen)
            ) {
                lookahead += 1; // `(`
                lookahead += 1; // `pkg`
                lookahead += 1; // `)`
            }
        }
        // Skip optional `foreign` keyword
        if matches!(
            self.tokens.get(lookahead).map(|t| &t.node),
            Some(Token::Foreign)
        ) {
            lookahead += 1;
        }
        match self.tokens.get(lookahead).map(|t| &t.node) {
            Some(Token::Const) => self.parse_const_item(),
            Some(Token::Alias) => self.parse_alias_item(),
            Some(Token::Struct) => self.parse_struct_item(),
            Some(Token::Type) => self.parse_type_item(),
            Some(Token::Capability) => self.parse_capability_item(),
            Some(Token::Trait) => self.parse_trait_item(),
            Some(Token::Effect) => self.parse_effect_item(),
            Some(Token::Handler) => self.parse_handler_item(),
            _ => self.parse_fn_item(),
        }
    }

    fn parse_alias_item(&mut self) -> Result<Item, ParseError> {
        let start = self.peek_span().start;
        let visibility = self.parse_visibility()?;
        self.expect(&Token::Alias)?;
        let name = self.expect_ident()?;
        self.expect(&Token::Eq)?;
        let target = self.parse_type_expr()?;
        let end = self.previous_span().end;
        Ok(Item::Alias(AliasDef {
            name,
            visibility,
            target,
            span: Some(Span::new(start, end)),
        }))
    }

    fn parse_const_item(&mut self) -> Result<Item, ParseError> {
        let start = self.peek_span().start;
        let visibility = self.parse_visibility()?;
        self.expect(&Token::Const)?;
        let name = self.expect_ident()?;
        self.expect(&Token::Colon)?;
        let ty = self.parse_type_expr()?;
        self.expect(&Token::Eq)?;
        let value = self.parse_expr()?;
        let end = self.previous_span().end;
        Ok(Item::Const(ConstDef {
            name,
            visibility,
            ty,
            value,
            span: Some(Span::new(start, end)),
        }))
    }

    fn parse_fn_item(&mut self) -> Result<Item, ParseError> {
        Ok(Item::Function(self.parse_fn_def()?))
    }

    fn parse_fn_def(&mut self) -> Result<FnDef, ParseError> {
        let start = self.peek_span().start;

        // optional visibility
        let visibility = self.parse_visibility()?;

        // optional `foreign` keyword
        let is_foreign = if self.at(&Token::Foreign) {
            self.advance();
            true
        } else {
            false
        };

        self.expect(&Token::Fn)?;
        let name = self.expect_ident()?;

        // optional type parameters: fn foo[T, U](...)
        let type_params = if self.at(&Token::LBracket) {
            self.advance();
            let ps = self.parse_comma_sep(|p| p.expect_ident(), &Token::RBracket)?;
            self.expect(&Token::RBracket)?;
            ps
        } else {
            vec![]
        };

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

        // optional errors clause: `! E1 | E2`
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

        // Optional clauses may appear in any order; the formatter later
        // normalizes them to `where -> uses -> cost -> spec`.
        let mut where_clause = None;
        let mut uses_clause = None;
        let mut cost_clause = None;
        let mut spec_clause = None;
        loop {
            if self.at(&Token::Where) {
                if where_clause.is_some() {
                    return Err(self.error("duplicate `where` clause".into()));
                }
                where_clause = Some(self.parse_where_clause()?);
                continue;
            }
            if self.at(&Token::Uses) {
                if uses_clause.is_some() {
                    return Err(self.error("duplicate `uses` clause".into()));
                }
                uses_clause = Some(self.parse_uses_clause()?);
                continue;
            }
            if self.at(&Token::Cost) {
                if cost_clause.is_some() {
                    return Err(self.error("duplicate `cost` clause".into()));
                }
                cost_clause = Some(self.parse_cost_clause()?);
                continue;
            }
            if self.at(&Token::Spec) {
                if spec_clause.is_some() {
                    return Err(self.error("duplicate `spec` clause".into()));
                }
                spec_clause = Some(self.parse_spec_clause()?);
                continue;
            }
            break;
        }

        // body: block or hole
        let body = if self.at(&Token::LBrace) {
            Some(self.parse_block_expr()?)
        } else {
            None
        };

        let end = self.previous_span().end;

        Ok(FnDef {
            name,
            visibility,
            type_params,
            params,
            return_type,
            errors,
            where_clause,
            cost_clause,
            spec_clause,
            uses_clause,
            is_unbounded: false,
            hole_allows: None,
            is_foreign,
            body,
            span: Some(Span::new(start, end)),
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
            if self.at(&Token::Plus) {
                return Err(self.error(
                    "multiple trait bounds are not supported yet; use a single bound like `T: Trait`"
                        .into(),
                ));
            }
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
        if self.at(&Token::Le2) || self.at(&Token::LtEq) {
            return Err(self.error(
                "scalar `cost <= expr` syntax was removed; use `cost [compute, alloc, io, parallel]`"
                    .into(),
            ));
        }
        self.expect(&Token::LBracket)?;
        let compute = self.parse_cost_expr()?;
        self.expect(&Token::Comma)?;
        let alloc = self.parse_cost_expr()?;
        self.expect(&Token::Comma)?;
        let io = self.parse_cost_expr()?;
        self.expect(&Token::Comma)?;
        let parallel = self.parse_cost_expr()?;
        self.expect(&Token::RBracket)?;
        Ok(CostClause {
            compute,
            alloc,
            io,
            parallel,
        })
    }

    fn parse_cost_expr(&mut self) -> Result<CostExpr, ParseError> {
        let expr = self.parse_cost_atom()?;
        if matches!(self.peek(), Token::Plus | Token::Star | Token::LParen) {
            return Err(self.error(
                "cost slot expressions only support integer literals, parameter variables, or linear `O(n)`; composed expressions are deferred"
                    .into(),
            ));
        }
        Ok(expr)
    }

    fn parse_cost_atom(&mut self) -> Result<CostExpr, ParseError> {
        match self.peek().clone() {
            Token::Int(n) => {
                self.advance();
                Ok(CostExpr::Literal(n as u64))
            }
            Token::Ident(s) => {
                if s == "O"
                    && matches!(
                        self.tokens.get(self.pos + 1).map(|t| &t.node),
                        Some(Token::LParen)
                    )
                {
                    self.advance(); // O
                    self.expect(&Token::LParen)?;
                    let var = self.expect_ident()?;
                    self.expect(&Token::RParen)?;
                    return Ok(CostExpr::Linear(var));
                }
                self.advance();
                Ok(CostExpr::Var(s))
            }
            _ => Err(self.error(format!("expected cost expression, found {:?}", self.peek()))),
        }
    }

    fn parse_spec_clause(&mut self) -> Result<SpecClause, ParseError> {
        let start = self.peek_span().start;
        self.expect(&Token::Spec)?;
        self.expect(&Token::LBrace)?;

        let mut items = Vec::new();

        while !self.at(&Token::RBrace) && !self.at_eof() {
            match self.peek().clone() {
                Token::Ident(ref s) if s == "example" => {
                    items.push(SpecItem::Example(self.parse_example_item()?));
                }
                Token::Ident(ref s) if s == "property" => {
                    items.push(SpecItem::Property(self.parse_property_item()?));
                }
                _ => {
                    return Err(self.error(format!(
                        "expected `example` or `property` in spec block, found {:?}",
                        self.peek()
                    )));
                }
            }
        }

        let end = self.peek_span().end;
        self.expect(&Token::RBrace)?;

        Ok(SpecClause {
            items,
            span: Some(Span::new(start, end)),
        })
    }

    fn parse_example_item(&mut self) -> Result<ExampleItem, ParseError> {
        let start = self.peek_span().start;
        // Consume contextual keyword `example`
        self.advance();

        // Parse label (string literal)
        let label = match self.peek().clone() {
            Token::Str(s) => {
                self.advance();
                s
            }
            _ => return Err(self.error("expected string label after `example`".into())),
        };

        let body = if self.at(&Token::Colon) {
            self.advance();
            self.parse_expr()?
        } else if self.at(&Token::LBrace) {
            self.parse_block_expr()?
        } else {
            return Err(self.error("expected `:` or `{` after `example` label".into()));
        };

        let end = self.previous_span().end;

        Ok(ExampleItem {
            label,
            body: Box::new(body),
            span: Some(Span::new(start, end)),
        })
    }

    fn parse_property_item(&mut self) -> Result<PropertyItem, ParseError> {
        let start = self.peek_span().start;
        // Consume contextual keyword `property`
        self.advance();

        // Parse label (string literal)
        let label = match self.peek().clone() {
            Token::Str(s) => {
                self.advance();
                s
            }
            _ => return Err(self.error("expected string label after `property`".into())),
        };

        // Expect `:` then lambda expression
        self.expect(&Token::Colon)?;
        let predicate = self.parse_expr()?;

        let end = self.previous_span().end;

        Ok(PropertyItem {
            label,
            predicate: Box::new(predicate),
            span: Some(Span::new(start, end)),
        })
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
        let base = self.parse_type_expr_base()?;
        // Check for refinement: `Type when predicate`
        if self.at(&Token::When) {
            self.advance();
            let pred = self.parse_expr()?;
            // Use "self" as the default binding variable name
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
                // Check for generic params: `List[Int]`
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
                // Tuple or function type
                let types = self.parse_comma_sep(|p| p.parse_type_expr(), &Token::RParen)?;
                self.expect(&Token::RParen)?;
                if self.at(&Token::Arrow) {
                    self.advance();
                    let ret = self.parse_type_expr()?;
                    // Parse optional error set: `! ErrorType | ErrorType2`
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

    // ── Struct definition ───────────────────────────────────────────

    fn parse_struct_item(&mut self) -> Result<Item, ParseError> {
        let visibility = self.parse_visibility()?;
        let start = self.peek_span().start;
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

        let deriving = self.parse_deriving_clause()?;

        let end = self.previous_span().end;

        Ok(Item::StructDef(StructDef {
            name,
            visibility,
            type_params,
            fields,
            implements: vec![],
            deriving,
            span: Some(Span::new(start, end)),
        }))
    }

    // ── Type (enum) definition ──────────────────────────────────────

    fn parse_type_item(&mut self) -> Result<Item, ParseError> {
        let visibility = self.parse_visibility()?;
        let start = self.peek_span().start;
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

        let deriving = self.parse_deriving_clause()?;

        let end = self.previous_span().end;

        Ok(Item::TypeDef(TypeDef {
            name,
            visibility,
            type_params,
            variants,
            implements: vec![],
            deriving,
            span: Some(Span::new(start, end)),
        }))
    }

    // ── Deriving clause ──────────────────────────────────────────────

    fn parse_deriving_clause(&mut self) -> Result<Vec<String>, ParseError> {
        if let Token::Ident(kw) = self.peek()
            && kw == "deriving"
        {
            self.advance();
            self.expect(&Token::LBracket)?;
            let names = self.parse_comma_sep(|p| p.expect_ident(), &Token::RBracket)?;
            self.expect(&Token::RBracket)?;
            return Ok(names);
        }
        Ok(vec![])
    }

    // ── Removed legacy capability syntax ────────────────────────────

    fn parse_capability_item(&mut self) -> Result<Item, ParseError> {
        let _visibility = self.parse_visibility()?;
        let span = self.peek_span();
        self.expect(&Token::Capability)?;
        Err(ParseError {
            message:
                "legacy `capability` syntax has been removed; use `trait` for interfaces and `effect` for effect declarations".into(),
            span,
        })
    }

    // ── Trait definition (preferred form of capability) ──────────────

    fn parse_trait_item(&mut self) -> Result<Item, ParseError> {
        let visibility = self.parse_visibility()?;
        let start = self.peek_span().start;
        self.expect(&Token::Trait)?;
        let name = self.expect_ident()?;

        let type_params = if self.at(&Token::LBracket) {
            self.advance();
            let ps = self.parse_comma_sep(|p| p.expect_ident(), &Token::RBracket)?;
            self.expect(&Token::RBracket)?;
            ps
        } else {
            vec![]
        };

        if self.at(&Token::Eq) {
            return Err(
                self.error("trait aliases are not supported; use `effect Name = Foo | Bar`".into())
            );
        }

        self.expect(&Token::LBrace)?;
        let mut methods = Vec::new();
        let mut assoc_types = Vec::new();
        while !self.at(&Token::RBrace) && !self.at_eof() {
            if self.at(&Token::Type) {
                self.advance();
                let aname = self.expect_ident()?;
                let bounds = if self.at(&Token::Colon) {
                    self.advance();
                    let mut bs = vec![self.parse_type_expr()?];
                    while self.at(&Token::Plus) {
                        self.advance();
                        bs.push(self.parse_type_expr()?);
                    }
                    bs
                } else {
                    vec![]
                };
                assoc_types.push(AssocType {
                    name: aname,
                    bounds,
                });
            } else {
                methods.push(self.parse_fn_def()?);
            }
        }
        self.expect(&Token::RBrace)?;

        let end = self.previous_span().end;

        Ok(Item::TraitDef(TraitDef {
            name,
            visibility,
            type_params,
            methods,
            assoc_types,
            span: Some(Span::new(start, end)),
        }))
    }

    // ── Effect definition / alias ───────────────────────────────────

    fn parse_effect_item(&mut self) -> Result<Item, ParseError> {
        let visibility = self.parse_visibility()?;
        let start = self.peek_span().start;
        self.expect(&Token::Effect)?;
        let name = self.expect_ident()?;

        // Effect alias: `effect IO = Console | FileRead | FileWrite`
        if self.at(&Token::Eq) {
            self.advance();
            let mut effects = vec![self.expect_ident()?];
            while self.at(&Token::Pipe) {
                self.advance();
                effects.push(self.expect_ident()?);
            }
            let end = self.previous_span().end;
            return Ok(Item::EffectAlias(EffectAlias {
                name,
                visibility,
                effects,
                span: Some(Span::new(start, end)),
            }));
        }

        // Effect definition: `effect Console { fn println(msg: Str) -> Unit }`
        self.expect(&Token::LBrace)?;
        let mut operations = Vec::new();
        while !self.at(&Token::RBrace) && !self.at_eof() {
            operations.push(self.parse_fn_def()?);
        }
        self.expect(&Token::RBrace)?;

        let end = self.previous_span().end;

        Ok(Item::EffectDef(EffectDef {
            name,
            visibility,
            operations,
            span: Some(Span::new(start, end)),
        }))
    }

    // ── Handler definition ──────────────────────────────────────────

    fn parse_handler_item(&mut self) -> Result<Item, ParseError> {
        let start = self.peek_span().start;
        self.expect(&Token::Handler)?;
        let first = self.expect_ident()?;
        let (effect, name) = if self.at(&Token::As) {
            self.advance();
            let name = self.expect_ident()?;
            (first, name)
        } else {
            // Compatibility path: `handler Name for Effect { ... }`
            let next = self.expect_ident()?;
            if next != "for" {
                return Err(self.error(format!(
                    "expected `as` or legacy `for` after handler head, got `{next}`"
                )));
            }
            let effect = self.expect_ident()?;
            (effect, first)
        };

        let fields = if self.at(&Token::LParen) {
            self.advance();
            let fields = self.parse_comma_sep(
                |p| {
                    let name = p.expect_ident()?;
                    p.expect(&Token::Colon)?;
                    let ty = p.parse_type_expr()?;
                    Ok(FieldDef { name, ty })
                },
                &Token::RParen,
            )?;
            self.expect(&Token::RParen)?;
            fields
        } else {
            vec![]
        };

        self.expect(&Token::LBrace)?;
        let mut methods = Vec::new();
        while !self.at(&Token::RBrace) && !self.at_eof() {
            methods.push(self.parse_fn_def()?);
        }
        self.expect(&Token::RBrace)?;

        let end = self.previous_span().end;

        Ok(Item::HandlerDef(HandlerDef {
            name,
            effect,
            fields,
            methods,
            span: Some(Span::new(start, end)),
        }))
    }

    // ── Impl block ──────────────────────────────────────────────────

    fn parse_impl_item(&mut self) -> Result<Item, ParseError> {
        let start = self.peek_span().start;
        self.expect(&Token::Impl)?;
        let capability = self.expect_ident()?;

        // Optional type arguments: `impl Show[T] for ...`
        let type_args = if self.at(&Token::LBracket) {
            self.advance();
            let args = self.parse_comma_sep(|p| p.parse_type_expr(), &Token::RBracket)?;
            self.expect(&Token::RBracket)?;
            args
        } else {
            vec![]
        };

        // Expect `for` (not a keyword — parsed as identifier)
        let next = self.expect_ident()?;
        if next != "for" {
            return Err(self.error(format!(
                "expected `for` after trait/effect name, got `{next}`"
            )));
        }

        let target_type = self.expect_ident()?;

        self.expect(&Token::LBrace)?;
        let mut methods = Vec::new();
        while !self.at(&Token::RBrace) && !self.at_eof() {
            methods.push(self.parse_fn_def()?);
        }
        self.expect(&Token::RBrace)?;

        let end = self.previous_span().end;

        Ok(Item::ImplDef(ImplDef {
            capability,
            target_type,
            type_args,
            methods,
            span: Some(Span::new(start, end)),
        }))
    }

    // ── Import declaration ──────────────────────────────────────────

    fn parse_import_item(&mut self) -> Result<Item, ParseError> {
        let start = self.peek_span().start;
        self.expect(&Token::Import)?;
        let path = self.expect_ident()?;

        // Collect path segments: `import std.io.File`
        let mut full_path = path;
        while self.at(&Token::Dot) {
            self.advance();
            let seg = self.expect_ident()?;
            full_path = format!("{full_path}.{seg}");
        }

        let alias = if self.at(&Token::As) {
            self.advance();
            self.expect_ident()?
        } else {
            // Default alias is the last segment
            full_path
                .rsplit('.')
                .next()
                .unwrap_or(&full_path)
                .to_string()
        };

        let end = self.previous_span().end;

        Ok(Item::Import(ImportDecl::Import {
            path: full_path,
            alias,
            span: Some(Span::new(start, end)),
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
                        // Check for method call: `obj.method(args)`
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
                        // function call
                        self.advance();
                        let args = self.parse_comma_sep(|p| p.parse_expr(), &Token::RParen)?;
                        self.expect(&Token::RParen)?;
                        lhs = desugar_placeholder_call(Box::new(lhs), args);
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
                let bp = prefix_bp(&tok).expect("prefix_bp: unreachable - token already matched");
                let op = token_to_unaryop(&tok)
                    .expect("token_to_unaryop: unreachable - token already matched");
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
            // f-string
            Token::FStr(parts) => {
                self.advance();
                self.expand_template_parts(&parts, true)
            }
            // t-string
            Token::TStr(parts) => {
                self.advance();
                self.expand_template_parts(&parts, false)
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

            // Return expression
            Token::Return => {
                self.advance();
                // If next token can't start an expression, return None
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

            // Throw expression
            Token::Throw => {
                self.advance();
                let expr = self.parse_expr()?;
                Ok(Expr::Throw(Box::new(expr)))
            }

            // List literal: `[elem, ...]`
            Token::LBracket => {
                self.advance();
                let elems = self.parse_comma_sep(|p| p.parse_expr(), &Token::RBracket)?;
                self.expect(&Token::RBracket)?;
                Ok(Expr::List(elems))
            }

            // Char literal
            Token::Char(c) => {
                self.advance();
                Ok(Expr::CharLit(c))
            }

            // Hole: `?`, `?name`, `?: Type`, `?name: Type`, optional `@allows [...]`
            Token::Question => {
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
                // Parse optional @allows [Cap1, Cap2]
                let allows = if self.at(&Token::At) {
                    self.advance();
                    let kw = self.expect_ident()?;
                    if kw != "allows" {
                        return Err(
                            self.error(format!("expected `allows` after `@`, found `{kw}`"))
                        );
                    }
                    self.expect(&Token::LBracket)?;
                    let caps = self.parse_comma_sep(|p| p.expect_ident(), &Token::RBracket)?;
                    self.expect(&Token::RBracket)?;
                    Some(caps)
                } else {
                    None
                };
                Ok(Expr::Hole(name, ty, allows))
            }

            // Lambda: `|params| body`
            Token::Pipe => self.parse_lambda(),

            // Parallel scope: `parallel_scope { body }` or `parallel_scope(lanes: N) { body }`
            Token::ParallelScope => {
                self.advance();
                let lanes = if self.at(&Token::LParen) {
                    self.advance();
                    // expect ident "lanes"
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

            // Select expression
            Token::Select => self.parse_select_expr(),

            // Perform expression: `perform Effect.operation(args)`
            Token::Perform => self.parse_perform_expr(),

            // Handle expression: `handle { body } with { arms }`
            Token::Handle => self.parse_handle_expr(),

            // Placeholder `_` in expression position (partial application)
            Token::Ident(ref name) if name == "_" => {
                self.advance();
                Ok(Expr::Placeholder)
            }

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

            // `self` as expression (e.g., in refinement predicates)
            Token::Self_ => {
                self.advance();
                Ok(Expr::Var("self".into()))
            }

            _ => Err(self.error(format!("expected expression, found {:?}", self.peek()))),
        }
    }

    // ── Template-string helpers ────────────────────────────────────

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
        Ok(Expr::If(Box::new(cond), Box::new(then_branch), else_branch))
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

    // ── Select expression ───────────────────────────────────────────

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
                // expect `from` keyword
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

    // ── Perform expression ──────────────────────────────────────────

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

    // ── Handle expression ───────────────────────────────────────────

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
            // List pattern: `[h, ..tail]`
            Token::LBracket => {
                self.advance();
                let mut elements = Vec::new();
                let mut rest = None;
                while !self.at(&Token::RBracket) && !self.at_eof() {
                    // Check for `..ident` rest binding
                    if self.at(&Token::DotDot) {
                        self.advance();
                        rest = Some(self.expect_ident()?);
                        // optional trailing comma
                        if self.at(&Token::Comma) {
                            self.advance();
                        }
                        break;
                    }
                    elements.push(self.parse_pattern()?);
                    if !self.at(&Token::Comma) {
                        break;
                    }
                    self.advance(); // eat comma
                }
                self.expect(&Token::RBracket)?;
                Ok(Pattern::List(elements, rest))
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
