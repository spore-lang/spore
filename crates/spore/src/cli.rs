use bpaf::*;

use crate::util::VERSION;

#[derive(Debug, Clone)]
pub(crate) enum Cmd {
    Run {
        file: String,
        json: bool,
    },
    Check {
        files: Vec<String>,
        verbose: bool,
        json: bool,
        deny_warnings: bool,
    },
    Test {
        files: Vec<String>,
        verbose: bool,
        json: bool,
        deny_warnings: bool,
    },
    Format {
        files: Vec<String>,
        check: bool,
        diff: bool,
    },
    Holes {
        file: String,
    },
    Build {
        file: String,
    },
    Watch {
        file: String,
        json: bool,
    },
    New {
        name: String,
        project_type: String,
    },
    Init {
        project_type: String,
    },
}

fn json_flag() -> impl Parser<bool> {
    long("json").help("Output results as JSON").switch()
}

fn cmd_run_parser() -> impl Parser<Cmd> {
    let json = json_flag();
    let file = positional::<String>("FILE").help("A .sp file to run");
    construct!(Cmd::Run { json, file })
        .to_options()
        .descr("Compile and execute a .sp file")
        .command("run")
}

fn cmd_check_parser() -> impl Parser<Cmd> {
    let verbose = long("verbose")
        .help("Show detailed type inference and cost info")
        .switch();
    let json = json_flag();
    let deny_warnings = long("deny-warnings")
        .help("Treat warnings as errors")
        .switch();
    let files = positional::<String>("PATH")
        .help(".sp file(s) or director(ies) to check (default: current directory)")
        .many();
    construct!(Cmd::Check {
        verbose,
        json,
        deny_warnings,
        files,
    })
    .to_options()
    .descr("Type-check .sp files. Accepts files, directories, or no args (uses current directory).")
    .command("check")
}

fn cmd_test_parser() -> impl Parser<Cmd> {
    let verbose = long("verbose")
        .help("Show detailed type inference and cost info")
        .switch();
    let json = json_flag();
    let deny_warnings = long("deny-warnings")
        .help("Treat warnings as errors")
        .switch();
    let files = positional::<String>("PATH")
        .help(".sp file(s) or director(ies) to test (default: current directory)")
        .many();
    construct!(Cmd::Test {
        verbose,
        json,
        deny_warnings,
        files,
    })
    .to_options()
    .descr("Execute spec examples and properties in .sp files. Accepts files, directories, or no args (uses current directory).")
    .command("test")
}

fn cmd_format_parser() -> impl Parser<Cmd> {
    let fmt_inner = || {
        let check = long("check")
            .help("Check if files are formatted (no changes)")
            .switch();
        let diff = long("diff").help("Show diff instead of rewriting").switch();
        let files = positional::<String>("PATH")
            .help(".sp file(s) or director(ies) to format (default: current directory)")
            .many();
        construct!(Cmd::Format { check, diff, files })
    };

    let format_cmd = fmt_inner()
        .to_options()
        .descr("Format .sp files. Accepts files, directories, or no args (uses current directory).")
        .command("format");

    let fmt_cmd = fmt_inner()
        .to_options()
        .descr("Format .sp files (alias for format). Accepts files, directories, or no args (uses current directory).")
        .command("fmt");

    construct!([format_cmd, fmt_cmd])
}

fn cmd_holes_parser() -> impl Parser<Cmd> {
    let file = positional::<String>("FILE").help("A .sp file");
    construct!(Cmd::Holes { file })
        .to_options()
        .descr("Show hole report (JSON)")
        .command("holes")
}

fn cmd_build_parser() -> impl Parser<Cmd> {
    let file = positional::<String>("FILE")
        .help("A standalone .sp file to compile to a native object file");
    construct!(Cmd::Build { file })
        .to_options()
        .descr("Compile a standalone .sp file to a native .o object file (projects unsupported)")
        .command("build")
}

fn cmd_watch_parser() -> impl Parser<Cmd> {
    let json = json_flag();
    let file = positional::<String>("FILE").help("A .sp file to watch");
    construct!(Cmd::Watch { json, file })
        .to_options()
        .descr("Watch a file and re-check on changes")
        .command("watch")
}

fn type_flag() -> impl Parser<String> {
    long("type")
        .short('t')
        .help("Project type: application, package, platform")
        .argument::<String>("TYPE")
        .fallback("application".to_string())
}

fn cmd_new_parser() -> impl Parser<Cmd> {
    let name = positional::<String>("NAME").help("Project name");
    let project_type = type_flag();
    construct!(Cmd::New { name, project_type })
        .to_options()
        .descr("Create a new Spore project")
        .command("new")
}

fn cmd_init_parser() -> impl Parser<Cmd> {
    let project_type = type_flag();
    construct!(Cmd::Init { project_type })
        .to_options()
        .descr("Initialize Spore project in current directory")
        .command("init")
}

pub(crate) fn cli() -> OptionParser<Cmd> {
    let cmd = construct!([
        cmd_run_parser(),
        cmd_check_parser(),
        cmd_test_parser(),
        cmd_format_parser(),
        cmd_holes_parser(),
        cmd_build_parser(),
        cmd_watch_parser(),
        cmd_new_parser(),
        cmd_init_parser(),
    ]);
    cmd.to_options()
        .version(VERSION)
        .descr("spore — the Spore language toolkit")
}
