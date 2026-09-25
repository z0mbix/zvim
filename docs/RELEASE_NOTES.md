A focused Neovim frontend built with GPUI. This is an early development release.

## New in 0.0.3

- Resize the terminal by dragging a thin, one-pixel divider. The previous header bar has been removed.
- Maximise the terminal to fill the window content area, then restore the previous split with Control+Shift+backtick or the Terminal menu.
- Configure show/hide and maximise/restore shortcuts in Settings → Terminal keybindings. Record a shortcut, cancel with Escape, or reset to defaults. Changes save automatically and apply immediately.
- Preserve the split proportion while hiding, showing, maximising and restoring the terminal in the same window.
- macOS builds and release archives now target Apple Silicon only. Intel Mac builds have been removed from GitHub Actions.

## Install

Requires an Apple Silicon Mac running macOS 13 or newer. Download the macos-arm64 archive, move Zvim.app to Applications, then launch it. Use Settings → Command-line launcher to install the optional CLI. Keep the chosen bin directory on your shell's PATH.

Bundled Neovim 0.12.5 loads your existing Neovim configuration. No separate Ghostty installation is required. SHA256SUMS.txt contains archive checksums.

## Known limitations

The terminal is experimental: one pane per window, with no tabs, search UI, persisted sessions or IME composition support. Split proportions are retained only while the window remains open. Linux requires Wayland for the terminal and remains experimental; this release ships a macOS Apple Silicon archive only.

The Rust/Neovim suite covers split limits, shortcut validation and preference persistence. Live dragging and shortcut recording have not yet been manually verified for this release.

macOS applications are ad-hoc signed, not Developer ID signed or notarised. Managed work laptops may require IT approval.
