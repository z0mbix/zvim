A focused Neovim frontend built with GPUI. This is an early development release.

## New in 0.0.12

- Added optional per-project terminal workspaces under Settings → General → Remember terminal layouts. Switching Neovim projects in the same window preserves each project's live shells, panes and tabs. Reopening a project after quitting starts fresh shells in its saved layout and directories.
- Added a generic Neovim project bridge, `require("zvim").set_project(root)`, for project/session plugins. Global working-directory changes are detected automatically; window-local and tab-local directory changes do not switch terminal workspaces.
- Stable macOS releases now publish a Homebrew cask automatically to `z0mbix/tap`.

## Fixed in 0.0.12

- Development builds now select Zig 0.16.0 and Rust 1.98.1 through `.config/mise.toml`. The `just` recipes use mise automatically, preventing newer Homebrew compilers from breaking the pinned Ghostty build. Run `mise trust .config/mise.toml` and `mise install` once per checkout.

## Install

Requires an Apple Silicon Mac running macOS 13 or newer. Install with `brew install --cask z0mbix/tap/zvim`, or quit all existing Zvim instances, download the macos-arm64 archive, and replace `/Applications/Zvim.app`. Reopen Zvim to use the new application host. Use Settings → General → Command-line launcher to install the optional CLI. Keep the chosen bin directory on your shell's PATH.

Bundled Neovim 0.12.5 loads your existing Neovim configuration. No separate Ghostty installation is required. SHA256SUMS.txt contains archive checksums.

## Known limitations

The terminal remains experimental. Shell input IME composition is not implemented; the search field uses native text-input composition. Remember terminal layouts restores pane geometry and shell directories, but does not restore running programs or scrollback after quitting. Neovim plugins continue to own editor sessions. Linux requires Wayland for the terminal and remains experimental; this release ships a macOS Apple Silicon archive only. Native macOS search and window-menu behaviour have automated smoke coverage; Linux desktop runtime and full manual IME validation remain outstanding.

macOS applications are ad-hoc signed, not Developer ID signed or notarised. For local installation, remove quarantine with `xattr -dr com.apple.quarantine /Applications/Zvim.app` if needed. Managed work laptops may require IT approval.
