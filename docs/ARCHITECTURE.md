# Architecture

`src/session.rs` owns a child Neovim process and its MessagePack stream. Dedicated reader and writer threads keep blocking pipes off the GPUI thread. An independent keyboard worker serialises `nvim_input` calls and resubmits any unconsumed suffix. General commands are notifications, so a `confirm qall` prompt cannot hold up subsequent keyboard input. RPC responses for synchronous clipboard provider requests bypass pending editor commands.

`src/grid.rs` is a GUI-independent linegrid reducer. It handles highlights, repeat cells, empty wide-character continuation cells, clipped scroll regions, cursor mode information, titles, options, and defaults. Unknown event kinds and extra parameters are ignored. Mutations stay private until `flush`; the process bridge coalesces complete frames into a single latest-frame slot so slow painting cannot accumulate screen snapshots indefinitely.

`src/ui.rs` paints backgrounds before glyphs and preserves Neovim cell coordinates. Shaped cell text is cached, including font/highlight attributes; font changes invalidate the cache. GPUI provides font fallback, logical-pixel scaling, and platform rendering. Cursor rendering respects mode shape and percentage. The initial implementation favours cell correctness over cross-cell ligatures or connected-script shaping.

Neovim remains the source of truth for layout and selection. Zvim requests a grid size from the usable window area but renders the most recent complete grid until the resize response arrives. GUI commands never copy buffer contents into a separate editor model.

Native input method composition is kept locally until committed, with an underlined preedit overlay and cursor bounds for candidate windows. Committed text is escaped for `nvim_input`; clipboard paste uses `nvim_paste`. The IME surrounding-text model currently contains preedit text only, so reconversion of existing buffer text is not implemented.

The OS window-close callback vetoes immediate destruction and asks Neovim to confirm closing. A successful child exit closes the window; abnormal exits remain visible as an error. Process cleanup on teardown reaps the owned child. No remote session is killed because remote sessions are not supported.

`packaging/neovim.json` pins upstream assets and hashes for each target. Scripts assemble the whole Neovim tree beside the application, preserve notices, and never silently fall back to a system editor. The build matrix establishes platform build checks, not runtime certification.

`src/cli.rs` preserves native OS arguments and Neovim option order while consuming an optional first directory operand. Option values are distinguished from file operands so startup scripts and configuration paths are never mistaken for project roots. The launcher installed by `scripts/install-cli.py` invokes a detached child through the app executable, preserving the shell environment and working directory; `--wait` runs in the foreground. Each invocation creates a new instance. The process bridge sets the chosen working directory before executing Neovim.

`src/preferences.rs` provides a separate GPUI settings window with keyboard navigation. App preferences are atomically replaced in `preferences.json`, independently of editor geometry. Global observers update live windows; activation reloads preferences written by another instance without an idle polling loop. Appearance syncing is optional and uses Neovim RPC; disabling it restores the prior background if it has not been changed independently. Settings windows own no editor process and are excluded from file-open routing.

Grid rendering uses a shared layer for highlighted cell backgrounds and another for glyphs. Each cached cell layout supplies its font fallback runs and positioned glyphs directly to GPUI; it does not create a `ShapedLine::paint` layer per cell. This avoids pathological bounds-ordering work for large regular grids while retaining Neovim cell placement, emoji and decorations. Optional `ZVIM_TRACE_STARTUP=1` stage timings include input RPC, redraw reduction, and CPU frame painting.
