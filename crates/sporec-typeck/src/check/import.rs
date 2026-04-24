use super::*;

impl Checker {
    // ── Registration (first pass) ───────────────────────────────────

    /// Resolve an import declaration, importing symbols into the current registry.
    pub(super) fn resolve_import(&mut self, import: &ImportDecl) {
        let (path, alias) = match import {
            ImportDecl::Import { path, alias, .. } => (path.as_str(), alias.as_str()),
            ImportDecl::Alias { name, path, .. } => (path.as_str(), name.as_str()),
        };
        let path_segments: Vec<String> = path.split('.').map(|s| s.to_string()).collect();
        let module = match self.module_registry.get(&path_segments) {
            Some(m) => m.clone(),
            None => {
                self.err(ErrorCode::M0001, format!("module `{path}` not found"));
                return;
            }
        };
        let all_names = module.all_exports();
        match self
            .module_registry
            .resolve_import(&path_segments, &all_names)
        {
            Ok(resolved) => {
                if !self.current_module_name.is_empty() {
                    self.module_registry
                        .record_dependency(&self.current_module_name, path);
                }
                self.import_resolved_symbols(&module, &resolved, alias);
            }
            Err(ModuleError::PrivateSymbol { symbol, module: m }) => {
                self.err(
                    ErrorCode::M0003,
                    format!("symbol `{symbol}` in module `{m}` is private and not accessible"),
                );
            }
            Err(ModuleError::SymbolNotFound { symbol, module: m }) => {
                self.err(
                    ErrorCode::M0002,
                    format!("symbol `{symbol}` not found in module `{m}`"),
                );
            }
            Err(ModuleError::ModuleNotFound(m)) => {
                self.err(ErrorCode::M0001, format!("module `{m}` not found"));
            }
            Err(ModuleError::CircularDependency(cycle)) => {
                self.err(
                    ErrorCode::M0101,
                    format!("circular module dependency: {}", cycle.join(" -> ")),
                );
            }
            Err(ModuleError::IoError { module: m, detail }) => {
                self.err(
                    ErrorCode::M0001,
                    format!("cannot read module `{m}`: {detail}"),
                );
            }
            Err(ModuleError::ParseErrors { module: m, errors }) => {
                self.err(
                    ErrorCode::M0001,
                    format!(
                        "parse error in module `{m}`: {}",
                        errors
                            .into_iter()
                            .map(|error| error.to_string())
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                );
            }
        }
    }

    /// Import resolved symbols from a module into the current type registry.
    ///
    /// When `alias` differs from the module's own leaf name, symbols are
    /// registered under `alias.symbol` so that user code can write
    /// `Alias.func(…)` instead of `Original.func(…)`.
    pub(super) fn import_resolved_symbols(
        &mut self,
        module: &crate::module::ModuleInterface,
        resolved: &[(String, ImportedSymbol)],
        alias: &str,
    ) {
        let _alias_prefix: Option<&str> = if alias.is_empty() { None } else { Some(alias) };

        for (name, kind) in resolved {
            match kind {
                ImportedSymbol::Function => {
                    if let Some((params, ret)) = module.functions.get(name) {
                        let required_effects = module
                            .function_required_effects
                            .get(name)
                            .cloned()
                            .unwrap_or_default();
                        let errors = module
                            .function_errors
                            .get(name)
                            .cloned()
                            .unwrap_or_default();
                        let type_params = module
                            .function_type_params
                            .get(name)
                            .cloned()
                            .unwrap_or_default();
                        let where_bounds = module
                            .function_where_bounds
                            .get(name)
                            .cloned()
                            .unwrap_or_default();
                        if let Some(existing) = self.registry.functions.get(name) {
                            let existing_errors = self
                                .registry
                                .fn_errors
                                .get(name)
                                .cloned()
                                .unwrap_or_default();
                            let existing_type_params = self
                                .registry
                                .fn_type_params
                                .get(name)
                                .cloned()
                                .unwrap_or_default();
                            let existing_where_bounds = self
                                .registry
                                .fn_where_bounds
                                .get(name)
                                .cloned()
                                .unwrap_or_default();
                            if existing.0 != *params
                                || existing.1 != *ret
                                || existing.2 != required_effects
                                || existing_errors != errors
                                || existing_type_params != type_params
                                || existing_where_bounds != where_bounds
                            {
                                self.err(
                                    ErrorCode::M0303,
                                    format!(
                                        "ambiguous import: `{name}` is exported by multiple imported modules"
                                    ),
                                );
                                continue;
                            }
                        }
                        self.registry.functions.insert(
                            name.clone(),
                            (params.clone(), ret.clone(), required_effects),
                        );
                        if !errors.is_empty() {
                            self.registry.fn_errors.insert(name.clone(), errors);
                        }
                        if !type_params.is_empty() {
                            self.registry
                                .fn_type_params
                                .insert(name.clone(), type_params);
                        }
                        if !where_bounds.is_empty() {
                            self.registry
                                .fn_where_bounds
                                .insert(name.clone(), where_bounds);
                        }
                    }
                }
                ImportedSymbol::Type => {
                    if let Some(variants) = module.types.get(name) {
                        self.registry.types.insert(name.clone(), variants.clone());
                    }
                }
                ImportedSymbol::Struct => {
                    if let Some(fields) = module.structs.get(name) {
                        self.registry.structs.insert(name.clone(), fields.clone());
                    }
                    if let Some(type_params) = module.struct_type_params.get(name) {
                        self.registry
                            .struct_type_params
                            .insert(name.clone(), type_params.clone());
                    }
                }
                ImportedSymbol::Handler => {
                    if let Some(handler) = module.handlers.get(name) {
                        if let Some(existing) = self.registry.handlers.get(name)
                            && existing != handler
                        {
                            self.err(
                                ErrorCode::M0303,
                                format!(
                                    "ambiguous import: `{name}` is exported by multiple imported modules"
                                ),
                            );
                            continue;
                        }
                        self.registry.handlers.insert(name.clone(), handler.clone());
                    }
                }
                ImportedSymbol::Interface => {
                    if module.interfaces.contains(name) {
                        let methods = module
                            .interface_members
                            .get(name)
                            .cloned()
                            .unwrap_or((Vec::new(), Vec::new()));
                        if let Some(existing) = self.registry.interfaces.get(name)
                            && existing != &methods
                        {
                            self.err(
                                ErrorCode::M0303,
                                format!(
                                    "ambiguous import: `{name}` is exported by multiple imported modules"
                                ),
                            );
                            continue;
                        }
                        self.registry.interfaces.insert(name.clone(), methods);
                    }
                }
            }
        }
    }
}
