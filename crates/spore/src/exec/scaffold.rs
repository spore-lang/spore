use std::path::Path;
use std::process::ExitCode;

use owo_colors::OwoColorize;

pub(crate) fn exec_new(name: &str, project_type: &str) -> ExitCode {
    if !is_valid_type(project_type) {
        eprintln!(
            "{}: unknown project type `{project_type}`",
            "error".red().bold()
        );
        eprintln!("       valid types: application, package, platform");
        return ExitCode::FAILURE;
    }

    let dir = Path::new(name);
    if dir.exists() {
        eprintln!(
            "{}: directory `{name}` already exists",
            "error".red().bold()
        );
        return ExitCode::FAILURE;
    }

    if let Err(e) = create_project(dir, name, project_type) {
        eprintln!("{}: {e}", "error".red().bold());
        return ExitCode::FAILURE;
    }
    println!("✨ Created {project_type} `{name}`");
    ExitCode::SUCCESS
}

pub(crate) fn exec_init(project_type: &str) -> ExitCode {
    if !is_valid_type(project_type) {
        eprintln!(
            "{}: unknown project type `{project_type}`",
            "error".red().bold()
        );
        eprintln!("       valid types: application, package, platform");
        return ExitCode::FAILURE;
    }

    let dir = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "{}: cannot determine current directory: {e}",
                "error".red().bold()
            );
            return ExitCode::FAILURE;
        }
    };
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();

    if dir.join("spore.toml").exists() {
        eprintln!(
            "{}: spore.toml already exists in this directory",
            "error".red().bold()
        );
        return ExitCode::FAILURE;
    }

    if let Err(e) = create_project(&dir, &name, project_type) {
        eprintln!("{}: {e}", "error".red().bold());
        return ExitCode::FAILURE;
    }
    println!("✨ Initialized {project_type} `{name}`");
    ExitCode::SUCCESS
}

pub(crate) fn is_valid_type(t: &str) -> bool {
    matches!(t, "application" | "package" | "platform")
}

const BASIC_CLI_SCAFFOLD_MANIFEST: &str = "\
[package]
name = \"basic-cli\"
version = \"0.1.0\"
type = \"platform\"
spore-version = \">=0.1.0\"

[platform]
contract-module = \"platform_contract\"
startup-contract = \"main\"
adapter-function = \"main_for_host\"
handled-effects = [\"Console\", \"FileRead\", \"FileWrite\", \"Env\", \"Spawn\"]

[dependencies]
";

const BASIC_CLI_SCAFFOLD_CONTRACT: &str = "\
/// Platform contract module for `basic-cli`.
/// Applications targeting this Platform must implement the same `main`
/// signature in their entry module.
pub fn main() -> () {
    ?platform_startup_contract
}

/// Platform-owned startup adapter.
pub fn main_for_host(app_main: () -> ()) -> () {
    app_main();
    return
}
";

const BASIC_CLI_SCAFFOLD_STDOUT: &str = "\
/// basic-cli platform — Standard output operations
pub foreign fn println(s: Str) -> () uses [Console]
";

const PLATFORM_SCAFFOLD_METADATA: &str = "\
\n[platform]
contract-module = \"platform_contract\"
startup-contract = \"main\"
adapter-function = \"main_for_host\"
handled-effects = []
";

const PLATFORM_SCAFFOLD_CONTRACT: &str = "\
/// Platform contract exposed to application packages.
/// Applications targeting this Platform must implement the same `main`
/// signature in their entry module.
pub fn main() -> () {
    ?platform_startup_contract
}

/// Platform-owned startup adapter.
pub fn main_for_host(app_main: () -> ()) -> () {
    app_main();
    return
}
";

fn write_basic_cli_scaffold(dir: &Path) -> std::io::Result<()> {
    let basic_cli_root = dir.join("vendor").join("basic-cli");
    std::fs::create_dir_all(basic_cli_root.join("src").join("basic_cli"))?;
    std::fs::write(
        basic_cli_root.join("spore.toml"),
        BASIC_CLI_SCAFFOLD_MANIFEST,
    )?;
    std::fs::write(
        basic_cli_root.join("src").join("platform_contract.sp"),
        BASIC_CLI_SCAFFOLD_CONTRACT,
    )?;
    std::fs::write(
        basic_cli_root
            .join("src")
            .join("basic_cli")
            .join("stdout.sp"),
        BASIC_CLI_SCAFFOLD_STDOUT,
    )?;
    Ok(())
}

pub(crate) fn create_project(dir: &Path, name: &str, project_type: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("src"))?;

    let manifest_header = format!(
        "\
[package]
name = \"{name}\"
version = \"0.1.0\"
type = \"{project_type}\"
spore-version = \">=0.1.0\"
"
    );
    let project_config = match project_type {
        "application" => {
            "\n[project]\nplatform = \"basic-cli\"\ndefault-entry = \"app\"\n\n[entries.app]\npath = \"main.sp\"\n".to_string()
        }
        "platform" => {
            "\n[project]\nplatform = \"cli\"\ndefault-entry = \"host\"\n\n[entries.host]\npath = \"host.sp\"\n".to_string()
        }
        _ => String::new(),
    };
    let dependencies = match project_type {
        "application" => "basic-cli = { path = \"vendor/basic-cli\" }\n",
        _ => "",
    };
    let platform_metadata = match project_type {
        "platform" => PLATFORM_SCAFFOLD_METADATA,
        _ => "",
    };
    let toml = format!(
        "{manifest_header}{project_config}{platform_metadata}\n[dependencies]\n{dependencies}"
    );
    std::fs::write(dir.join("spore.toml"), toml)?;

    let (filename, content) = match project_type {
        "package" => (
            "lib.sp",
            "/// Add two integers.\npub fn add(a: I64, b: I64) -> I64 cost [1, 0, 0, 0] {\n    a + b\n}\n"
                .to_string(),
        ),
        "platform" => (
            "host.sp",
            "/// Platform host entry.\n/// This placeholder satisfies the current CLI startup contract while runtime host wiring is still evolving.\npub fn main() -> () {\n    return\n}\n"
                .to_string(),
        ),
        _ => (
            "main.sp",
            format!(
                "import basic_cli.stdout\n\nfn main() -> () uses [Console] {{\n    println(\"Hello from {name}!\");\n    return\n}}\n"
            ),
        ),
    };
    std::fs::write(dir.join("src").join(filename), content)?;
    if project_type == "application" {
        write_basic_cli_scaffold(dir)?;
    } else if project_type == "platform" {
        std::fs::write(
            dir.join("src").join("platform_contract.sp"),
            PLATFORM_SCAFFOLD_CONTRACT,
        )?;
    }
    std::fs::write(dir.join(".gitignore"), "/target\n/.spore-store\n")?;
    Ok(())
}
