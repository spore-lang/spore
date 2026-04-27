# List all available commands
default:
    @just --list

# Install
install:
    uvx prek install --install-hooks --hook-type pre-commit --hook-type commit-msg
    uvx maturin develop

# Build a platform wheel for the packaged CLI into dist/
package-cli:
    uvx maturin build --release --locked --out dist

# Build a source distribution for the packaged CLI into dist/
package-cli-sdist:
    uvx maturin sdist --out dist

# Format all code
format:
    just --fmt --unstable
    cargo fmt

# Run all static checks (fmt check + clippy)
check:
    cargo fmt --all -- --check
    cargo clippy --all-targets -- -D warnings

# Run tests with coverage (requires cargo-llvm-cov)
cov:
    cargo llvm-cov test --lcov --output-path lcov.info -- --no-capture

# Open coverage HTML report
cov-open:
    cargo llvm-cov test --html -- --no-capture
    open target/llvm-cov/html/index.html || xdg-open target/llvm-cov/html/index.html

# MSRV check
msrv:
    cargo +1.95 check --all-targets

# Clean build artifacts
clean:
    cargo clean
    rm -f lcov.info

# Run pre-commit on all files
pre-commit:
    uvx prek run --all-files

# Display project information
info:
    @echo "=== Spore ==="
    @echo "Rust: $(rustc --version)"
    @echo "Cargo: $(cargo --version)"
    @echo ""
    @echo "Workspace members:"
    @cargo metadata --no-deps --format-version 1 2>/dev/null | jq -r '.packages[].name' 2>/dev/null || echo "  (install jq for detailed info)"
