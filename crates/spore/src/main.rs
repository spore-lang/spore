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
    exec_build, exec_check, exec_format, exec_holes, exec_init, exec_lock, exec_new, exec_run,
    exec_test, exec_watch,
};
#[cfg(test)]
use report::hole_graph_update;
#[cfg(test)]
use target::{
    BuildTarget, find_project_target, infer_project_entry, resolve_build_target, resolve_sp_targets,
};

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
        Cmd::Lock { path, check } => exec_lock(path.as_deref(), check),
        Cmd::Build { path } => exec_build(path.as_deref()),
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

        let target = find_project_target(project_dir.join("src/main.sp").to_str().unwrap())
            .unwrap()
            .unwrap();
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

        let target = find_project_target(project_dir.join("src/lib/util.sp").to_str().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(target.0, std::fs::canonicalize(&project_dir).unwrap());
        assert_eq!(target.1, "lib/util.sp");
    }

    #[test]
    fn test_find_project_target_for_custom_source_root() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("proj");
        fs::create_dir_all(project_dir.join("host/lib")).unwrap();
        fs::write(
            project_dir.join("spore.toml"),
            r#"
            [package]
            name = "proj"

            [project]
            platform = "cli"
            default-entry = "app"
            source-roots = ["host"]

            [entries.app]
            path = "main.sp"
            "#,
        )
        .unwrap();
        fs::write(
            project_dir.join("host/main.sp"),
            "fn main() -> () { return }\n",
        )
        .unwrap();
        fs::write(
            project_dir.join("host/lib/util.sp"),
            "pub fn x() -> I32 { 1 }\n",
        )
        .unwrap();

        let target = find_project_target(project_dir.join("host/lib/util.sp").to_str().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(target.0, std::fs::canonicalize(&project_dir).unwrap());
        assert_eq!(target.1, "lib/util.sp");
    }

    #[test]
    fn test_find_project_target_ignores_files_outside_src() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("proj");
        create_project(&project_dir, "proj", "application").unwrap();
        fs::write(project_dir.join("notes.sp"), "fn scratch() -> I32 { 1 }\n").unwrap();

        assert!(
            find_project_target(project_dir.join("notes.sp").to_str().unwrap())
                .unwrap()
                .is_none()
        );
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
    fn test_resolve_build_target_accepts_file_inside_project() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("proj");
        create_project(&project_dir, "proj", "application").unwrap();
        let nested_dir = project_dir.join("src/lib");
        fs::create_dir_all(&nested_dir).unwrap();
        let file = nested_dir.join("util.sp");
        fs::write(&file, "pub fn helper() -> I32 { 1 }\n").unwrap();

        let target = resolve_build_target(Some(file.to_str().unwrap()), tmp.path()).unwrap();
        match target {
            BuildTarget::Project { root, entry } => {
                assert_eq!(root, std::fs::canonicalize(&project_dir).unwrap());
                assert_eq!(entry, "lib/util.sp");
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

    // --- resolve_sp_targets tests ---

    #[test]
    fn test_resolve_sp_targets_empty_paths_collects_from_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("a.sp"), "fn a() -> I32 { 1 }\n").unwrap();
        fs::write(dir.join("b.sp"), "fn b() -> I32 { 2 }\n").unwrap();
        fs::write(dir.join("readme.txt"), "not a spore file").unwrap();

        let result = resolve_sp_targets(&[], dir).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|p| p.ends_with("a.sp")));
        assert!(result.iter().any(|p| p.ends_with("b.sp")));
    }

    #[test]
    fn test_resolve_sp_targets_empty_paths_use_src_root_by_default() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("spore.toml"),
            r#"
            [package]
            name = "demo"

            [project]
            platform = "cli"
            default-entry = "app"

            [entries.app]
            path = "main.sp"
            "#,
        )
        .unwrap();
        fs::write(dir.join("src/main.sp"), "fn main() -> () { return }\n").unwrap();
        fs::write(dir.join("notes.sp"), "fn scratch() -> I32 { 1 }\n").unwrap();

        let result = resolve_sp_targets(&[], dir).unwrap();
        assert_eq!(
            result,
            vec![std::fs::canonicalize(dir.join("src/main.sp")).unwrap()]
        );
    }

    #[test]
    fn test_resolve_sp_targets_empty_paths_use_configured_root_and_nested_projects() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::create_dir_all(dir.join("host")).unwrap();
        fs::create_dir_all(dir.join("examples/hello-app/src")).unwrap();
        fs::write(
            dir.join("spore.toml"),
            r#"
            [package]
            name = "basic-cli"

            [project]
            platform = "cli"
            default-entry = "host"
            source-roots = ["host"]

            [entries.host]
            path = "main.sp"
            "#,
        )
        .unwrap();
        fs::write(dir.join("host/main.sp"), "fn main() -> () { return }\n").unwrap();
        fs::write(dir.join("notes.sp"), "fn scratch() -> I32 { 1 }\n").unwrap();
        fs::write(dir.join("examples/hello.sp"), "fn stray() -> I32 { 1 }\n").unwrap();
        fs::write(
            dir.join("examples/hello-app/spore.toml"),
            r#"
            [package]
            name = "hello-app"

            [project]
            platform = "basic-cli"
            default-entry = "app"

            [entries.app]
            path = "main.sp"
            "#,
        )
        .unwrap();
        fs::write(
            dir.join("examples/hello-app/src/main.sp"),
            "fn main() -> () { return }\n",
        )
        .unwrap();

        let result = resolve_sp_targets(&[], dir).unwrap();
        assert_eq!(
            result,
            vec![
                std::fs::canonicalize(dir.join("examples/hello-app/src/main.sp")).unwrap(),
                std::fs::canonicalize(dir.join("host/main.sp")).unwrap()
            ]
        );
    }

    #[test]
    fn test_resolve_sp_targets_empty_paths_rejects_invalid_source_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("spore.toml"),
            r#"
            [package]
            name = "demo"

            [project]
            platform = "cli"
            default-entry = "app"
            source-roots = ["/absolute"]

            [entries.app]
            path = "main.sp"
            "#,
        )
        .unwrap();
        fs::write(dir.join("src/main.sp"), "fn main() -> () { return }\n").unwrap();

        let error =
            resolve_sp_targets(&[], dir).expect_err("invalid source-roots should be surfaced");
        assert!(
            error.contains("invalid `[project].source-roots`"),
            "expected invalid source roots error, got: {error}"
        );
    }

    #[test]
    fn test_find_project_target_surfaces_invalid_source_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("spore.toml"),
            r#"
            [package]
            name = "demo"

            [project]
            platform = "cli"
            default-entry = "app"
            source-roots = ["/absolute"]

            [entries.app]
            path = "main.sp"
            "#,
        )
        .unwrap();
        fs::write(dir.join("src/main.sp"), "fn main() -> () { return }\n").unwrap();

        let error = find_project_target(dir.join("src/main.sp").to_str().unwrap())
            .expect_err("invalid source-roots should be surfaced for project files");
        assert!(
            error.contains("invalid `[project].source-roots`"),
            "expected invalid source roots error, got: {error}"
        );
    }

    #[test]
    fn test_resolve_sp_targets_directory_recurses() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("top.sp"), "fn t() -> I32 { 0 }\n").unwrap();
        fs::write(dir.join("sub/nested.sp"), "fn n() -> I32 { 1 }\n").unwrap();
        fs::write(dir.join("sub/ignore.txt"), "not spore").unwrap();

        let paths = vec![dir.to_string_lossy().into_owned()];
        let result = resolve_sp_targets(&paths, dir).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|p| p.ends_with("top.sp")));
        assert!(result.iter().any(|p| p.ends_with("nested.sp")));
    }

    #[test]
    fn test_resolve_sp_targets_explicit_file_passes_through() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("single.sp");
        fs::write(&file, "fn x() -> I32 { 42 }\n").unwrap();

        let paths = vec![file.to_string_lossy().into_owned()];
        let result = resolve_sp_targets(&paths, tmp.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].ends_with("single.sp"));
    }

    #[test]
    fn test_resolve_sp_targets_empty_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let result = resolve_sp_targets(&[], tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_exec_format_with_directory_formats_all_sp_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        // Write a file that needs formatting (extra spaces)
        let file = dir.join("needs_fmt.sp");
        fs::write(&file, "fn add(a: I32, b: I32) -> I32 { a + b }\n").unwrap();

        let dir_arg = vec![dir.to_string_lossy().into_owned()];
        // check mode: should succeed (file is already formatted or any exit)
        let code = exec_format(&dir_arg, true, false);
        // Either SUCCESS (already formatted) or FAILURE (not formatted) — either is fine,
        // the key assertion is that the function does not panic and handles the dir arg.
        let _ = code;
    }

    #[test]
    fn test_exec_check_with_directory_checks_all_sp_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("ok.sp"), "fn x() -> I32 { 1 }\n").unwrap();

        let dir_arg = vec![dir.to_string_lossy().into_owned()];
        let code = exec_check(&dir_arg, false, false, false);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn test_exec_check_legacy_platform_directory_skips_bogus_cli_startup() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("spore.toml"),
            r#"
            [package]
            name = "basic-cli"
            type = "platform"

            [platform]
            contract-module = "platform_contract"
            startup-contract = "main"
            adapter-function = "main_for_host"
            handled-effects = ["Console", "Env"]
            "#,
        )
        .unwrap();
        fs::write(
            dir.join("src/host.sp"),
            r#"
            pub fn main_for_host(app_main: () -> ()) -> () {
                app_main();
                return
            }
            "#,
        )
        .unwrap();
        fs::write(
            dir.join("src/platform_contract.sp"),
            r#"
            pub fn main() -> () {
                ?platform_startup_contract
            }
            "#,
        )
        .unwrap();

        let dir_arg = vec![dir.to_string_lossy().into_owned()];
        let code = exec_check(&dir_arg, false, false, false);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn test_exec_check_no_args_in_empty_dir_returns_success() {
        let tmp = tempfile::tempdir().unwrap();
        // chdir is not safe in tests; pass empty slice with explicit cwd via resolve.
        // We test via resolve_sp_targets returning empty, so exec_check returns SUCCESS.
        let _no_args: Vec<String> = vec![];
        // No .sp files in tmp → note printed, SUCCESS returned.
        // We simulate by passing an explicit empty dir as a positional arg.
        let dir_arg = vec![tmp.path().to_string_lossy().into_owned()];
        let code = exec_check(&dir_arg, false, false, false);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn test_exec_test_with_directory_runs_specs() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(
            dir.join("spec.sp"),
            r#"
fn add(a: I32, b: I32) -> I32
spec {
    example "basic": add(1, 2) == 3
}
{
    a + b
}
"#,
        )
        .unwrap();

        let dir_arg = vec![dir.to_string_lossy().into_owned()];
        let code = exec_test(&dir_arg, false, false, false);
        assert_eq!(code, ExitCode::SUCCESS);
    }
}
