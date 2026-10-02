A focused Neovim frontend built with GPUI. This is an early development release.

## New in 0.0.9

- Terminal numbering now advances only when a shell is created. Split panes and dividers no longer cause skipped numbers; names remain stable when terminals close.
- Cmd+[ and Cmd+] now cycle backwards and forwards through all terminal tabs across every split pane, in terminal-number order, wrapping at either end. Inactive tabs and vertically stacked panes are reachable. Cmd+Shift+[ / ] still cycles within the focused pane, and Cmd+1–9 still selects a tab within that pane.
- Settings and Terminal menu labels now describe cycling across all panes.

## Install

Requires an Apple Silicon Mac running macOS 13 or newer. Quit all existing Zvim instances, download the macos-arm64 archive, and replace `/Applications/Zvim.app`. Reopen Zvim to use the new application host. Use Settings → General → Command-line launcher to install the optional CLI. Keep the chosen bin directory on your shell's PATH.

Bundled Neovim 0.12.5 loads your existing Neovim configuration. No separate Ghostty installation is required. SHA256SUMS.txt contains archive checksums.

## Known limitations

The terminal remains experimental. Shell input IME composition and persisted sessions are not implemented; the search field uses native text-input composition. Split proportions are retained only while the window remains open. Linux requires Wayland for the terminal and remains experimental; this release ships a macOS Apple Silicon archive only. Native macOS search and window-menu behaviour have automated smoke coverage; Linux desktop runtime and full manual IME validation remain outstanding.

macOS applications are ad-hoc signed, not Developer ID signed or notarised. For local installation, remove quarantine with `xattr -dr com.apple.quarantine /Applications/Zvim.app` if needed. Managed work laptops may require IT approval.
