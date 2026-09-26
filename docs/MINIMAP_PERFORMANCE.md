# Minimap resize investigation, 2026-09-25

The installed minimap.vim (commit `28c530f8e0929b73ef27c86f705ff8bcfcec97d8`) blocks Neovim during resize. The user configuration enables Git colours, viewport highlighting and a 16-column map. The >200-line autocmd runs on BufWinEnter/BufReadPost to open the map; it is not the resize handler.

The plugin's `VimResized` handler calls `refresh_minimap(1)` and `update_highlight()`. The forced refresh synchronously pipes the entire buffer through `code-minimap`. Git highlighting synchronously executes `system('git diff -U0 -- ' . expand('%'))`, invoking the user's shell for each resize. This machine's default shell is Fish. The map generator temporarily uses sh, but Git highlighting restores and uses Fish.

## Measurements

Three runs of 40 callbacks each, using bundled Neovim, `-u NONE`, no ShaDa, only the installed minimap plugin and the relevant user settings, on `src/ui.rs`. Each sample changes rows/columns and explicitly invokes VimResized. Neovim function profiling was enabled. These are isolated synchronous callback costs, not actual macOS drag/render latency and not the complete user configuration.

| Configuration | Median callback | p95 |
| --- | ---: | ---: |
| Minimap closed | 0.074 ms | 0.186 ms |
| Minimap with configured Git colours, Fish | 129.257 ms | 158.742 ms |
| Minimap, Git colours disabled | 12.288 ms | 19.194 ms |
| Minimap with Git colours, shell changed to /bin/sh | 32.873 ms | 39.516 ms |

An initial function profile attributed approximately 90% of callback time to `minimap_color_git`; generation accounted for most of the remainder. The shell comparison shows that shell startup is a major contributor, but even sh leaves substantial synchronous work. Turning off Git colours reduced median callback time by approximately 10.5 times in these runs.

Zvim's `layout_grid` sends `nvim_ui_try_resize` whenever the calculated grid dimensions change. Its local `requested` field avoids identical requests, but there is no in-flight resize coalescing. This can amplify a slow plugin during continuous dragging; these measurements establish a plugin bottleneck without involving the GPUI renderer.

## Options

- Immediate, smallest configuration change: set `g.minimap_git_colors = 0` in variables.lua. The minimap and viewport highlighting remain available. This was tested only in the isolated benchmark; no user settings were changed.
- Preserve Git colours: cache or asynchronously refresh Git state, and debounce minimap regeneration until resizing settles. This belongs in the plugin or a carefully scoped adapter; avoid a global shell change merely to work around this plugin.
- Separately, coalescing pending resize requests in Zvim would prevent stale sizes accumulating. It cannot make a blocking 129 ms callback cheap, and needs its own measurements before claiming an improvement.

Reproduce from the repository root:

```sh
MINIMAP_BENCH_MODE=on NVIM_LOG_FILE="$PWD/.cache/minimap/nvim.log" \
  vendor/macos-arm64/neovim/bin/nvim --headless -u NONE -i NONE -n \
  -l scripts/benchmark-minimap.lua
```

Repeat with `off`, `no_git` and `sh`. The script opens only an isolated headless editor, reads src/ui.rs without saving, and writes timings/profiles under .cache/minimap. Recorded samples are in [the benchmark JSON](benchmarks/minimap-resize-2026-09-25.json).
