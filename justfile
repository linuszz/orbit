# Orbit — build tasks. See 06_tech-design/01-tech-stack-and-workspace.md §3.
# Requires `just` (https://github.com/casey/just). All commands also available
# directly via cargo.

# Default: show available recipes.
default:
    @just --list

# Run the daemon (orbitd) in dev mode.
daemon:
    cargo run -p orbitd

# Attach the TUI client to the default constellation.
dev:
    cargo run -p orbit

# Build all workspace members (debug).
build:
    cargo build --workspace

# Build all workspace members (release).
release:
    cargo build --workspace --release

# Type-check without producing binary (fast).
check:
    cargo check --workspace --all-targets

# Run all unit + integration tests.
test:
    cargo test --workspace

# Run only the cross-crate integration test suite.
test-integration:
    cargo test --test integration

# Lint: deny any clippy warnings workspace-wide.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Format the entire workspace in-place.
fmt:
    cargo fmt --all

# Verify formatting without writing.
fmt-check:
    cargo fmt --all --check

# Quality gate: format check + lint + test.
qa: fmt-check lint test

# Remove build artifacts.
clean:
    cargo clean
