default:
    @just --list

bundle:
    python3 scripts/bundle-neovim.py

run *args:
    cargo build --locked
    python3 scripts/bundle-ghostty.py
    cargo run --locked -- {{args}}

# Optimised build for normal use and responsiveness comparisons.
run-release *args:
    cargo build --release --locked
    python3 scripts/bundle-ghostty.py
    cargo run --release --locked -- {{args}}

check:
    cargo fmt --check
    cargo test --locked --all-targets
    cargo clippy --locked --all-targets -- -D warnings

package:
    cargo build --release --locked
    python3 scripts/package.py

# macOS: quit Zvim first, then build and replace /Applications/Zvim.app.
[macos]
install-dev:
    #!/usr/bin/env bash
    set -euo pipefail
    if pgrep -x zvim >/dev/null; then
        echo "Quit Zvim before running just install-dev so the new build can be used." >&2
        exit 1
    fi
    just package
    packaged_app="dist/zvim-macos-$(uname -m)/Zvim.app"
    test -d "$packaged_app"
    codesign --verify --deep --strict "$packaged_app"
    rm -rf /Applications/Zvim.app
    ditto "$packaged_app" /Applications/Zvim.app
    xattr -dr com.apple.quarantine /Applications/Zvim.app
    echo "Installed development build. Launch it with: open /Applications/Zvim.app"

install-cli:
    python3 scripts/install-cli.py

# macOS: opens a temporary native window and checks live terminal theme updates.
check-terminal-theme:
    cargo build --locked
    python3 scripts/bundle-ghostty.py
    python3 scripts/test-terminal-theme.py
