use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::{Span, Token};

use super::Parser;

impl Parser {
    pub(super) fn parse_item(&mut self) -> Result<Item, ParseError> {
        match self.peek() {
            Token::Fn | Token::Pub => self.parse_fn_or_const_or_alias_item(),
            Token::Const => self.parse_const_item(),
            Token::Struct => self.parse_struct_item(),
            Token::Enum => self.parse_enum_item(),
            Token::Type => self.parse_type_item(),
            Token::Trait => self.parse_trait_item(),
            Token::Effect => self.parse_effect_item(),
            Token::Surface => self.parse_surface_item(),
            Token::Handler => self.parse_handler_item(),
            Token::Impl => self.parse_impl_item(),
            Token::Import => self.parse_import_item(),
            Token::Alias => Err(self.error(
                "`alias Name = Type` is not part of the current syntax; use `type Name = Type`"
                    .into(),
            )),
            Token::At => self.parse_attributed_item(),
            Token::Foreign => Err(self.error(
                "`foreign fn` is not part of the current syntax; use `@foreign` before the declaration"
                    .into(),
            )),
            _ => Err(self.error(format!(
                "expected item (fn, pub, const, struct, enum, type, trait, effect, surface, handler, impl, import), found {:?}",
                self.peek()
            ))),
        }
    }

    fn parse_attributed_item(&mut self) -> Result<Item, ParseError> {
        let attributes = self.parse_attributes()?;
        if self.at(&Token::LBracket) {
            return Err(self.error(
                "attribute arguments must use parentheses, for example `@name(...)`".into(),
            ));
        }
        let mut item = self.parse_item()?;
        match &mut item {
            Item::Function(function) => {
                function.is_foreign = has_attribute(&attributes, "foreign");
                function.attributes = attributes;
            }
            Item::Const(constant) => constant.attributes = attributes,
            Item::StructDef(struct_def) => struct_def.attributes = attributes,
            Item::TypeDef(type_def) => type_def.attributes = attributes,
            Item::ImplDef(impl_def) => impl_def.attributes = attributes,
            Item::Alias(alias_def) => alias_def.attributes = attributes,
            Item::OpaqueType(type_def) => type_def.attributes = attributes,
            Item::TraitDef(trait_def) => trait_def.attributes = attributes,
            Item::EffectDef(effect_def) => effect_def.attributes = attributes,
            Item::SurfaceDef(surface_def) => surface_def.attributes = attributes,
            Item::HandlerDef(handler_def) => handler_def.attributes = attributes,
            Item::Import(_) => {
                return Err(self.error("attributes are not valid on import declarations".into()));
            }
        }
        Ok(item)
    }

    fn parse_attributes(&mut self) -> Result<Vec<Attribute>, ParseError> {
        let mut attributes = Vec::new();
        while self.at(&Token::At) {
            let start = self.peek_span().start;
            self.advance();
            let name = match self.peek().clone() {
                Token::Ident(name) => {
                    self.advance();
                    name
                }
                Token::Foreign => {
                    self.advance();
                    "foreign".into()
                }
                token => {
                    return Err(self.error(format!(
                        "expected attribute name after `@`, found {token:?}"
                    )));
                }
            };
            let args = if self.at(&Token::LParen) {
                self.advance();
                let args =
                    self.parse_comma_sep(|parser| parser.parse_attr_arg(), &Token::RParen)?;
                self.expect(&Token::RParen)?;
                args
            } else {
                Vec::new()
            };
            attributes.push(Attribute {
                name,
                args,
                span: Some(Span::new(start, self.previous_span().end)),
            });
        }
        Ok(attributes)
    }

    fn parse_attr_arg(&mut self) -> Result<AttrArg, ParseError> {
        if let Token::Ident(name) = self.peek().clone()
            && matches!(
                self.tokens.get(self.pos + 1).map(|token| &token.node),
                Some(Token::Eq)
            )
        {
            self.advance();
            self.expect(&Token::Eq)?;
            return Ok(AttrArg::Named {
                name,
                value: self.parse_attr_value()?,
            });
        }
        Ok(AttrArg::Positional(self.parse_attr_value()?))
    }

    fn parse_attr_value(&mut self) -> Result<AttrValue, ParseError> {
        match self.peek().clone() {
            Token::Ident(value) => {
                self.advance();
                Ok(AttrValue::Ident(value))
            }
            Token::Str(value) => {
                self.advance();
                Ok(AttrValue::Str(value))
            }
            Token::Int(value) => {
                self.advance();
                Ok(AttrValue::Int(value))
            }
            token => Err(self.error(format!(
                "expected attribute argument value, found {token:?}"
            ))),
        }
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
        match self.tokens.get(lookahead).map(|t| &t.node) {
            Some(Token::Const) => self.parse_const_item(),
            Some(Token::Alias) => Err(self.error(
                "`alias Name = Type` is not part of the current syntax; use `type Name = Type`"
                    .into(),
            )),
            Some(Token::Struct) => self.parse_struct_item(),
            Some(Token::Enum) => self.parse_enum_item(),
            Some(Token::Type) => self.parse_type_item(),
            Some(Token::Trait) => self.parse_trait_item(),
            Some(Token::Effect) => self.parse_effect_item(),
            Some(Token::Surface) => self.parse_surface_item(),
            Some(Token::Handler) => self.parse_handler_item(),
            _ => self.parse_fn_item(),
        }
    }

    fn parse_type_item(&mut self) -> Result<Item, ParseError> {
        let start = self.peek_span().start;
        let visibility = self.parse_visibility()?;
        self.expect(&Token::Type)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_decl_type_params()?;
        if self.at(&Token::LBrace) {
            return Err(self.error(
                "`type Name { ... }` is not part of the current syntax; use `enum Name { ... }`"
                    .into(),
            ));
        }
        if self.at(&Token::Semicolon) {
            self.advance();
            return Ok(Item::OpaqueType(OpaqueTypeDef {
                attributes: Vec::new(),
                name,
                visibility,
                type_params,
                span: Some(Span::new(start, self.previous_span().end)),
            }));
        }
        self.expect(&Token::Eq)?;
        let target = self.parse_type_expr()?;
        let end = self.previous_span().end;
        Ok(Item::Alias(AliasDef {
            attributes: Vec::new(),
            name,
            visibility,
            type_params,
            target,
            span: Some(Span::new(start, end)),
        }))
    }

    fn parse_decl_type_params(&mut self) -> Result<Vec<String>, ParseError> {
        if !self.at(&Token::LBracket) {
            return Ok(Vec::new());
        }
        self.advance();
        let params = self.parse_comma_sep(|parser| parser.expect_ident(), &Token::RBracket)?;
        self.expect(&Token::RBracket)?;
        Ok(params)
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
            attributes: Vec::new(),
            name,
            visibility,
            ty,
            value,
            span: Some(Span::new(start, end)),
        }))
    }

    fn parse_fn_item(&mut self) -> Result<Item, ParseError> {
        Ok(Item::Function(self.parse_fn_def(false, false)?))
    }

    fn parse_fn_def(
        &mut self,
        allow_receiver: bool,
        allow_qualified_name: bool,
    ) -> Result<FnDef, ParseError> {
        let start = self.peek_span().start;
        let attributes = self.parse_attributes()?;
        let visibility = self.parse_visibility()?;
        if self.at(&Token::Foreign) {
            return Err(self.error(
                "`foreign fn` is not part of the current syntax; use `@foreign` before the declaration"
                    .into(),
            ));
        }
        let is_foreign = has_attribute(&attributes, "foreign");

        self.expect(&Token::Fn)?;
        let mut name = self.expect_ident()?;
        if self.at(&Token::Dot) {
            if !allow_qualified_name {
                return Err(
                    self.error("qualified function names are only valid in handlers".into())
                );
            }
            self.advance();
            name.push('.');
            name.push_str(&self.expect_ident()?);
        }

        let (type_params, type_param_bounds) = self.parse_fn_type_params()?;

        self.expect(&Token::LParen)?;
        let params = self.parse_params(allow_receiver)?;
        self.expect(&Token::RParen)?;

        if !self.at(&Token::Arrow) {
            return Err(
                self.error("function signatures must declare a return type with `-> Type`".into())
            );
        }
        self.advance();
        let return_type = Some(self.parse_type_expr()?);

        let mut uses_clause = None;
        let mut budget_clause = None;
        let mut properties_clause = None;
        let mut last_intent_clause_rank = 0;
        loop {
            if self.at(&Token::Where) {
                return Err(
                    self.error("put generic bounds inline, e.g. `fn f[T: Trait](...)`".into())
                );
            }
            if self.at(&Token::Cost) {
                return Err(
                    self.error("use `budget { field: limit }` for signature budgets".into())
                );
            }
            if self.at(&Token::Spec) {
                return Err(self.error(
                    "use `properties { name(params): expr }` for signature properties".into(),
                ));
            }
            if self.at(&Token::Uses) {
                if uses_clause.is_some() {
                    return Err(self.error("duplicate `uses` clause".into()));
                }
                if last_intent_clause_rank > 1 {
                    return Err(self.error(
                        "intent signature clauses must appear in order: `uses`, `budget`, `properties`"
                            .into(),
                    ));
                }
                uses_clause = Some(self.parse_uses_clause()?);
                last_intent_clause_rank = 1;
                continue;
            }
            if self.at(&Token::Budget) {
                if budget_clause.is_some() {
                    return Err(self.error("duplicate `budget` block".into()));
                }
                if last_intent_clause_rank > 2 {
                    return Err(self.error(
                        "intent signature clauses must appear in order: `uses`, `budget`, `properties`"
                            .into(),
                    ));
                }
                budget_clause = Some(self.parse_budget_clause()?);
                last_intent_clause_rank = 2;
                continue;
            }
            if self.at(&Token::Properties) {
                if properties_clause.is_some() {
                    return Err(self.error("duplicate `properties` block".into()));
                }
                properties_clause = Some(self.parse_properties_clause()?);
                last_intent_clause_rank = 3;
                continue;
            }
            break;
        }

        let body = if self.at(&Token::LBrace) {
            Some(self.parse_block_expr()?)
        } else {
            self.expect(&Token::Semicolon)?;
            None
        };

        let end = self.previous_span().end;

        Ok(FnDef {
            attributes,
            name,
            visibility,
            type_params,
            type_param_bounds,
            params,
            return_type,
            budget_clause,
            properties_clause,
            uses_clause,
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

    fn parse_fn_type_params(&mut self) -> Result<(Vec<String>, Vec<TypeConstraint>), ParseError> {
        if !self.at(&Token::LBracket) {
            return Ok((Vec::new(), Vec::new()));
        }

        self.advance();
        let mut params = Vec::new();
        let mut bounds = Vec::new();

        while !self.at(&Token::RBracket) && !self.at_eof() {
            if self.at_contextual_ident("const") {
                return Err(self.error(
                    "const generic parameters are reserved but not implemented yet".into(),
                ));
            }

            let type_var = self.expect_ident()?;
            params.push(type_var.clone());

            if self.at(&Token::Colon) {
                self.advance();
                loop {
                    let bound = self.expect_ident()?;
                    bounds.push(TypeConstraint {
                        type_var: type_var.clone(),
                        bound,
                    });
                    if !self.at(&Token::Plus) {
                        break;
                    }
                    self.advance();
                }
            }

            if !self.at(&Token::Comma) {
                break;
            }
            self.advance();
        }

        self.expect(&Token::RBracket)?;
        Ok((params, bounds))
    }

    fn parse_params(&mut self, allow_receiver: bool) -> Result<Vec<Param>, ParseError> {
        let mut index = 0;
        self.parse_comma_sep(
            |parser| {
                let receiver_allowed_here = allow_receiver && index == 0;
                index += 1;
                parser.parse_param(receiver_allowed_here)
            },
            &Token::RParen,
        )
    }

    fn parse_param(&mut self, allow_receiver: bool) -> Result<Param, ParseError> {
        let name = self.expect_ident()?;
        if name == "self" {
            if !allow_receiver {
                return Err(self.error(
                    "receiver `self` is only valid as the first parameter of a trait or impl member"
                        .into(),
                ));
            }
            if !self.at(&Token::Colon) {
                return Ok(Param {
                    name,
                    ty: TypeExpr::Named("Self".into()),
                });
            }
        }
        self.expect(&Token::Colon)?;
        let ty = self.parse_type_expr()?;
        Ok(Param { name, ty })
    }

    fn parse_budget_clause(&mut self) -> Result<BudgetClause, ParseError> {
        let start = self.peek_span().start;
        self.expect(&Token::Budget)?;
        self.expect(&Token::LBrace)?;

        let mut items = Vec::new();
        while !self.at(&Token::RBrace) && !self.at_eof() {
            let item_start = self.peek_span().start;
            let field = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let limit = match self.peek().clone() {
                Token::Int(n) => {
                    self.advance();
                    u64::try_from(n).map_err(|_| {
                        self.error("budget values must be non-negative integer literals".into())
                    })?
                }
                _ => {
                    return Err(self.error(format!(
                        "expected non-negative integer literal for budget `{field}`, found {:?}",
                        self.peek()
                    )));
                }
            };
            let item_end = self.previous_span().end;
            items.push(BudgetItem {
                field,
                limit,
                span: Some(Span::new(item_start, item_end)),
            });
            if self.at(&Token::Comma) || self.at(&Token::Semicolon) {
                self.advance();
            }
        }

        let end = self.peek_span().end;
        self.expect(&Token::RBrace)?;
        Ok(BudgetClause {
            items,
            span: Some(Span::new(start, end)),
        })
    }

    fn parse_properties_clause(&mut self) -> Result<PropertiesClause, ParseError> {
        let start = self.peek_span().start;
        self.expect(&Token::Properties)?;
        self.expect(&Token::LBrace)?;

        let mut items = Vec::new();
        while !self.at(&Token::RBrace) && !self.at_eof() {
            let item_start = self.peek_span().start;
            let name = self.expect_ident()?;
            self.expect(&Token::LParen)?;
            let params = self.parse_comma_sep(|p| p.parse_param(false), &Token::RParen)?;
            self.expect(&Token::RParen)?;
            self.expect(&Token::Colon)?;
            let predicate = self.parse_expr()?;
            let item_end = self.previous_span().end;
            items.push(PropertyDecl {
                name,
                params,
                predicate: Box::new(predicate),
                span: Some(Span::new(item_start, item_end)),
            });
            if self.at(&Token::Comma) || self.at(&Token::Semicolon) {
                self.advance();
            }
        }

        let end = self.peek_span().end;
        self.expect(&Token::RBrace)?;
        Ok(PropertiesClause {
            items,
            span: Some(Span::new(start, end)),
        })
    }

    fn parse_uses_clause(&mut self) -> Result<UsesClause, ParseError> {
        self.expect(&Token::Uses)?;
        Ok(UsesClause {
            surface: self.parse_surface_expr()?,
        })
    }

    fn parse_surface_expr(&mut self) -> Result<SurfaceExpr, ParseError> {
        if self.at(&Token::LBracket) {
            self.advance();
            let references = self.parse_comma_sep(|p| p.parse_surface_ref(), &Token::RBracket)?;
            self.expect(&Token::RBracket)?;
            Ok(SurfaceExpr::Set(references))
        } else {
            Ok(SurfaceExpr::Named(self.parse_surface_ref()?))
        }
    }

    fn parse_surface_ref(&mut self) -> Result<SurfaceRef, ParseError> {
        let name = self.expect_ident()?;
        let type_args = if self.at(&Token::LBracket) {
            self.advance();
            let args = self.parse_comma_sep(|p| p.parse_type_expr(), &Token::RBracket)?;
            self.expect(&Token::RBracket)?;
            args
        } else {
            Vec::new()
        };
        Ok(SurfaceRef { name, type_args })
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
            attributes: Vec::new(),
            name,
            visibility,
            type_params,
            fields,
            implements: vec![],
            deriving,
            span: Some(Span::new(start, end)),
        }))
    }

    fn parse_enum_item(&mut self) -> Result<Item, ParseError> {
        let visibility = self.parse_visibility()?;
        let start = self.peek_span().start;
        self.expect(&Token::Enum)?;
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
            attributes: Vec::new(),
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

        let (type_params, type_param_bounds) = self.parse_fn_type_params()?;

        if self.at(&Token::Eq) {
            return Err(self.error(
                "trait aliases are not supported; use `surface Name = [EffectA, EffectB]` for reusable effect surfaces".into(),
            ));
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
                methods.push(self.parse_fn_def(true, false)?);
            }
        }
        self.expect(&Token::RBrace)?;

        let end = self.previous_span().end;

        Ok(Item::TraitDef(TraitDef {
            attributes: Vec::new(),
            name,
            visibility,
            type_params,
            type_param_bounds,
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
        let (type_params, type_param_bounds) = self.parse_fn_type_params()?;

        if self.at(&Token::Eq) {
            return Err(self.error(format!(
                "`effect {name} = ...` is not part of the current syntax; use `surface {name} = [EffectA, EffectB]`"
            )));
        }

        self.expect(&Token::LBrace)?;
        let mut operations = Vec::new();
        while !self.at(&Token::RBrace) && !self.at_eof() {
            operations.push(self.parse_fn_def(false, false)?);
        }
        self.expect(&Token::RBrace)?;

        let end = self.previous_span().end;

        Ok(Item::EffectDef(EffectDef {
            attributes: Vec::new(),
            name,
            visibility,
            type_params,
            type_param_bounds,
            operations,
            span: Some(Span::new(start, end)),
        }))
    }

    fn parse_surface_item(&mut self) -> Result<Item, ParseError> {
        let visibility = self.parse_visibility()?;
        let start = self.peek_span().start;
        self.expect(&Token::Surface)?;
        let name = self.expect_ident()?;
        let (type_params, type_param_bounds) = self.parse_fn_type_params()?;
        self.expect(&Token::Eq)?;
        let surface = self.parse_surface_expr()?;
        let end = self.previous_span().end;

        Ok(Item::SurfaceDef(SurfaceDef {
            attributes: Vec::new(),
            name,
            visibility,
            type_params,
            type_param_bounds,
            surface,
            span: Some(Span::new(start, end)),
        }))
    }

    fn parse_handler_item(&mut self) -> Result<Item, ParseError> {
        let start = self.peek_span().start;
        let visibility = self.parse_visibility()?;
        self.expect(&Token::Handler)?;
        let name = self.expect_ident()?;
        if !self.at_contextual_ident("for") {
            return Err(
                self.error("handler declarations must use `handler Name for Surface`".into())
            );
        }
        self.advance();
        let surface = self.parse_surface_expr()?;
        self.expect(&Token::LBrace)?;

        let mut impls = Vec::<HandlerImpl>::new();
        while !self.at(&Token::RBrace) && !self.at_eof() {
            let method = self.parse_fn_def(false, true)?;
            let Some((effect, operation)) = method
                .name
                .split_once('.')
                .map(|(effect, operation)| (effect.to_string(), operation.to_string()))
            else {
                return Err(self.error(
                    "handler methods must name an effect operation, for example `fn Console.println(...)`"
                        .into(),
                ));
            };
            let mut method = method;
            method.name = operation;
            if let Some(handler_impl) = impls.iter_mut().find(|item| item.effect == effect) {
                handler_impl.methods.push(method);
            } else {
                impls.push(HandlerImpl {
                    effect,
                    methods: vec![method],
                    span: None,
                });
            }
        }
        self.expect(&Token::RBrace)?;

        let end = self.previous_span().end;

        Ok(Item::HandlerDef(HandlerDef {
            attributes: Vec::new(),
            name,
            visibility,
            surface,
            impls,
            span: Some(Span::new(start, end)),
        }))
    }

    fn parse_impl_item(&mut self) -> Result<Item, ParseError> {
        let start = self.peek_span().start;
        self.expect(&Token::Impl)?;
        let (type_params, type_param_bounds) = self.parse_fn_type_params()?;
        let interface_type = self.parse_type_expr()?;
        let target_type = if self.at_contextual_ident("for") {
            self.advance();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        self.expect(&Token::LBrace)?;
        let mut methods = Vec::new();
        while !self.at(&Token::RBrace) && !self.at_eof() {
            methods.push(self.parse_fn_def(true, false)?);
        }
        self.expect(&Token::RBrace)?;

        let end = self.previous_span().end;

        Ok(Item::ImplDef(ImplDef {
            attributes: Vec::new(),
            type_params,
            type_param_bounds,
            interface_type,
            target_type,
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

fn has_attribute(attributes: &[Attribute], name: &str) -> bool {
    attributes.iter().any(|attribute| attribute.name == name)
}
