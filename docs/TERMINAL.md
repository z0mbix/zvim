# Native Ghostty trial

Zvim targets macOS and Linux. The terminal uses Ghostty's native Metal renderer on macOS and a Wayland/OpenGL surface on Linux; X11 remains editor-only. This is one optional, lazily created bottom pane per editor window. Ctrl+backtick by default, the Terminal menu and `:ZvimTerminal` toggle it. Hiding keeps the shell running. The initial cwd comes from Neovim's current window, including `:lcd`; an existing shell keeps its own cwd. The pane starts the user's `$SHELL` as a login shell, falling back to `/bin/sh`.

## Build and packaging

The pinned native renderer requires macOS 13 or newer. Install Zig 0.16.0 in addition to the existing Rust toolchain. Build with `cargo build --locked`, then run `python3 scripts/bundle-ghostty.py`. `just run` stages resources automatically. `scripts/package.py` stages and ships themes, shell integration, the xterm-256color terminfo entry used by the native shim, and native dependency notices. The first native compilation downloads content-pinned Zig packages. Native build caches follow gpui-libghostty's `GHOSTTY_*_CACHE_DIR` settings; preserve the same settings when packaging. No installed Ghostty application is needed.

`gpui-libghostty` is pinned to bfa3771f0ef2290e54acf9f1fc11e05d42a882ee, including its patched Ghostty revision 9f0e1719dc918368367d368bfe300f59bb68b5a4. Zvim uses a local MIT-derived adapter for GPUI 0.2.2 and the small `crates/zvim-ghostty` bridge for live colour updates; the upstream dependency still builds the pinned native engine. The library's internal native wrapper is deliberately revision-pinned; upgrades require rebuilding and repeating interaction QA.

On Ubuntu 24.04, `scripts/linux-dependencies.sh` installs the development libraries, including libc++ 21 from LLVM's APT repository. This script changes system package sources and is intended for CI or explicit developer setup. Linux requires a Wayland session and OpenGL 4.3. Current release archives remain macOS-only; Linux CI produces an experimental archive.

## Behaviour and limits

- Neovim's editor renderer, `:terminal` and terminal plugins are unchanged. The native pane is not a Neovim buffer and cannot be navigated with Neovim window commands.
- The terminal follows Neovim's colours by default. Settings → Terminal theme switches between **Follow Neovim** and **Use Ghostty theme**, immediately, without restarting the shell. This preference is separate from Sync editor appearance, which controls Neovim's light/dark setting.
- Foreground, background, cursor, selection and the 16 ANSI colours follow Neovim where defined. Missing ANSI and selection entries use the terminal's original Ghostty configuration. Neovim schemes that leave stale terminal globals themselves retain those values; Base46 uses its active `base_16` palette because its reload does not always refresh those globals.
- Standard `ColorScheme`, `background` changes, NvChad's `NvThemeReload` and redraw highlight changes trigger a debounced palette read. Identical palettes are ignored. There is no periodic polling and no modification of user configuration files. Direct changes to terminal globals alone need `:doautocmd ColorScheme` to publish them.
- User Ghostty fonts, shell settings and keybindings load at shell creation. Changing those files requires closing and recreating the terminal. Switching back to Use Ghostty theme restores that terminal's original configuration. Bundled resources take precedence over an inherited `GHOSTTY_RESOURCES_DIR`.
- One session per window, initially 40% height. Drag the one-pixel divider to resize; its transparent grab area is wider than the line. Both panes retain at least 80 logical pixels where window size permits. The split proportion survives hide/show and maximise/restore within the window, but is not saved across app restarts. No terminal tabs, split manager, search UI or persisted terminal sessions yet. Ghostty application actions such as new windows, tabs and settings are not wired to Zvim.
- The adapter handles key presses/releases, repeats, modifier keys, mouse input, selection, scrolling and clipboard. It does not implement a terminal IME composition interface; dead keys and CJK composition need further work. GPUI's abstract keys also lose some physical-key/keypad distinctions.
- Ghostty's protected clipboard requests are denied when the embedder cannot obtain approval. Ordinary copy/paste follows Ghostty configuration; unsafe/multiline pastes can be refused. No clipboard approval UI is provided yet.
- Native surfaces paint above GPUI content. Zvim hides the terminal before close prompts; future overlapping in-window UI needs explicit native-surface handling.
- Window close and application Quit check Ghostty before asking Neovim to close. Idle prompts with working shell integration close quietly; running commands or unknown shell state prompt first. Ghostty's `confirm-close-surface` preference is respected. Cancel keeps Neovim and the terminal alive, preserving focus and pane visibility. Approving the terminal warning still allows cancellation of Neovim's unsaved-buffer prompt; the terminal is only destroyed once Neovim actually exits.
- The close dialog composites a snapshot of the terminal while hiding its native surface, keeping terminal content visible behind the modal. If snapshot capture fails, the pane remains blank only while the dialog is open.
- Direct Neovim exit commands (`:q`, `:qa`, etc.) close the editor independently. If the remaining terminal needs confirmation, **Keep Terminal** retains it in a full-window terminal layout with a small status banner. Neovim's exit hooks do not reliably veto these commands, so Zvim does not install an exit interception autocmd or rewrite user commands. Terminal → Close Terminal closes that remaining window.
- A shell that exits on its own can be recreated by hiding/showing the pane while the editor is open.
- No performance or feature-parity claim against standalone Ghostty is established by this trial. Linux, Intel macOS, mixed-DPI displays and comprehensive interactive CLI compatibility need runtime QA.

## Validation

The real-Neovim test checks `:ZvimTerminal` notifications with a quoted, window-local cwd and verifies that built-in terminal buffers still work. The theme integration test covers direct highlight updates, colour-scheme events, reverse selection colours, palette removal and Base46 reloads. After `cargo build --locked` and staging resources, `python3 scripts/test-terminal-theme.py` opens a temporary macOS test window and verifies rendered background changes and restoration while retaining the live surface, output and input. macOS runtime checks and final build results are recorded in `docs/VALIDATION.md`.

## Pane shortcuts

Settings → Terminal keybindings records replacements for show/hide (Control+backtick) and maximise/restore (Control+Shift+backtick). The maximised terminal fills the window content area without restarting the shell; restoring returns to the chosen split proportion. The Terminal menu exposes both actions even if a custom shortcut is unavailable.

Changes save in preferences.json and update the app keymap immediately. Invalid hand-edited combinations fall back to the defaults. Shortcut recording requires Control, Alt/Option or Command/Super, rejects conflicts with other pane shortcuts and reserved app keys, and supports Escape to cancel and Reset shortcuts. Other Neovim/Ghostty shortcuts are still controlled by their respective configurations.
