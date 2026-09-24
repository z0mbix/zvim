default:
    @just --list

bundle:
    python3 scripts/bundle-neovim.py

run *args:
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
