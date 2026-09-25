# Validation status

Zvim targets macOS and Linux only. Linux runtime behaviour still requires validation.

## Verified locally (2026-09-23, macOS arm64)

- Rust build using GPUI 0.2.2 and bundled Neovim 0.12.5.
- SHA-256 verification, complete Neovim runtime extraction, and local app packaging/ad-hoc signing.
- Unit tests: flush snapshots, highlight reuse, region scrolling, Unicode continuation cells, repeated cells, malformed update rejection, and key encoding.
- Real-editor integration: file names with spaces/quotes, edit/save, splits, floating windows, grid resize, opening another file, paste, clipboard provider round trips, and cancel/discard on quit.
- Installed launcher smoke test: detached launch returned immediately; the actual GUI session reported the requested project directory, relative files, vertical splits, and an attached UI. `--wait` stayed attached until its test window closed, then exited successfully. Help/version output, incompatible-mode errors, and rebuilt app signature checked.
- CLI integration: vertical/horizontal splits, tabs, readonly, diff mode, project working directory before configuration, custom init, startup-command ordering, and line selection.
- Packaged app launched through macOS Launch Services.
- Twenty automated tests passed; `cargo fmt --check` and strict Clippy passed.
- Visual interaction: typing, line numbers, commands, saving a scratch file, split windows, emoji/CJK/combining-character paste, Option-E accented input, live `guifont` changes, unsaved-change close cancellation, native file dialogs, mouse scrolling, and cursor positioning.
- Release font fallback visually verified, including Nerd Font icons.
- Multi-window Command-Q verified on the rebuilt release: both sessions exit. The callback defers window updates until the active GPUI event finishes.
- Release executable is approximately 5.2 MB; the compressed macOS archive with Neovim is approximately 16 MB (49 MB extracted app). Ad-hoc signature verification passed.
- Normal NvChad configuration loaded its theme, tabs, statusline, file picker, Rust highlighting, and minimap. No user configuration files were changed.

## Settings validation (2026-09-24, macOS arm64)

- Settings opens with Command-comma, persists choices across reopening, and closes independently of editor windows.
- Light/System controls, optional editor background sync and restoration, and geometry toggle exercised in an isolated copy of the release app. Native titlebars follow the OS.
- Tab navigation and Space activation verified after correcting explicit GPUI focus-handle tab stops. Final layout shows all controls and footer.
- Quit closes both settings and editor windows. Preferences restored after testing; rebuilt app signature verified.
- Unit coverage checks missing-field defaults, persistence independent of window geometry, and malformed preferences. Real-editor coverage checks light/dark switching, restoration, and preserving a manual background change.
- Live OS appearance changes, independently launched instance activation reloads, and Linux settings UI still require runtime QA.

## Base46 appearance regression (2026-09-24)

- Reproduced with the user's actual NvChad configuration: changing `background` replaced the custom Rose Pine Moon Normal highlights with Neovim defaults, while `nvconfig.base46.theme` still reported `rose-pine-moon`.
- The sync bridge now preserves Base46 palettes instead of changing `background`. Regression coverage includes preserving the custom palette and repairing state from the former implementation.
- `cargo run --offline --locked --no-default-features --example diagnose -- --config --appearance` verified the actual configuration retains `background=dark`, `theme=rose-pine-moon`, and Normal foreground/background through light, dark, and disabled transitions. No Neovim configuration files were edited.

## Still required before v1

- Broader interactive testing on Intel macOS. Both macOS architectures have passed CI tests, Clippy, release compilation and packaging.
- Linux remains outside the current release artifacts; Linux CI builds require validation with the new Ghostty dependency.
- Runtime tests on Linux under Wayland; editor-only runtime checks on X11.
- Comprehensive IME/CJK input and candidate placement, less common font fallbacks, mixed-DPI monitor transitions, drag/drop, and accessibility review.
- Resolve an intermittent `CmdlineChanged` Lua callback error observed in the existing configuration. It did not reproduce in a separate embedded-editor diagnostic session; its cause is not yet established. Broader plugin workflow compatibility is not certified.
- Public signing/notarisation, broader Linux distribution compatibility.

## Performance

The frontend records time from editor-view construction to first grid paint in `startup.log`; this excludes OS launch and pre-window GPU setup. It is not an end-to-end launch benchmark. Measurements should report frontend and Neovim together and use identical configuration/files for comparisons.

Initial debug-package idle observation before further polishing: 65,520 KiB frontend RSS + 8,656 KiB Neovim RSS (about 72.4 MiB combined), with both reporting 0.0% CPU in a point sample. This is not a release-build benchmark, long-duration CPU measurement, or comparison with Zed. Full scrolling latency and mixed-DPI profiling remain outstanding.

Warm configured-session observation: 187 ms from editor-view construction to first grid paint. This excludes initial OS/window/GPU startup and is a single sample, not a launch-time guarantee.

## Startup picker performance (2026-09-24)

See [the startup investigation](STARTUP.md) for measured stages, raw samples, and limitations. Added project-local generated-file exclusions and replaced whole-system font enumeration with candidate lookup. All 20 automated tests and strict Clippy passed; release GUI probes verified the configured Nerd Font and missing-first-font fallback. Native window/GPU initialization and the initial grid resize remain measurable overheads.

## Picker interaction rendering fix (2026-09-24)

The earlier startup-only changes did not establish a fix for typing/navigation lag. A subsequent native interaction test with ignore rules bypassed reproduced ~179 ms median CPU paints. Process sampling identified per-cell GPUI layer ordering; grouped layers and direct painting of cached shaped glyphs reduced median paint time to ~2.5 ms under the unreduced workload. See [the corrected investigation](STARTUP.md) and its visual QA list.

## Linux development dependencies (Ubuntu 24.04)

For experimental local builds, install:

```sh
sudo apt-get install build-essential clang cmake pkg-config libasound2-dev libfontconfig1-dev libfreetype6-dev libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev libxcb1-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev libxinerama-dev libxkbcommon-dev libvulkan-dev libssl-dev libzstd-dev
```

## Native Ghostty trial (2026-09-24, macOS arm64)

- Pinned gpui-libghostty 0.3.0 and its native Ghostty renderer compiled with Zig 0.16.0 against the existing GPUI 0.2.2. Release build and local app packaging/ad-hoc signing passed. Linked libraries are system frameworks/libraries; no Homebrew dylib dependency was found.
- All 22 Rust tests passed, including a real-Neovim notification test for quoted window-local working directories and preservation of Neovim's built-in terminal. Formatting, strict Clippy, Python script syntax and the packaging regression test passed.
- In an isolated app copy: shell command input/output, Ctrl-C interruption, Unicode clipboard paste, ANSI colours, window resize, hide/show with preserved output, terminal-close confirmation, cancellation preserving the shell, and confirmed app teardown were exercised.
- Rose Pine Moon loaded from the bundled theme resources. A generated PNG displayed through Kitty graphics. These checks establish basic native rendering, not full graphics-protocol or standalone Ghostty parity.
- Fixed resize/input attempts after Neovim exit when terminal-close confirmation is cancelled; the terminal remains usable beside an explicit exited-editor message.
- Automated background launches initially produced blank/occluded screenshots. Opening the app normally through Finder restored the editor and first terminal opening without a resize. This was not established as a Ghostty rendering failure. The final pane layout uses explicit viewport-derived heights and refreshes after native attachment.
- Linux/Wayland, Intel macOS, IME/dead-key composition, mixed-DPI displays, search, multiple sessions, comprehensive keyboard protocols and sustained performance remain unvalidated or unimplemented as detailed in `TERMINAL.md`. No comparison against standalone Ghostty performance is claimed.

## Live terminal theme sync (2026-09-24)

- `cargo test --locked --offline`: all 25 tests passed. The real-Neovim theme test covers direct `Normal`/`Visual` updates, `ColorScheme`, missing palette entries, reverse video and Base46's `NvThemeReload` with stale terminal globals.
- `cargo fmt --all -- --check`, strict Clippy over all targets and the release build passed. Existing dependency future-compatibility warnings remain.
- `python3 scripts/test-terminal-theme.py` passed with native macOS access: Metal pixels changed to the supplied background and returned to the baseline; the same live surface retained earlier output and accepted new input. The test allows a small RGB tolerance for macOS display colour conversion. The sandbox itself cannot create a Metal device.
- The bridge is implemented for both platform shims, but Linux/Wayland runtime validation is still outstanding. The native smoke test currently targets macOS only (`just check-terminal-theme`).

## Settings CLI installer (2026-09-24)

The Settings page now installs the CLI through Rust filesystem APIs, with a saved bin directory, native folder chooser and a default of `~/.local/bin`. No Python or installer subprocess is used. The installer creates an executable launcher pointing to the current app, updates Zvim-managed launchers, and refuses unrelated files, directories and symlinks. App Translocation requires moving and reopening the app before installation.

All 27 Rust/Neovim tests passed, including execution of an installed launcher from a quoted app path, exact argument/cwd forwarding, reinstalling, and collision protection. Preference tests also verify the custom bin directory round trip and backwards-compatible defaults. Formatting and strict Clippy passed. Native Settings UI interaction and Linux desktop integration were not exercised in this check.

## Terminal close experience (2026-09-24)

- All 28 Rust/Neovim tests passed. A new real-Neovim test waits for the unsaved-buffer prompt, cancels it, receives the close-return notification and verifies the same editor PID and unsaved text remain intact.
- The native macOS test verifies Ghostty requires confirmation for a running program, recognises an idle zsh prompt using isolated shell integration, then requires confirmation after starting `sleep` with a real Enter key. Existing Metal snapshot and live-theme checks still pass.
- Window close and Quit now confirm terminal shutdown before initiating Neovim exit, with a captured terminal frame behind the modal. Cancelling the terminal question leaves the editor running; cancelling a subsequent Neovim save prompt resets the close attempt. Direct editor exit retains a full-window terminal if the user chooses Keep Terminal.
- Neovim exit-hook cancellation was tested and rejected because an ExitPre exception does not reliably stop `:qall`. No interception hook remains in the implementation. Linux close detection and the full GPUI modal interaction still need interactive runtime QA; tests above exercise the native renderer and Neovim protocol independently.

## Resizable terminal and pane shortcuts

Automated checks cover split limits (including tiny windows), shortcut conflicts/reserved keys, old preference defaults and custom shortcut persistence. The full Rust/Neovim suite, formatting and strict Clippy pass. GPUI's capture routing and the native view's mouse pass-through were inspected, but live pointer dragging and shortcut recording have not been exercised in this session.

Manual follow-up: drag in both directions and beyond the window bounds; hide/show; maximise/restore from each pane; record a replacement shortcut and confirm the old one reaches Neovim/Ghostty again; cancel recording and reject duplicate/reserved shortcuts; restart to check persistence. Confirm the same shell and running process survive all layout changes.

## Terminal tabs and Zed-style defaults

- All 36 Rust/Neovim tests pass, including tab retention, selection wraparound, stable close identities, shortcut migration and key routing by terminal/editor context. Real-Neovim coverage checks that new tabs use the current window-local directory, including quoted paths.
- The native macOS smoke test passes with two independent shells. It verifies retained output and input after hiding/restoring and resizing, confirmation requirements for a hidden running shell, and survival of one shell after another is closed.
- Full app pointer interaction with the tab strip has not been automated in this session. Linux/Wayland multi-surface behaviour remains unverified.

## v0.0.4 release preparation

The user reports that terminal tabs and the revised Settings UI work well in their local app. The Settings page has General and Keybindings tabs, compact buttons and green/grey switches. Existing automated coverage includes 36 Rust/Neovim tests and the native macOS two-shell smoke test described above.

## Terminal splits

- All 39 Rust/Neovim tests and strict Clippy pass. Split tests cover independent tabs, nested geometry, directional focus, divider ratios, tiny windows, zoom visibility and sibling selection after closing a pane.
- Keymap tests verify Cmd+D / Cmd+Shift+D split right/down and Cmd+[ / ] navigate panes only in terminal context, while shifted brackets still switch tabs.
- The native macOS smoke test passes with simultaneous side-by-side and stacked surfaces, independent colours/output, zoom/restore and closing one shell while another remains alive.
- Full app split dragging and focus interaction still require manual confirmation. Linux/Wayland split rendering remains unverified.

Shell exit cleanup (2026-09-25): `cargo test --locked --all-targets` passed all 40 tests; strict Clippy, formatting and diff checks passed. The macOS native test verifies that visible and hidden interactive shells report exit and wake the adapter without another keypress. Layout regression coverage verifies that background removal preserves the selected pane and nested survivors expand to fill the freed area. Full-app exit/focus interaction remains a manual check.

Window geometry persistence (2026-09-25): saves moves/resizes after 300 ms of inactivity, uses content dimensions on macOS to match GPUI window creation, and preserves the last windowed placement during fullscreen. All 40 tests and strict Clippy passed. Interactive move/resize/relaunch verification remains manual.

Performance pass (2026-09-25): all 46 core Rust/Neovim tests, strict all-target Clippy, debug application build, formatting and diff checks passed. The native macOS theme/split/exit integration test passed. Three alternating before/after release GUI pairs measured 40–56% less editor CPU paint time across the four workloads; this is not an input-to-display latency claim. See `docs/PERFORMANCE.md` and its linked JSON reports for methodology, mixed results in smaller stages, cache allocation counts and reproduction.
