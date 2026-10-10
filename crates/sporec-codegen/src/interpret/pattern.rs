use sporec_parser::ast::Pattern;

use crate::value::Value;

use super::Interpreter;

impl Interpreter {
    pub(super) fn match_pattern(
        &mut self,
        pat: &Pattern,
        val: &Value,
    ) -> Option<Vec<(String, Value)>> {
        match pat {
            Pattern::Wildcard => Some(vec![]),
            Pattern::Var(name) => Some(vec![(name.clone(), val.clone())]),
            Pattern::IntLit(n) => {
                if val.as_int() == Some(*n) {
                    Some(vec![])
                } else {
                    None
                }
            }
            Pattern::StrLit(s) => {
                if val.as_str() == Some(s) {
                    Some(vec![])
                } else {
                    None
                }
            }
            Pattern::BoolLit(b) => {
                if val.as_bool() == Some(*b) {
                    Some(vec![])
                } else {
                    None
                }
            }
            Pattern::OutcomeOk(inner) => {
                if let Value::OutcomeOk(value) = val {
                    self.match_pattern(inner, value)
                } else {
                    None
                }
            }
            Pattern::OutcomeFail(inner) => {
                if let Value::OutcomeFail(value) = val {
                    self.match_pattern(inner, value)
                } else {
                    None
                }
            }
            Pattern::Constructor(name, sub_pats) => {
                if let Value::Enum(vname, fields) = val {
                    if vname != name {
                        return None;
                    }
                    if fields.len() != sub_pats.len() {
                        return None;
                    }
                    let mut bindings = Vec::new();
                    for (sp, field_val) in sub_pats.iter().zip(fields.iter()) {
                        let sub_bindings = self.match_pattern(sp, field_val)?;
                        bindings.extend(sub_bindings);
                    }
                    return Some(bindings);
                }
                if let Value::Struct(vname, fields) = val {
                    if vname != name {
                        return None;
                    }
                    let mut bindings = Vec::new();
                    for (i, sp) in sub_pats.iter().enumerate() {
                        let field_val = fields.get(&i.to_string())?;
                        let sub_bindings = self.match_pattern(sp, field_val)?;
                        bindings.extend(sub_bindings);
                    }
                    Some(bindings)
                } else if sub_pats.is_empty()
                    && matches!(val, Value::Enum(vname, fields) if vname == name && fields.is_empty())
                {
                    Some(vec![])
                } else {
                    None
                }
            }
            Pattern::Struct(name, field_pats) => {
                if let Value::Struct(sname, fields) = val {
                    if sname != name {
                        return None;
                    }
                    let mut bindings = Vec::new();
                    for (fname, fpat) in field_pats {
                        let fval = fields.get(fname)?;
                        let sub_bindings = self.match_pattern(fpat, fval)?;
                        bindings.extend(sub_bindings);
                    }
                    Some(bindings)
                } else {
                    None
                }
            }
            Pattern::Or(alternatives) => {
                for alt in alternatives {
                    if let Some(bindings) = self.match_pattern(alt, val) {
                        return Some(bindings);
                    }
                }
                None
            }
            Pattern::List(elements, rest) => {
                if let Value::List(items) = val {
                    if rest.is_some() {
                        if items.len() < elements.len() {
                            return None;
                        }
                    } else if items.len() != elements.len() {
                        return None;
                    }
                    let mut bindings = Vec::new();
                    for (pat, item) in elements.iter().zip(items.iter()) {
                        let sub = self.match_pattern(pat, item)?;
                        bindings.extend(sub);
                    }
                    if let Some(rest_name) = rest {
                        let rest_items = items[elements.len()..].to_vec();
                        bindings.push((rest_name.clone(), Value::List(rest_items)));
                    }
                    Some(bindings)
                } else {
                    None
                }
            }
        }
    }
}
