mod cli;
mod exec;
mod report;
mod target;
mod util;

use std::process::ExitCode;

use cli::{Cmd, cli};
use exec::{
    exec_build, exec_check, exec_format, exec_holes, exec_init, exec_lock, exec_new, exec_run,
    exec_test, exec_watch,
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
mod tests;
