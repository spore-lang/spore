//! Intent-signature context projection for typed holes.
//!
//! This module attaches source-level intent clauses that are already checked
//! elsewhere to the HoleReport boundary consumed by humans and tools.

use std::collections::BTreeMap;

use sporec_parser::ast::{Item, Module};

use crate::hole::{HoleReport, PropertyContext};

/// Attach enclosing source properties to holes in the same function.
pub fn enrich_hole_report_with_properties(module: &Module, report: &mut HoleReport) {
    let mut properties_by_function = BTreeMap::new();

    for item in &module.items {
        let Item::Function(function) = item else {
            continue;
        };
        let Some(properties) = &function.properties_clause else {
            continue;
        };

        let property_names = properties
            .items
            .iter()
            .map(|property| property.name.clone())
            .collect::<Vec<_>>();
        if !property_names.is_empty() {
            properties_by_function.insert(
                function.name.clone(),
                PropertyContext {
                    properties: property_names,
                },
            );
        }
    }

    for hole in &mut report.holes {
        if let Some(context) = properties_by_function.get(&hole.function) {
            hole.property_context = Some(context.clone());
        }
    }
}
