#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdlibModule {
    pub logical_name: &'static str,
    pub file_name: &'static str,
    pub source: &'static str,
}

static MODULES: [StdlibModule; 11] = [
    StdlibModule {
        logical_name: "prelude",
        file_name: "prelude.sp",
        source: include_str!("../../../stdlib/prelude.sp"),
    },
    StdlibModule {
        logical_name: "math",
        file_name: "math.sp",
        source: include_str!("../../../stdlib/math.sp"),
    },
    StdlibModule {
        logical_name: "string",
        file_name: "string.sp",
        source: include_str!("../../../stdlib/string.sp"),
    },
    StdlibModule {
        logical_name: "collections",
        file_name: "collections.sp",
        source: include_str!("../../../stdlib/collections.sp"),
    },
    StdlibModule {
        logical_name: "dict",
        file_name: "dict.sp",
        source: include_str!("../../../stdlib/dict.sp"),
    },
    StdlibModule {
        logical_name: "set",
        file_name: "set.sp",
        source: include_str!("../../../stdlib/set.sp"),
    },
    StdlibModule {
        logical_name: "char",
        file_name: "char.sp",
        source: include_str!("../../../stdlib/char.sp"),
    },
    StdlibModule {
        logical_name: "spore.combine",
        file_name: "spore/combine.sp",
        source: include_str!("../../../stdlib/spore/combine.sp"),
    },
    StdlibModule {
        logical_name: "spore.merge",
        file_name: "spore/merge.sp",
        source: include_str!("../../../stdlib/spore/merge.sp"),
    },
    StdlibModule {
        logical_name: "spore.order",
        file_name: "spore/order.sp",
        source: include_str!("../../../stdlib/spore/order.sp"),
    },
    StdlibModule {
        logical_name: "spore.laws",
        file_name: "spore/laws.sp",
        source: include_str!("../../../stdlib/spore/laws.sp"),
    },
];

pub fn prelude() -> &'static StdlibModule {
    &MODULES[0]
}

pub fn all() -> &'static [StdlibModule] {
    &MODULES
}

pub fn get(logical_name: &str) -> Option<&'static StdlibModule> {
    MODULES
        .iter()
        .find(|module| module.logical_name == logical_name)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use super::{MODULES, all, get, prelude};

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root must exist")
            .to_path_buf()
    }

    fn collect_stdlib_files(dir: PathBuf) -> BTreeSet<String> {
        let mut files = BTreeSet::new();
        let mut stack = vec![dir.clone()];
        while let Some(path) = stack.pop() {
            for entry in std::fs::read_dir(&path).expect("stdlib directory should be readable") {
                let entry = entry.expect("stdlib entry should be readable");
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    stack.push(entry_path);
                    continue;
                }
                if entry_path.extension().is_none_or(|ext| ext != "sp") {
                    continue;
                }
                let rel = entry_path
                    .strip_prefix(&dir)
                    .expect("stdlib file should stay under stdlib root")
                    .to_string_lossy()
                    .replace('\\', "/");
                files.insert(rel);
            }
        }
        files
    }

    #[test]
    fn prelude_lookup_round_trips() {
        assert_eq!(Some(prelude()), get("prelude"));
    }

    #[test]
    fn compositional_module_lookup_round_trips() {
        assert_eq!(Some(&MODULES[8]), get("spore.merge"));
        assert_eq!(Some(&MODULES[10]), get("spore.laws"));
    }

    #[test]
    fn registry_matches_stdlib_directory() {
        let expected = collect_stdlib_files(workspace_root().join("stdlib"));

        let actual: BTreeSet<_> = all()
            .iter()
            .map(|module| module.file_name.to_string())
            .collect();

        assert_eq!(actual, expected);
    }

    #[test]
    fn python_packaging_includes_stdlib_tree() {
        let pyproject = std::fs::read_to_string(workspace_root().join("pyproject.toml"))
            .expect("pyproject.toml should exist");

        assert!(
            pyproject.contains("stdlib/**/*.sp"),
            "python packaging must include the bundled stdlib tree",
        );
    }
}
