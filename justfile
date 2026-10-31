# List developer commands.
default:
    @just --list

# Install the CLI and Git hooks.
install:
    uvx --isolated prek install --prepare-hooks
    uvx maturin==1.12.6 develop

# Build the wheel and source distribution.
build:
    uvx maturin==1.12.6 build --release --locked --out dist
    uvx maturin==1.12.6 sdist --out dist

# Format Justfile and Rust sources.
format:
    just --fmt --unstable
    cargo fmt

# Run the read-only repository gate.
check:
    uvx --isolated prek run --all-files

# Run the compiler and documentation tests.
test:
    cargo test --workspace --locked --all-features

# Collect LLVM coverage and run doctests.
coverage:
    cargo llvm-cov --workspace --locked --lcov --output-path lcov.info
    cargo test --workspace --locked --doc --all-features

# Measure the parser and formatter locally.
benchmark:
    cargo bench -p sporec-parser --bench source --locked

# Remove local build artifacts.
clean:
    cargo clean
    rm -f lcov.info
