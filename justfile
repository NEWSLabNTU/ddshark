# List available recipes
default:
    @just --list

# Build the release binary
build:
    cargo build --release

# Run the test suite
test:
    cargo test

# Format the source code
format:
    cargo +nightly fmt

# Lint and verify formatting without modifying files
check:
    cargo +nightly fmt --check
    cargo clippy --all-targets -- -D warnings

# Full CI pipeline: check, build, test
ci: check build test
