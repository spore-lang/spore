//! AST-based formatter for the Spore language.
//!
//! Takes a parsed `Module` and produces canonical, formatted source code.

mod expr;
mod items;
#[cfg(test)]
mod tests;

use crate::ast::*;
use crate::lexer::Comment;

/// Format a parsed Spore module back to canonical source text.
pub fn format_module(module: &Module) -> String {
    let mut f = Formatter::new(&module.comments);
    f.fmt_module(module);
    let mut result = f.output;
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

struct Formatter<'a> {
    output: String,
    indent: usize,
    /// Source comments to interleave during formatting.
    comments: &'a [Comment],
    /// Index of the next comment to consider emitting.
    comment_idx: usize,
}

impl<'a> Formatter<'a> {
    fn new(comments: &'a [Comment]) -> Self {
        Self {
            output: String::new(),
            indent: 0,
            comments,
            comment_idx: 0,
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn writeln(&mut self, s: &str) {
        self.write_indent();
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }

    /// Emit all comments whose position falls before `before_pos`.
    /// If `before_pos` is `None`, emit all remaining comments.
    fn emit_comments_before(&mut self, before_pos: Option<usize>) {
        while self.comment_idx < self.comments.len() {
            let c = &self.comments[self.comment_idx];
            if let Some(pos) = before_pos
                && c.span.start >= pos
            {
                break;
            }
            if c.has_leading_blank_line && !self.output.is_empty() && !self.output.ends_with("\n\n")
            {
                self.newline();
            }
            self.write_indent();
            self.write(&c.text);
            self.output.push('\n');
            self.comment_idx += 1;
        }
    }

    fn fmt_module(&mut self, module: &Module) {
        for (i, item) in module.items.iter().enumerate() {
            if i > 0 {
                self.newline();
            }
            let item_start = item.span().map(|s| s.start);
            self.emit_comments_before(item_start);
            self.fmt_item(item);
        }
        self.emit_comments_before(None);
    }

    fn fmt_item(&mut self, item: &Item) {
        match item {
            Item::Function(f) => self.fmt_fn_def(f),
            Item::Const(c) => self.fmt_const(c),
            Item::StructDef(s) => self.fmt_struct_def(s),
            Item::TypeDef(t) => self.fmt_type_def(t),
            Item::CapabilityDef(c) => self.fmt_capability_def(c),
            Item::CapabilityAlias {
                name, components, ..
            } => {
                self.fmt_capability_alias(name, components);
            }
            Item::ImplDef(i) => self.fmt_impl_def(i),
            Item::Import(i) => self.fmt_import(i),
            Item::Alias(a) => self.fmt_alias(a),
            Item::TraitDef(t) => self.fmt_trait_def(t),
            Item::EffectDef(e) => self.fmt_effect_def(e),
            Item::EffectAlias(ea) => self.fmt_effect_alias(ea),
            Item::HandlerDef(h) => self.fmt_handler_def(h),
        }
    }

    fn fmt_visibility(&mut self, vis: &Visibility) {
        match vis {
            Visibility::Pub => self.write("pub "),
            Visibility::PubPkg => self.write("pub(pkg) "),
            Visibility::Private => {}
        }
    }
}

fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn binop_str(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::BitXor => "^",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
    }
}

fn unaryop_str(op: &UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
        UnaryOp::BitNot => "~",
    }
}
