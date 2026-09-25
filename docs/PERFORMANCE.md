# Rendering and persistence review — 2026-09-25

This pass targets avoidable CPU work while preserving native Ghostty rendering, Neovim's ownership of editing, and the existing split/tab behaviour. Measurements use optimised release builds, not `just run`'s debug build. `just run-release` is available for everyday responsiveness comparisons.

## Changes

- Cache the terminal theme inputs and the generated configuration. Unchanged editor frames return before walking tabs or formatting strings. A changed configuration is shared across terminals; newly created tabs/splits still receive the current theme. Disabling theme synchronisation restores each terminal's Ghostty configuration.
- Construct terminal pane layouts and tab controls only when the dock is visible. Hidden shells continue receiving events and can exit normally.
- Use structured style keys and borrowed text lookups in the shaped-cell cache. Painting borrows grid cell text instead of allocating a clone. Cache hits do not allocate a formatted key or perform a second lookup.
- Bound the shaped-cell cache to 8,192 entries with second-chance eviction instead of clearing the whole cache. Recently used entries survive pressure from cold glyphs. Font changes still invalidate the cache; fallback fonts, Unicode sequences, decorations and cursor painting use the existing shaping/painting implementation.
- Read the geometry preference from the application preferences already in memory. A single background worker serialises writes, coalesces queued placements and skips duplicate placements. Files are atomically replaced. The application waits for submitted writes through an asynchronous quit hook. The 300 ms resize/move debounce remains; isolated diagnostic windows do not save user geometry.

## Measurement methods

`examples/performance_ui.rs` opens the real GPUI editor with an isolated Neovim state directory, `-u NONE`, a fixed Menlo font and a fixed initial window size. A Lua timer scrolls and edits a syntax-highlighted buffer containing ASCII, CJK, combining marks and emoji. The harness requests refreshes every 16 ms and exercises four phases:

1. Editor only.
2. Eight shell tabs with the dock hidden.
3. Ten shells across three visible terminal panes.
4. Repeated window resizing with those panes visible.

Each phase samples 180 refresh requests after settling. Actual rendered frame counts vary with scheduling; the timer interval is not an asserted frame rate. The script uses `/bin/sh` for terminal shells and the current Ghostty configuration. Before and after binaries are run alternately three times without concurrent builds. Configuration, hardware and display remain the same. CPU-stage tracing runs in both binaries and adds overhead, particularly to very short stages. These results do not measure input-to-display latency, GPU completion time, energy use, or human-perceived smoothness. The resize workload resizes the whole window, not a divider through native mouse input.

An intermediate run after only theme caching and hidden-UI elimination is retained separately. The initial exploratory baseline overlapped compilation; use the final alternating comparison for conclusions.

`examples/performance_cache.rs` compares the previous lookup/formatting algorithms with the production caches. Five samples measure 250,000 warm text lookups and 10,000 unchanged theme frames with eight terminals. Both paths have allocation counting enabled. Cached integers stand in for shaped lines: these are algorithm measurements, not timings of glyph shaping or GPU rendering. A separate pressure workload uses 12,000 cold keys interspersed with a hot set of 256 keys.

The geometry writer has deterministic regression coverage that blocks the writer, submits newer placements without releasing it, then verifies only the first and latest placements are written in order. A shutdown test verifies that the last placement reaches an atomically replaced file. We do not claim a measured whole-app frame-rate gain from background disk writes; their purpose is to keep slow filesystem work off the UI thread.

## Results

Median of the three per-run medians; p95 is likewise the median of the three per-run p95 values. Times below measure the editor's CPU paint stage in milliseconds.

| Workload | Before | After | Less paint CPU time | p95 before → after |
| --- | ---: | ---: | ---: | ---: |
| Editor only | 2.889 | 1.649 | 43% | 4.373 → 2.363 |
| Eight hidden tabs | 2.779 | 1.670 | 40% | 4.097 → 2.341 |
| Three visible panes | 2.322 | 1.132 | 51% | 3.276 → 1.650 |
| Window resize with panes | 0.996 | 0.441 | 56% | 1.251 → 0.586 |

Editor element construction with eight hidden tabs fell from 0.169 ms to 0.099 ms (42%). The hidden terminal-UI stage disappears entirely. Visible-pane element construction did not improve in this sample: 0.200 → 0.216 ms; resizing element construction was 0.091 → 0.094 ms. These small stages are sensitive to scheduling and tracing overhead. Theme-sync timings were mixed, including worse traced medians without visible terminals, so they are not presented as a consistent GUI-stage improvement.

The separate algorithm benchmark recorded:

| Work | Before | After | Allocation calls before → after |
| --- | ---: | ---: | ---: |
| 250,000 warmed text lookups | 75.597 ms | 6.188 ms | 1,250,000 → 0 |
| 10,000 unchanged frames, eight terminal configurations | 89.735 ms | 0.268 ms | 560,000 → 0 |

Cache pressure caused 12,512 misses before and 12,256 after. The benefit is avoiding a burst of 256 hot-entry misses when the previous implementation cleared everything, not a large reduction in total misses for this workload. A full second-chance sweep is still possible under saturation; this is not a hard real-time cache.

Evidence: [alternating GUI runs](benchmarks/performance-gui-2026-09-25.json), [cache benchmark](benchmarks/performance-cache-2026-09-25.json), [intermediate theme/hidden-UI run](benchmarks/performance-theme-stage-2026-09-25.json), and [exploratory baseline](benchmarks/performance-before-2026-09-25.json). The final GUI benchmark preceded only the addition of a filesystem-failure retry to the writer; diagnostic windows do not write geometry, so that change does not affect this measured path.

## Reproduction

```sh
cargo build --release --locked --example performance_ui --example performance_cache
python3 scripts/bundle-ghostty.py
python3 scripts/benchmark-ui.py --output .cache/performance/gui.json --runs 3
target/release/examples/performance_cache > .cache/performance/cache.json
```

The GUI benchmark opens a temporary window and real terminal shells. Raw traces go to `.cache/performance/`; summary JSON is saved at the requested output path. Preserve a before binary before editing to reproduce an alternating comparison. The checked-in comparison records the binary SHA-256 hashes and macOS platform version.

## Validation

All 46 core tests, strict all-target Clippy, the application build and native macOS theme/split/exit checks passed. Tests cover cache eviction under pressure, Unicode and distinct styles, invalidation, theme changes/restoration, ordered nonblocking saves and shutdown persistence. Existing Neovim protocol, terminal layout, keybinding and native macOS theme/split/exit checks remain applicable. Linux/Wayland runtime measurements and input-to-display latency are outside this run.
