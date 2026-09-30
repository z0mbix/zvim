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

# Test stock Neovim in a window with disposable config and state.
[positional-arguments]
run-clean *args:
    #!/usr/bin/env bash
    set -euo pipefail
    python3 scripts/bundle-neovim.py
    cargo build --release --locked
    python3 scripts/bundle-ghostty.py
    clean_root="$(mktemp -d "${TMPDIR:-/tmp}/zvim-clean.XXXXXX")"
    trap 'rm -rf -- "$clean_root"' EXIT
    mkdir -p "$clean_root"/{config,data,cache,state}
    echo "Opening stock Neovim. Close the test window to finish and remove its temporary state."
    env -u NVIM_APPNAME -u VIMINIT -u EXINIT \
        XDG_CONFIG_HOME="$clean_root/config" \
        XDG_DATA_HOME="$clean_root/data" \
        XDG_CACHE_HOME="$clean_root/cache" \
        XDG_STATE_HOME="$clean_root/state" \
        target/release/zvim --wait --clean "$@"

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
