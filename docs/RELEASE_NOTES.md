A focused Neovim frontend built with GPUI. This is an early development release.

## New in 0.0.2

- Native Ghostty terminal pane: toggle with Ctrl+backtick, the Terminal menu or `:ZvimTerminal`.
- Terminal colours follow Neovim live, including NvChad/Base46. Settings can retain your Ghostty theme instead.
- Install or update the `zvim` command from Settings, choosing your bin directory. No Python is required.
- Improved window-close and Quit behaviour: confirm terminal shutdown before closing the editor, preserve the session on Cancel, and skip warnings for idle prompts detected by shell integration.
- Retain terminal content behind close dialogs. After quitting Neovim directly, Keep Terminal expands the remaining session to fill the window.
- macOS and Linux project scope; Windows support removed.

## Install

Requires macOS 13 or newer. Apple Silicon Macs use `macos-arm64`; Intel Macs use `macos-x86_64`. Move `Zvim.app` to Applications, then launch it. Use Settings → Command-line launcher to install the optional CLI. Keep the chosen bin directory on your shell's PATH.

Bundled Neovim 0.12.5 loads your existing Neovim configuration. No separate Ghostty installation is required. `SHA256SUMS.txt` contains archive checksums.

## Known limitations

The terminal is experimental: one pane per window, with no tabs, search UI, persisted sessions or IME composition support. Linux requires Wayland for the terminal and remains experimental; this release ships macOS archives only. Apple Silicon has been exercised locally; Intel runtime and Linux desktop behaviour need further interactive testing.

macOS applications are ad-hoc signed, not Developer ID signed or notarised. Managed work laptops may require IT approval.
