mod cli;
mod exec;
mod report;
mod target;
mod util;

use std::process::ExitCode;

use cli::{Cmd, cli};
#[cfg(test)]
use exec::{create_project, is_valid_type};
use exec::{
    exec_build, exec_check, exec_format, exec_holes, exec_init, exec_new, exec_run, exec_test,
    exec_watch,
};
#[cfg(test)]
use report::hole_graph_update;
#[cfg(test)]
use target::{BuildTarget, find_project_target, infer_project_entry, resolve_build_target};

fn main() -> ExitCode {
    let cmd = cli().run();
    match cmd {
        Cmd::Run { file, json } => exec_run(&file, json),
        Cmd::Check {
            files,
            verbose,
            json,
            deny_warnings,
        } => exec_check(&files, verbose, json, deny_warnings),
        Cmd::Test {
            files,
            verbose,
            json,
            deny_warnings,
        } => exec_test(&files, verbose, json, deny_warnings),
        Cmd::Format { files, check, diff } => exec_format(&files, check, diff),
        Cmd::Holes { file } => exec_holes(&file),
        Cmd::Build { file } => exec_build(file.as_deref()),
        Cmd::Watch { file, json } => exec_watch(&file, json),
        Cmd::New { name, project_type } => exec_new(&name, &project_type),
        Cmd::Init { project_type } => exec_init(&project_type),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_new_creates_application() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("my-app");
        create_project(&project_dir, "my-app", "application").unwrap();
        assert!(project_dir.join("spore.toml").exists());
        assert!(project_dir.join("src/main.sp").exists());
        let toml = fs::read_to_string(project_dir.join("spore.toml")).unwrap();
        assert!(toml.contains("name = \"my-app\""));
        assert!(toml.contains("type = \"application\""));
        assert!(toml.contains("[project]"));
        assert!(toml.contains("platform = \"basic-cli\""));
        assert!(toml.contains("default-entry = \"app\""));
        assert!(toml.contains("[entries.app]"));
        assert!(toml.contains("basic-cli = { path = \"vendor/basic-cli\" }"));
        assert!(project_dir.join("vendor/basic-cli/spore.toml").exists());
        assert!(
            project_dir
                .join("vendor/basic-cli/src/platform_contract.sp")
                .exists()
        );
        assert!(
            project_dir
                .join("vendor/basic-cli/src/basic_cli/stdout.sp")
                .exists()
        );
        let main = fs::read_to_string(project_dir.join("src/main.sp")).unwrap();
        assert!(main.contains("import basic_cli.stdout"));
        assert!(main.contains("uses [Console]"));
    }

    #[test]
    fn test_new_creates_package() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("my-lib");
        create_project(&project_dir, "my-lib", "package").unwrap();
        assert!(project_dir.join("src/lib.sp").exists());
        let toml = fs::read_to_string(project_dir.join("spore.toml")).unwrap();
        assert!(toml.contains("type = \"package\""));
        assert!(!toml.contains("[project]"));
    }

    #[test]
    fn test_new_creates_platform() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("my-platform");
        create_project(&project_dir, "my-platform", "platform").unwrap();
        assert!(project_dir.join("src/host.sp").exists());
        let toml = fs::read_to_string(project_dir.join("spore.toml")).unwrap();
        assert!(toml.contains("type = \"platform\""));
        assert!(toml.contains("[project]"));
        assert!(toml.contains("default-entry = \"host\""));
        assert!(toml.contains("[entries.host]"));
        let host = fs::read_to_string(project_dir.join("src/host.sp")).unwrap();
        assert!(host.contains("pub fn main() -> ()"));
    }

    #[test]
    fn test_is_valid_type() {
        assert!(is_valid_type("application"));
        assert!(is_valid_type("package"));
        assert!(is_valid_type("platform"));
        assert!(!is_valid_type("unknown"));
    }

    #[test]
    fn test_gitignore_content() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("test-proj");
        create_project(&project_dir, "test-proj", "application").unwrap();
        let gi = fs::read_to_string(project_dir.join(".gitignore")).unwrap();
        assert!(gi.contains("/target"));
    }

    #[test]
    fn test_scaffolded_projects_typecheck() {
        let tmp = tempfile::tempdir().unwrap();
        let cases = [
            ("application", "app", "main.sp"),
            ("package", "pkg", "lib.sp"),
            ("platform", "plat", "host.sp"),
        ];

        for (project_type, name, entry) in cases {
            let project_dir = tmp.path().join(name);
            create_project(&project_dir, name, project_type).unwrap();

            let result = sporec_driver::compile_project(&project_dir, entry);
            assert!(
                result.is_ok(),
                "scaffolded {project_type} project should type-check: {result:?}"
            );
        }
    }

    #[test]
    fn test_exec_test_accepts_valid_spec_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("sample.sp");
        fs::write(
            &file,
            r#"
            fn add(a: I32, b: I32) -> I32
            spec {
                example "basic": add(2, 3) == 5
                property "left_identity": |a: I32, b: I32 when self == 0| a
            }
            {
                a + b
            }
            "#,
        )
        .unwrap();

        let code = exec_test(&[file.to_string_lossy().to_string()], false, false, false);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn test_exec_test_rejects_invalid_spec_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("sample.sp");
        fs::write(
            &file,
            r#"
            fn add(a: I32, b: I32) -> I32
            spec {
                example "bad": 42
            }
            {
                a + b
            }
            "#,
        )
        .unwrap();

        let code = exec_test(&[file.to_string_lossy().to_string()], false, false, false);
        assert_eq!(code, ExitCode::FAILURE);
    }

    #[test]
    fn test_exec_test_rejects_type_errors_before_running_specs() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("sample.sp");
        fs::write(
            &file,
            r#"
            fn add(a: I32, b: I32) -> I32
            spec {
                example "basic": add(2, 3) == 5
            }
            {
                "oops"
            }
            "#,
        )
        .unwrap();

        let code = exec_test(&[file.to_string_lossy().to_string()], false, false, false);
        assert_eq!(code, ExitCode::FAILURE);
    }

    #[test]
    fn test_exec_test_denies_warnings_when_requested() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("sample.sp");
        fs::write(
            &file,
            r#"
            fn expensive(x: I32) -> I32 cost [100, 0, 0, 0] {
                x + x
            }

            fn cheap(a: I32) -> I32 cost [2, 0, 0, 0]
            spec {
                example "basic": cheap(1) == 4
            }
            {
                expensive(expensive(a))
            }
            "#,
        )
        .unwrap();

        let code = exec_test(&[file.to_string_lossy().to_string()], false, false, true);
        assert_eq!(code, ExitCode::FAILURE);
    }

    #[test]
    fn test_exec_check_verbose_denies_warnings_when_requested() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("sample.sp");
        fs::write(
            &file,
            r#"
            fn expensive(x: I32) -> I32 cost [100, 0, 0, 0] {
                x + x
            }

            fn cheap(a: I32) -> I32 cost [2, 0, 0, 0] {
                expensive(expensive(a))
            }
            "#,
        )
        .unwrap();

        let code = exec_check(&[file.to_string_lossy().to_string()], true, false, true);
        assert_eq!(code, ExitCode::FAILURE);
    }

    #[test]
    fn test_find_project_target_for_main_file() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("proj");
        create_project(&project_dir, "proj", "application").unwrap();

        let target =
            find_project_target(project_dir.join("src/main.sp").to_str().unwrap()).unwrap();
        assert_eq!(target.0, std::fs::canonicalize(&project_dir).unwrap());
        assert_eq!(target.1, "main.sp");
    }

    #[test]
    fn test_find_project_target_for_nested_module() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("proj");
        create_project(&project_dir, "proj", "application").unwrap();
        let nested_dir = project_dir.join("src/lib");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(nested_dir.join("util.sp"), "pub fn x() -> I32 { 1 }\n").unwrap();

        let target =
            find_project_target(project_dir.join("src/lib/util.sp").to_str().unwrap()).unwrap();
        assert_eq!(target.0, std::fs::canonicalize(&project_dir).unwrap());
        assert_eq!(target.1, "lib/util.sp");
    }

    #[test]
    fn test_find_project_target_ignores_files_outside_src() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("proj");
        create_project(&project_dir, "proj", "application").unwrap();
        fs::write(project_dir.join("notes.sp"), "fn scratch() -> I32 { 1 }\n").unwrap();

        assert!(find_project_target(project_dir.join("notes.sp").to_str().unwrap()).is_none());
    }

    #[test]
    fn test_resolve_build_target_without_arg_uses_project_root() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("proj");
        create_project(&project_dir, "proj", "application").unwrap();

        let target = resolve_build_target(None, &project_dir).unwrap();
        match target {
            BuildTarget::Project { root, entry } => {
                assert_eq!(root, std::fs::canonicalize(&project_dir).unwrap());
                assert_eq!(entry, "main.sp");
            }
            BuildTarget::File(path) => panic!("expected project target, got file target `{path}`"),
        }
    }

    #[test]
    fn test_resolve_build_target_accepts_dot_for_project_root() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("pkg");
        create_project(&project_dir, "pkg", "package").unwrap();

        let target = resolve_build_target(Some("."), &project_dir).unwrap();

        match target {
            BuildTarget::Project { root, entry } => {
                assert_eq!(root, std::fs::canonicalize(&project_dir).unwrap());
                assert_eq!(entry, "lib.sp");
            }
            BuildTarget::File(path) => panic!("expected project target, got file target `{path}`"),
        }
    }

    #[test]
    fn test_resolve_build_target_accepts_project_directory_argument() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("plat");
        create_project(&project_dir, "plat", "platform").unwrap();

        let target = resolve_build_target(Some(project_dir.to_str().unwrap()), tmp.path()).unwrap();
        match target {
            BuildTarget::Project { root, entry } => {
                assert_eq!(root, std::fs::canonicalize(&project_dir).unwrap());
                assert_eq!(entry, "host.sp");
            }
            BuildTarget::File(path) => panic!("expected project target, got file target `{path}`"),
        }
    }

    #[test]
    fn test_infer_project_entry_falls_back_to_single_default_file() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("proj");
        fs::create_dir_all(project_dir.join("src")).unwrap();
        fs::write(
            project_dir.join("spore.toml"),
            "[package]\nname = \"proj\"\n",
        )
        .unwrap();
        fs::write(
            project_dir.join("src/main.sp"),
            "fn main() -> () { return }\n",
        )
        .unwrap();

        assert_eq!(infer_project_entry(&project_dir).unwrap(), "main.sp");
    }

    #[test]
    fn test_hole_graph_update_emits_json_event_for_sources_with_holes() {
        let summary = hole_graph_update("fn main() -> I32 { ?todo }\n", true)
            .expect("hole-bearing source should produce a watch summary");
        let value = serde_json::to_value(&summary).expect("serialize hole summary");

        assert_eq!(value["event"], "hole_graph_update");
        assert!(
            value["holes_total"]
                .as_u64()
                .is_some_and(|count| count >= 1)
        );
    }

    #[test]
    fn test_hole_graph_update_skips_non_json_and_hole_free_sources() {
        assert!(hole_graph_update("fn main() -> I32 { ?todo }\n", false).is_none());
        assert!(hole_graph_update("fn main() -> I32 { 42 }\n", true).is_none());
    }
}
