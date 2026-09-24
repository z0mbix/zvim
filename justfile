default:
    @just --list

bundle:
    python3 scripts/bundle-neovim.py

run *args:
    cargo build --locked
    python3 scripts/bundle-ghostty.py
    cargo run --locked -- {{args}}

check:
    cargo fmt --check
    cargo test --locked --all-targets
    cargo clippy --locked --all-targets -- -D warnings

package:
    cargo build --release --locked
    python3 scripts/package.py

install-cli:
    python3 scripts/install-cli.py

# macOS: opens a temporary native window and checks live terminal theme updates.
check-terminal-theme:
    cargo build --locked
    python3 scripts/bundle-ghostty.py
    python3 scripts/test-terminal-theme.py
