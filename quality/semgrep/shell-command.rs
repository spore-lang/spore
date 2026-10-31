fn examples(command: &str) {
    // ruleid: shell-command
    Command::new("sh").arg("-c").arg(command);
    // ruleid: shell-command
    std::process::Command::new("bash").arg("-c").arg(command);
    // ruleid: shell-command
    Command::new("cmd.exe").arg("/C").arg(command);
    // ruleid: shell-command
    Command::new("sh").args(["-c", command]);
    // ok: shell-command
    Command::new(command).args(["--help"]);
    // ok: shell-command
    Command::new("git").arg("show").arg(command);
}
