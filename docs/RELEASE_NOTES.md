A focused Neovim frontend built with GPUI. This is an early development release.

## New in 0.0.6

- One application owns multiple windows, with one Dock icon. CLI launches join the running application while retaining independent Neovim processes, project directories and arguments. `--wait` waits for its own window to close.
- Optional **Focus follows mouse** in Settings → General switches focus between editor and terminal panes without clicking. It is off by default.
- Terminal scrollback search with **Cmd+F**, match counts, previous/next controls, **Enter / Shift+Enter**, and **Escape** to return to the terminal. **Cmd+G / Cmd+Shift+G** also navigate matches.
- A native macOS **Window** menu lists open windows and marks the current one. **Cmd+` / Cmd+Shift+`** cycle windows; **Cmd+M** minimises. The menu also offers **Bring All to Front**.
- Resize requests retain only the latest pending dimensions while Neovim is busy. In a benchmark with a simulated 25 ms plugin resize callback, 100 resize steps settled in 283–312 ms instead of 2.64–2.66 seconds. Lightweight callbacks retained the same throughput. These are RPC settling measurements, not GUI frame latency.
- `just install-dev` builds and installs a development package into `/Applications` and removes its quarantine attribute after Zvim has been quit.

## Install

Requires an Apple Silicon Mac running macOS 13 or newer. Quit all existing Zvim instances, download the macos-arm64 archive, and replace `/Applications/Zvim.app`. Reopen Zvim to use the new application host. Use Settings → General → Command-line launcher to install the optional CLI. Keep the chosen bin directory on your shell's PATH.

Bundled Neovim 0.12.5 loads your existing Neovim configuration. No separate Ghostty installation is required. SHA256SUMS.txt contains archive checksums.

## Known limitations

The terminal remains experimental. Shell input IME composition and persisted sessions are not implemented; the search field uses native text-input composition. Split proportions are retained only while the window remains open. Linux requires Wayland for the terminal and remains experimental; this release ships a macOS Apple Silicon archive only. Native macOS search and window-menu behaviour have automated smoke coverage; Linux desktop runtime and full manual IME validation remain outstanding.

macOS applications are ad-hoc signed, not Developer ID signed or notarised. For local installation, remove quarantine with `xattr -dr com.apple.quarantine /Applications/Zvim.app` if needed. Managed work laptops may require IT approval.
