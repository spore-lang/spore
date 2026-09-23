# AGENTS.md

Repository-wide standing orders for coding agents working on Spore.

Human-oriented product overview and install paths live in [`README.md`](./README.md).
Do not invent language syntax, effects, or CLI behavior beyond what README, the
compiler, and tests show.

## Precedence

- This file applies to the entire repository.
- A more specific `AGENTS.md` in a subtree, when present, augments or overrides
  this file for that subtree.
- Follow the user's explicit task and preserve unrelated work.

## Working rules

- Prefer a dedicated git worktree for edits; do not disturb unrelated
  uncommitted work in the main worktree.
- Do not commit secrets, `.env` files, or credentials.
- Keep facts in one authoritative place and link instead of copying.
- When user-facing install steps or surface syntax change, update `README.md`
  in the same change.

## Validation

Use the repository `justfile` entry points and report what actually ran:

- `just format` — format Justfile and Rust sources
- `just check` — `cargo fmt --check` and Clippy (`-D warnings`)
- `just pre-commit` — run prek hooks on all files
- `cargo test --all` — full compiler/test suite when behavior changes

Coverage recipes (`just cov`, `just cov-open`) and MSRV (`just msrv`) are
optional when relevant; do not claim they ran unless they did.


## Security

Report vulnerabilities **privately** via [`SECURITY.md`](./SECURITY.md)
(GitHub Security Advisories). Do not open public issues for security findings.

## Pull requests

- Keep the change focused.
- Use [`.github/pull_request_template.md`](./.github/pull_request_template.md)
  (`Summary`, `Validation`, `Notes`).
- In the PR, state what changed, what was validated, and what was not verified.
