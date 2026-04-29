#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdlibModule {
    pub logical_name: &'static str,
    pub file_name: &'static str,
    pub source: &'static str,
}

static MODULES: [StdlibModule; 7] = [
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

    use super::{all, get, prelude};

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root must exist")
            .to_path_buf()
    }

    #[test]
    fn prelude_lookup_round_trips() {
        assert_eq!(Some(prelude()), get("prelude"));
    }

    #[test]
    fn registry_matches_stdlib_directory() {
        let expected: BTreeSet<_> = std::fs::read_dir(workspace_root().join("stdlib"))
            .expect("stdlib directory should exist")
            .map(|entry| {
                entry
                    .expect("stdlib entry should be readable")
                    .file_name()
                    .into_string()
                    .expect("stdlib file name should be valid UTF-8")
            })
            .filter(|file_name| file_name.ends_with(".sp"))
            .collect();

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
