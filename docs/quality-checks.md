# Quality checks

Run deterministic checks with `just check`. It leaves source
files unchanged. Use `just format` for Rust/Justfile repair and `spore format` for
explicit Spore source formatting. The published `spore-format` consumer hook retains
its existing repair behavior; this repository's CI hook passes `--check`.

| Entry point | Evidence produced |
| --- | --- |
| `cargo test --all --locked` | Compiler, CLI, integration and documentation tests |
| `just coverage` | Workspace LLVM coverage in `lcov.info`, followed by doctests |
| `cargo +1.95 check --all-targets --locked` | Rust 1.95 all-targets compilation |
| `Rust Dependency Review in CI - Static Checks` | cargo-machete, cargo-deny advisories and license inventory |
| `Mutation Fuzz and Native Sanitizer in CI - Tests` | Effect-set propagation mutants with caught, missed and unviable outcomes |
| `Mutation Fuzz and Native Sanitizer in CI - Tests` | Seeded UTF-8 parser/formatter checks with AddressSanitizer for 60 seconds |
| `Mutation Fuzz and Native Sanitizer in CI - Tests` | Native scalar ABI, arithmetic errors and object emission under host AddressSanitizer |
| `just benchmark` | Local wall-clock measurements of lexing, parsing and formatting 128 functions |
| `Structure History and External Links in CI - Static Checks` | jscpd and Lizard inventories without new score thresholds |
| `Structure History and External Links in CI - Static Checks` | External HTTPS link results |
| `just build` / `uvx maturin==1.12.6 sdist --out dist` | Wheel and source distributions; CI installs and invokes both |
| `uvx prek==0.5.7 run gitleaks-history --hook-stage manual --all-files` | Full Git history scan after creating `reports/` |

## Tool ownership

Tombi, typos, rumdl, Vale, check-jsonschema and local Lychee cover repository files.
The parser IO ast-grep rule follows the parser's text-to-AST boundary. The Semgrep
rule keeps shell command text out of the effect handler's structured process argument
path. Both rules have accepted and rejected fixtures. Gitleaks checks the working tree
and runs a separate history scan.

jactionlint 2.3.0 runs offline locally with ShellCheck and Pyflakes. Its strict online
workflow runs only on trusted `main`, records completed lookups and fails on unavailable
queries. zizmor remains enabled because the negated bot-gate control exposes a detection
gap in jactionlint. CI references, permissions and timeout budgets are explicit.

There is no Python implementation, frontend application, Dockerfile or standalone shell
script in this checkout. Python packaging uses maturin's binary bindings. Adding Python,
frontend or container checks with no corresponding source would not add coverage.
Rust libraries are internal path dependencies in the CLI distribution; no public crate
publication or stable library API contract is declared, so registry semver comparisons
would not represent this delivery boundary.

## Behavior and scope

Property tests cover arbitrary UTF-8, literal formatting and effect algebra, including
cyclic implication graphs. Fuzzing adds arbitrary accepted-source formatting and reparse
checks. Minimized failures become ordinary regression tests. The generated corpus and
crash artifacts are retained for inspection; a bounded run does not prove absence of
parser defects or cover unlimited source nesting.

The runtime currently simulates structured tasks without a custom thread/atomic protocol,
so there is no Loom model. Miri cannot execute the native backend's generated machine
code. AddressSanitizer exercises the native host boundary; it does not instrument JIT
instructions or establish cross-platform native correctness. Those remain separate
validation limits. Compiler checks here do not establish Spore Lab or Spec semantics.

Only the native object writer is enabled in `object`: this project does not read or
decompress object files. Removing its unused default reader/compression features removes
the vulnerable `ruzstd` path. `anyhow` is locked to its patched version. cargo-deny and
OSV retain their full inventories; no advisory suppression is added.

## CI and services

Deterministic checks run on pull requests. Mutation, fuzz, sanitizer, structure, links
and history have weekly and manual runs. Parser/formatter benchmarks also run on PRs.
Their CodSpeed build and local smoke execution are distinct from hosted measurement.

Coverage is saved as an artifact before Codecov and Sonar uploads. Codecov uses GitHub
OIDC and fails on upload errors. Sonar's Rust configuration imports LCOV and requires
project access plus `SONAR_TOKEN`. CodeQL analyzes Rust and Actions with Rust's supported
`none` build mode. Check the current commit's service result before claiming integration;
a configuration file or successful upload is not a completed analysis.
