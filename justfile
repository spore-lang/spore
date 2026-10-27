# List all available commands
default:
    @just --list

# Install
install:
    uvx prek==0.5.7 install --prepare-hooks
    uvx maturin==1.12.6 develop

# Build a platform wheel for the packaged CLI into dist/
package-cli:
    uvx maturin==1.12.6 build --release --locked --out dist

# Build a source distribution for the packaged CLI into dist/
package-cli-sdist:
    uvx maturin==1.12.6 sdist --out dist

# Format all code
format:
    just --fmt --unstable
    cargo fmt

# Run all static checks (fmt check + clippy)
check:
    cargo fmt --all -- --check
    cargo clippy --all-targets --locked -- -D warnings

# Run tests with coverage (requires cargo-llvm-cov)
cov:
    cargo llvm-cov --workspace --locked --lcov --output-path lcov.info
    cargo test --workspace --locked --doc

# Open coverage HTML report
cov-open:
    cargo llvm-cov test --html -- --no-capture
    open target/llvm-cov/html/index.html || xdg-open target/llvm-cov/html/index.html

# MSRV check
msrv:
    cargo +1.95 check --all-targets --locked

# Clean build artifacts
clean:
    cargo clean
    rm -f lcov.info

# Run pre-commit on all files
pre-commit:
    uvx prek==0.5.7 run --all-files

# Display project information
info:
    @echo "=== Spore ==="
    @echo "Rust: $(rustc --version)"
    @echo "Cargo: $(cargo --version)"
    @echo ""
    @echo "Workspace members:"
    @cargo metadata --no-deps --format-version 1 2>/dev/null | jq -r '.packages[].name' 2>/dev/null || echo "  (install jq for detailed info)"

# Install hooks without building or installing the CLI
pre-commit-install:
    uvx prek==0.5.7 install --prepare-hooks

# Rust dependency hygiene; license inventory does not introduce an admission policy
quality:
    cargo machete
    cargo deny check advisories
    mkdir -p reports
    cargo deny list --format json > reports/rust-licenses.json

# Focused mutation of effect propagation; preserve full mutant classifications
mutation:
    cargo mutants --file crates/sporec-typeck/src/effect_set.rs --timeout 90 --output reports/mutation -- --locked

# Seed the source parser and formatter with shipped examples; ASan is enabled by cargo-fuzz
fuzz:
    mkdir -p fuzz/corpus/source
    cp examples/*.sp fuzz/corpus/source/
    cargo +nightly-2026-10-26 fuzz run source -- -max_total_time=60 -max_len=4096 -timeout=5

# Native scalar ABI and object emission with host-side address sanitization
sanitizer:
    RUSTFLAGS="-Zsanitizer=address" CARGO_TARGET_DIR=target/sanitizer cargo +nightly-2026-10-26 test -p sporec-codegen --test native_scalar_tests --target {{ arch() }}-{{ if os() == "macos" { "apple-darwin" } else { "unknown-linux-gnu" } }} --locked

# Local timings are descriptive; the CodSpeed workflow measures the same workloads
benchmark:
    cargo bench -p sporec-parser --bench source --locked

# Non-blocking structure inventories; no new score threshold
reports:
    mkdir -p reports
    npx --yes jscpd@5.4.2
    uvx lizard==1.24.3 -X --ignore_warnings -1 crates > reports/complexity.xml

# External requests remain distinct from deterministic local link checks
links:
    mkdir -p reports
    git ls-files -z '*.md' | xargs -0 uvx --from lychee-bin==0.24.2 lychee --no-progress --scheme https --format json --output reports/links.json
