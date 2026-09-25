A focused Neovim frontend built with GPUI. This is an early development release.

## New in 0.0.4

- Independent terminal tabs with a compact tab strip, new-tab and close buttons. Switching or hiding tabs preserves each shell, running program and scrollback.
- Zed-style terminal defaults: Cmd+Shift+, toggles focus between terminal and editor; Cmd+Shift+. shows/hides the terminal; Cmd+Shift+Enter maximises/restores it.
- With the terminal focused, Cmd+N creates a tab, Cmd+W closes it, Cmd+1–9 selects a tab, and Cmd+Shift+[ / ] switches tabs. Cmd+Alt+Left/Right also switches tabs.
- Configure all seven terminal actions in Settings → Keybindings. Previous default backtick shortcuts migrate to the new defaults; custom shortcuts are preserved.
- Window close and Quit check every terminal tab, including hidden tabs, before stopping running commands. New tabs use Neovim's current window-local directory.
- Settings now has General and Keybindings tabs, smaller buttons and tighter spacing. Boolean settings use green switches when enabled and grey switches when disabled.

## Install

Requires an Apple Silicon Mac running macOS 13 or newer. Download the macos-arm64 archive, move Zvim.app to Applications, then launch it. Use Settings → General → Command-line launcher to install the optional CLI. Keep the chosen bin directory on your shell's PATH.

Bundled Neovim 0.12.5 loads your existing Neovim configuration. No separate Ghostty installation is required. SHA256SUMS.txt contains archive checksums.

## Known limitations

The terminal is experimental: one pane per window with multiple tabs, but no terminal splits, search UI, persisted sessions or IME composition support. Split proportions are retained only while the window remains open. Linux requires Wayland for the terminal and remains experimental; this release ships a macOS Apple Silicon archive only.

macOS applications are ad-hoc signed, not Developer ID signed or notarised. Managed work laptops may require IT approval.
