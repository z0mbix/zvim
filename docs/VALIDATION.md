# Validation status

This is a macOS prototype with cross-platform source/build infrastructure. Do not label it a validated three-platform v1 yet.

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
- Live OS appearance changes, independently launched instance activation reloads, and Linux/Windows settings UI still require runtime QA.

## Base46 appearance regression (2026-09-24)

- Reproduced with the user's actual NvChad configuration: changing `background` replaced the custom Rose Pine Moon Normal highlights with Neovim defaults, while `nvconfig.base46.theme` still reported `rose-pine-moon`.
- The sync bridge now preserves Base46 palettes instead of changing `background`. Regression coverage includes preserving the custom palette and repairing state from the former implementation.
- `cargo run --offline --locked --no-default-features --example diagnose -- --config --appearance` verified the actual configuration retains `background=dark`, `theme=rose-pine-moon`, and Normal foreground/background through light, dark, and disabled transitions. No Neovim configuration files were edited.

## Still required before v1

- Run the GitHub Actions matrix; Linux/Windows/Intel macOS results are not yet available from this machine.
- Runtime tests on Linux under Wayland and X11, and on Windows with display scaling and AltGr layouts.
- Comprehensive IME/CJK input and candidate placement, less common font fallbacks, mixed-DPI monitor transitions, drag/drop, and accessibility review.
- Resolve an intermittent `CmdlineChanged` Lua callback error observed in the existing configuration. It did not reproduce in a separate embedded-editor diagnostic session; its cause is not yet established. Broader plugin workflow compatibility is not certified.
- Public signing/notarisation, broader Linux distribution compatibility, and Windows distribution QA.

## Performance

The frontend records time from editor-view construction to first grid paint in `startup.log`; this excludes OS launch and pre-window GPU setup. It is not an end-to-end launch benchmark. Measurements should report frontend and Neovim together and use identical configuration/files for comparisons.

Initial debug-package idle observation before further polishing: 65,520 KiB frontend RSS + 8,656 KiB Neovim RSS (about 72.4 MiB combined), with both reporting 0.0% CPU in a point sample. This is not a release-build benchmark, long-duration CPU measurement, or comparison with Zed. Full scrolling latency and mixed-DPI profiling remain outstanding.

Warm configured-session observation: 187 ms from editor-view construction to first grid paint. This excludes initial OS/window/GPU startup and is a single sample, not a launch-time guarantee.

## Startup picker performance (2026-09-24)

See [the startup investigation](STARTUP.md) for measured stages, raw samples, and limitations. Added project-local generated-file exclusions and replaced whole-system font enumeration with candidate lookup. All 20 automated tests and strict Clippy passed; release GUI probes verified the configured Nerd Font and missing-first-font fallback. Native window/GPU initialization and the initial grid resize remain measurable overheads.

## Picker interaction rendering fix (2026-09-24)

The earlier startup-only changes did not establish a fix for typing/navigation lag. A subsequent native interaction test with ignore rules bypassed reproduced ~179 ms median CPU paints. Process sampling identified per-cell GPUI layer ordering; grouped layers and direct painting of cached shaped glyphs reduced median paint time to ~2.5 ms under the unreduced workload. See [the corrected investigation](STARTUP.md) and its visual QA list.
