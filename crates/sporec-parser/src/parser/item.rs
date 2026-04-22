use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::{Span, Token};

use super::Parser;

impl Parser {
    pub(super) fn parse_item(&mut self) -> Result<Item, ParseError> {
        match self.peek() {
            Token::Fn | Token::Pub | Token::Foreign => self.parse_fn_or_const_or_alias_item(),
            Token::Const => self.parse_const_item(),
            Token::Struct => self.parse_struct_item(),
            Token::Type => self.parse_type_item(),
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
        fn_def.span = fn_def.span.map(|s| Span::new(start, s.end));
        Ok(Item::Function(fn_def))
    }

    fn parse_fn_or_const_or_alias_item(&mut self) -> Result<Item, ParseError> {
        let mut lookahead = self.pos;
        if matches!(self.tokens[lookahead].node, Token::Pub) {
            lookahead += 1;
            if matches!(
                self.tokens.get(lookahead).map(|t| &t.node),
                Some(Token::LParen)
            ) {
                lookahead += 3;
            }
        }
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
        let visibility = self.parse_visibility()?;
        let is_foreign = if self.at(&Token::Foreign) {
            self.advance();
            true
        } else {
            false
        };

        self.expect(&Token::Fn)?;
        let name = self.expect_ident()?;

        let type_params = if self.at(&Token::LBracket) {
            self.advance();
            let ps = self.parse_comma_sep(|p| p.expect_ident(), &Token::RBracket)?;
            self.expect(&Token::RBracket)?;
            ps
        } else {
            vec![]
        };

        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;

        let return_type = if self.at(&Token::Arrow) {
            self.advance();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

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
            self.advance();
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
                    self.advance();
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
        self.advance();
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
        self.advance();
        let label = match self.peek().clone() {
            Token::Str(s) => {
                self.advance();
                s
            }
            _ => return Err(self.error("expected string label after `property`".into())),
        };
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

    fn parse_struct_item(&mut self) -> Result<Item, ParseError> {
        let visibility = self.parse_visibility()?;
        let start = self.peek_span().start;
        self.expect(&Token::Struct)?;
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

    fn parse_effect_item(&mut self) -> Result<Item, ParseError> {
        let visibility = self.parse_visibility()?;
        let start = self.peek_span().start;
        self.expect(&Token::Effect)?;
        let name = self.expect_ident()?;

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

    fn parse_handler_item(&mut self) -> Result<Item, ParseError> {
        let start = self.peek_span().start;
        self.expect(&Token::Handler)?;
        let first = self.expect_ident()?;
        let (effect, name) = if self.at(&Token::As) {
            self.advance();
            let name = self.expect_ident()?;
            (first, name)
        } else {
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

    fn parse_impl_item(&mut self) -> Result<Item, ParseError> {
        let start = self.peek_span().start;
        self.expect(&Token::Impl)?;
        let trait_name = self.expect_ident()?;

        let type_args = if self.at(&Token::LBracket) {
            self.advance();
            let args = self.parse_comma_sep(|p| p.parse_type_expr(), &Token::RBracket)?;
            self.expect(&Token::RBracket)?;
            args
        } else {
            vec![]
        };

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
            trait_name,
            target_type,
            type_args,
            methods,
            span: Some(Span::new(start, end)),
        }))
    }

    fn parse_import_item(&mut self) -> Result<Item, ParseError> {
        let start = self.peek_span().start;
        self.expect(&Token::Import)?;
        let path = self.expect_ident()?;

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
}
