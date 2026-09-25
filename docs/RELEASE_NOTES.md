A focused Neovim frontend built with GPUI. This is an early development release.

## New in 0.0.5

- Terminal splits with independent tabs and draggable dividers. Cmd+D splits right, Cmd+Shift+D splits below, and Cmd+[ / ] focuses the left/right pane, matching the Zed defaults. These shortcuts are configurable in Settings → Keybindings.
- Cmd+Shift+Enter zooms the focused terminal pane and restores the split layout. The Terminal menu also offers directional pane focus.
- Exiting a shell automatically closes its tab. Empty splits collapse and remaining panes expand; the final terminal returns the space to the editor. Background exits preserve focus and do not reopen a hidden dock.
- About Zvim in the macOS app menu displays the installed version.
- Window size and position save shortly after moving/resizing, as well as on normal close. Corrected macOS title-bar sizing during restoration; fullscreen preserves the previous windowed placement.
- Less rendering work: cached terminal themes, no hidden terminal control construction, allocation-free warmed text-cache lookups, and bounded cache eviction. Three paired release benchmarks measured 40–56% less editor CPU paint time across the tested workloads. This is not an equivalent claim about total application speed or input latency; see docs/PERFORMANCE.md for methodology and results.
- Window geometry writes now run in an ordered background worker with atomic file replacement. Added `just run-release` and reproducible performance benchmarks for development.

## Install

Requires an Apple Silicon Mac running macOS 13 or newer. Download the macos-arm64 archive, move Zvim.app to Applications, then launch it. Use Settings → General → Command-line launcher to install the optional CLI. Keep the chosen bin directory on your shell's PATH.

Bundled Neovim 0.12.5 loads your existing Neovim configuration. No separate Ghostty installation is required. SHA256SUMS.txt contains archive checksums.

## Known limitations

The terminal is experimental: no search UI, persisted sessions or IME composition support. Split proportions are retained only while the window remains open. Linux requires Wayland for the terminal and remains experimental; this release ships a macOS Apple Silicon archive only.

macOS applications are ad-hoc signed, not Developer ID signed or notarised. Managed work laptops may require IT approval.
