# Startup picker investigation — 2026-09-24

**Correction:** the initial investigation below measured opening/search work, but the reported problem was lag while typing and navigating an already visible picker. The rendering bottleneck and its fix are described first.

## Typing/navigation lag: reproduced and fixed

With `.ignore` bypassed only in the test environment, native keyboard input exposed a GUI-thread bottleneck. Neovim accepted input quickly, but each frame blocked the GUI thread for roughly 180 ms. Sampling the process showed GPUI `BoundsTree::insert` / `find_max_ordering` beneath `paint_line` consuming the paint time.

Zvim was calling `ShapedLine::paint` separately for every cell. That method creates a paint layer; inserting thousands of cell-sized layers in grid order caused expensive bounds-ordering work. The renderer now groups non-overlapping cell backgrounds and glyphs into separate grid-wide layers and uses GPUI's glyph painting APIs for cached shaped cells. Cursor and composition overlays remain ordered above the grid. Undecorated spaces need no glyph paint.

| CPU stage | Before median | After median |
| --- | ---: | ---: |
| Frame paint | 179.259 ms | 2.521 ms |
| Redraw reduction | 0.181 ms | 0.245 ms |
| Input RPC | 0.165 ms | 0.235 ms |

The paint samples numbered 23 before and 36 after, with maximums of 272.784 ms and 12.247 ms. This is approximately **71× less CPU paint time**, not a claim about total application speed or input-to-photon latency. Both runs used the same saved window geometry, configured font, and native typing/navigation sequence. The file list grew from 21,396 to 22,834 due to the rebuild; the fix was tested without shrinking the workload. [Recorded measurements](benchmarks/picker-render-2026-09-24.json).

Visual QA covered the picker, selected rows, colours, Nerd Font icons, emoji including a ZWJ sequence, CJK, combining accents, underline/undercurl/strikethrough including spaces, bold/italic, a block cursor, and a floating window. All 20 automated tests and strict Clippy passed. Cross-platform runtime QA remains outstanding.

To trace a future regression locally (the trace records stage names and times, not typed text):

```sh
mkdir -p .cache
ZVIM_TRACE_STARTUP=1 zvim --wait . 2>.cache/picker.trace
# Exercise the picker, then close this instance normally.
python3 scripts/analyze-startup-trace.py .cache/picker.trace
```

To include generated files without editing the project's ignore files, create a temporary ripgrep config containing `--no-ignore-dot` and pass its path as `RIPGREP_CONFIG_PATH` only to the test instance.

## Earlier startup-only investigation

Investigated the user's normal NvChad configuration when opening Zvim in this project with no file arguments. Its `VimEnter` callback opens Telescope `find_files` because this directory has no `.git` directory.

## Findings and changes

1. Telescope's `rg --files` returned **21,182 files**, including generated build/package/runtime files. Ripgrep was not applying `.gitignore` outside a Git repository. A project-local `.ignore` now excludes those directories independently of Git metadata. At measurement time the result count fell to **29**; source additions will naturally change that count. No global ripgrep or Neovim configuration was modified.
2. The GUI enumerated every installed font when resolving `guifont`, including an initial empty value. Profiling measured **135–152 ms** in font resolution. It now resolves only explicitly requested families, checks whether resolution fell back, and preserves the configured candidate order. Measured resolution time fell to **6–10 ms**, including the final installed CLI checks. A missing first font followed by JetBrainsMono Nerd Font still produced the same 221-column grid at size 13.
3. Zvim attaches an initial 100×35 grid, then resizes after receiving the configured font. Telescope relayouts once. This remains, as does native window/GPU startup overhead. These changes do not establish parity with terminal startup.

## Measurements

Telescope completion was instrumented with a temporary pre-configuration Lua probe wrapping the picker's completion callback. All sessions loaded the user's real config, opened no files, and disabled ShaDa writes with `-i NONE`. They closed themselves after four seconds. Probe files did not change the config.

| Stage | Observed local samples |
| --- | --- |
| Configuration to VimEnter, initial warm GUI samples | 24–26 ms |
| Telescope completion, original GUI | 603–704 ms from pre-config probe |
| Telescope completion, `.ignore` only | 202 ms from pre-config probe |
| Telescope completion, `.ignore` and font lookup optimization | 166–313 ms from pre-config probe |
| Font resolution before optimization | 135–152 ms |
| Font resolution after optimization | 6–10 ms |

The final range includes a slower first launch and a font-fallback test; it is not evidence that font lookup alone improves Telescope's search time. First-frame and full-process launch times vary with macOS initialization. Raw stage measurements and frontend timings are in [the recorded samples](benchmarks/startup-2026-09-24.json).

Both system and bundled Neovim report 0.12.5. Their initial synthetic-terminal picker completion measurements were similar (about 0.52–0.60 s); the bundled engine did not explain the discrepancy. Those initial PTYs did not answer terminal colour queries and logged a warning, so they are not exact measurements of the user's terminal. Later PTY checks disabled those queries with `NVIM_NOTTYFAST=1` and used a larger grid; their completion measurements after ignoring generated files were about 0.12–0.20 s from the probe. Do not treat these short samples as a controlled cross-application benchmark.

The final installed CLI smoke tests completed the picker at **1.45 s on the first launch after packaging** and **0.71 s on the subsequent launch**, measured from process creation. Time before the pre-config Neovim probe was **1.11 s** and **0.36 s** respectively. These measurements show a remaining native/process initialization cost; its detailed causes are not yet isolated. The list contained 31 files after adding these investigation documents. Neither run reported Neovim errors.

## Repeat basic checks

From the project directory:

```sh
rg --files | wc -l
mkdir -p .cache
nvim --startuptime .cache/terminal-startup.log
zvim --startuptime .cache/gui-startup.log .
```

Close each test editor normally. Neovim's `--startuptime` records configuration and its first screen update; it does not include the frontend's window/GPU setup or necessarily Telescope's completed asynchronous search. Zvim's `startup.log` records `first_grid_paint_ms` relative to editor-view construction and `font_resolution_ms` accumulated before that frame. It is also not an end-to-end launch timer.
