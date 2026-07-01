# List available recipes
default:
    @just --list

# Build the release binary
build:
    cargo build --release

# Run the fast test tiers (L1-L3) with nextest; no elevated privileges needed
test:
    cargo nextest run

# Run the live E2E tier too (needs CAP_NET_RAW / a netns runner)
test-e2e:
    cargo nextest run --profile e2e

# Format the source code
format:
    cargo +nightly fmt

# Lint and verify formatting without modifying files
check:
    cargo +nightly fmt --check
    cargo clippy --all-targets -- -D warnings

# Full CI pipeline: check, build, test
ci: check build test
