# Zvim

A focused native frontend for Neovim, written in Rust with GPUI. Neovim owns editing, splits, plugins, LSP, and configuration. Zvim draws the screen and integrates input, clipboard, windows, and file opening with the desktop.

**Platforms: macOS and Linux only.** macOS is exercised locally; Linux remains experimental. The native Ghostty terminal requires Wayland on Linux; the editor can still run on X11. See [validation](docs/VALIDATION.md).

## Run on macOS

Requires macOS 13 or newer, Rust 1.98.1, Apple command-line developer tools, Python 3, curl, and Zig 0.16.0. GPUI's runtime Metal shader compilation is enabled, so the separate Xcode Metal compiler component is not required.

```sh
python3 scripts/bundle-neovim.py
cargo build --locked
python3 scripts/bundle-ghostty.py
cargo run --locked -- --clean
cargo run --locked -- path/to/file.rs
```

The first command downloads Neovim 0.12.5 from its official release and verifies the SHA-256 in `packaging/neovim.json`. No system Neovim is required. The entire runtime is bundled, not just the executable.

```sh
cargo build --release --locked
python3 scripts/package.py
open dist/zvim-macos-arm64/Zvim.app
```

On an Intel Mac the directory is `dist/zvim-macos-x86_64`. The `.app` is signed ad hoc for local use; it is not notarised for public distribution. Signing identities and notarisation credentials are deliberately not part of the repository.

## Command-line launcher

After moving Zvim to its permanent location, open **Settings → Command-line launcher**:

1. Keep the default `~/.local/bin`, or use **Choose folder…** to select your bin directory.
2. Click **Install / update zvim**.

The app installs the launcher directly, with no Python or additional tools required. The selected directory is remembered, missing default directories are created, and unrelated commands or symlinks are never replaced. Choose a writable location; the app does not request administrator privileges. If you move the app, repeat installation. Choosing a different directory leaves the previous launcher in place.

Keep that directory on your shell's PATH. For fish, run `fish_add_path ~/.local/bin` once when using the default location. For zsh, add `export PATH="$HOME/.local/bin:$PATH"` to `~/.zshrc` and open a new terminal. The app does not edit shell configuration.

The Python installer remains available for development or scripted setup (`python3 scripts/install-cli.py --app /Applications/Zvim.app --bin-dir ~/.local/bin`), but is not needed when installing from Settings.

```sh
zvim .
zvim ~/Projects/foo
zvim -O left.rs right.rs
zvim -o2 a.txt b.txt
zvim -p a.txt b.txt
zvim -R file.txt
zvim -d before.txt after.txt
zvim +42 src/main.rs
zvim --clean .
zvim --wait .
```

Each launcher invocation opens a new application instance and returns immediately. `--wait` keeps it attached until the application exits. Shell environment variables are inherited. If the first file operand is an existing directory, it sets Neovim's working directory **before configuration loads**. All remaining relative paths, including `-u` and `-S` values, resolve relative to that directory. Otherwise the shell's working directory is retained. Neovim configuration supplies project navigation.

GUI-compatible Neovim options are forwarded in their original order, including split/tab counts, readonly/diff modes, `-u`, `-S`, `--cmd`, `-c`, `+command`, and `--listen`. Use `--` before filenames beginning with `-` or `+`. `--help` and `--version` print in the terminal. Headless, Ex/batch, standalone Lua, remote-client, and stdin-input modes conflict with the embedded GUI connection and produce an error; use Neovim directly for those. Detached startup errors are recorded in `launcher.log` in Zvim's configuration directory.

## App settings

Open **Zvim → Settings…** (`⌘,` on macOS; `Ctrl+Shift+,` elsewhere). Changes save automatically.

- **Appearance: System / Light / Dark** styles Zvim controls. System responds to desktop appearance changes; native titlebars continue to follow the OS.
- **Sync editor appearance**, off by default, also sets Neovim's `background` option. Your colourscheme must support both appearances. NvChad/Base46 palettes are preserved unchanged because changing `background` resets their custom highlights; Zvim does not choose a replacement theme. Disabling sync restores the background from before sync was enabled unless you changed it yourself in Neovim. No configuration files are edited.
- **Remember window size and position**, on by default, reuses the last saved editor-window placement. Turning it off opens new windows centred at the default size and stops saving geometry. This does not restore files, sessions, or multiple window layouts.
- **Application icon** previews the bundled Neovim mark. The saved `app_icon` ID is `neovim`; this release has one choice. Unknown IDs fall back to it without losing other settings. Additional artwork and platform switching can be added later. The macOS bundle embeds the icon for Finder, the Dock and the app switcher.
- **Open diagnostics folder** reveals the directory containing settings and startup/editor logs.

Preferences live in `preferences.json`, separate from `window.json`. Changes apply to open windows in the same instance; independently launched instances reload preferences when activated. Font and plugin settings remain in Neovim.

## Behaviour

- `zvim [files...]` loads your normal Neovim configuration. `zvim --clean [files...]` starts without it. Use `--` before a filename beginning with `-`.
- Each window owns one bundled editor process. `g:zvim` is true before your configuration loads; `ZVIM=1` is also available to child tools.
- Set `vim.opt.guifont = 'Menlo:h16'` (or your installed monospace font) and `vim.opt.linespace` in your configuration. Highlight colours and cursor shapes come from Neovim.
- On macOS: Command-O opens files, Command-V pastes, Command-W requests close, Command-N opens a window, and Command-Q requests closing all windows. Ctrl shortcuts are forwarded to Neovim except Ctrl+backtick, which toggles the native terminal. Menus also expose Open, Paste, New Window, and Close.
- Mouse clicking, selection, dragging, and scrolling require Neovim's `mouse` option, for example `set mouse=a`.
- The `+` and `*` registers use the system clipboard. Neovim's own register commands provide copy/cut; Zvim does not replace Ctrl-C.
- File → Open, Finder's Open With, and file drops open files in the current session. Opening files never forces away unsaved edits.
- Close requests use `:confirm qall`. Choose Yes/No/Cancel in Neovim. No automatic discard or forced shutdown on ordinary close.
- Window bounds, `neovim.log`, and the latest `startup.log` are stored in the platform's Zvim configuration directory. Neovim's configuration is not edited.
- Desktop launches preserve PATH and append common Unix package-manager locations. Tools in custom shell-only paths still need to be on the desktop environment's PATH or configured in Neovim.

Your existing plugins remain responsible for their own installations, external programs, and network activity. Zvim adds no AI, accounts, collaboration, telemetry, or automatic updates.

## Native terminal (experimental)

Press **Ctrl+`** or choose **Terminal → Show / Hide Terminal**. From Neovim, use `:ZvimTerminal` (which you can map in your own configuration). The first opening starts `$SHELL -l` in Neovim's current window-local working directory. Hiding retains the shell, scrollback and running programs; showing resumes it. After a shell exits, hide and show the pane to start another.

The pane uses Ghostty's native Metal renderer on macOS and OpenGL renderer on Linux/Wayland. It loads your Ghostty configuration for fonts and keybindings. Terminal colours follow Neovim live by default, including NvChad/Base46 themes; choose **Use Ghostty theme** under Settings → Terminal theme to opt out. Zvim's editor font remains controlled by Neovim. Ghostty application actions such as opening tabs/windows and settings are not implemented by this embedder. This first trial has one terminal per window, a fixed 40% bottom pane, and no terminal search UI or session restoration. Built-in `:terminal` and existing terminal plugins retain Neovim's behaviour.

Copy/paste use Ghostty bindings (normally Command-C/V on macOS, Ctrl-Shift-C/V on Linux). Terminal → Close Terminal asks before ending a live shell. Window close and Quit warn about terminal processes before closing Neovim; Cancel preserves the editor and shell. Idle prompts detected by Ghostty close quietly. If Neovim is quit directly, **Keep Terminal** expands the remaining session to fill the window.

The native dependency and its patched Ghostty source are pinned in Cargo. The initial build compiles Ghostty using Zig and downloads its dependencies; no standalone Ghostty installation is required. The integration is a trial, not a claim of complete standalone Ghostty parity. See `docs/TERMINAL.md` for limitations and verification.

## Development checks

```sh
cargo fmt --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

Run the bundling step before tests. The integration tests use real bundled Neovim with isolated state. `--no-default-features` runs the protocol/process tests without compiling a GUI. `just check`, `just run`, and `just package` are optional shortcuts.

## Linux

CI builds macOS arm64/x86-64 and Linux x86-64. Releases currently publish macOS app bundles; Linux archives remain experimental build artifacts.

- Linux: install the GPUI development dependencies listed in `docs/VALIDATION.md`; use `python3 scripts/bundle-neovim.py`, then `cargo build --release --locked` and `python3 scripts/package.py`. The archive contains `zvim`, `neovim/`, and a desktop entry. Keep the runtime beside the binary; add that directory to PATH before installing the desktop entry under `~/.local/share/applications`. Wayland and X11 editor backends are enabled. The Ghostty pane requires Wayland, EGL, OpenGL 4.3, libxml2 and libc++ 21 or newer.

## Architecture and limitations

See [architecture](docs/ARCHITECTURE.md) and [validation](docs/VALIDATION.md). In particular, cell-based text rendering does not currently provide cross-cell ligatures, screen-reader document semantics, images, or externalised graphical widgets. Native input-method hooks and composition display are implemented, but comprehensive CJK/IME and mixed-DPI QA remains required.

## Licences

Original Zvim code is Apache-2.0. GPUI is used as a pinned library; no Zed editor/application code is imported. Packaging preserves the upstream Neovim licence/runtime and collects available licence files and declared licence expressions for resolved Rust dependencies. Release maintainers must review that inventory when updating dependencies. See [third-party notes](THIRD_PARTY_NOTICES.md).

## Releases

Download builds from [GitHub Releases](https://github.com/z0mbix/zvim/releases).
Versions follow SemVer, starting at **0.0.1**. To publish a release:

1. Update the package version in `Cargo.toml` and refresh `Cargo.lock` with `cargo check`.
2. Update `docs/RELEASE_NOTES.md`, run the checks, and commit the changes.
3. Push the commit and a matching annotated tag, for example:

   ```sh
   git push origin main
   git tag -a v0.0.1 -m 'Zvim v0.0.1'
   git push origin v0.0.1
   ```

GitHub Actions rejects tags that do not match the package version. Both macOS
builds must pass before it publishes versioned archives and `SHA256SUMS.txt`.
SemVer tags containing a prerelease suffix publish as GitHub prereleases. The release
is staged as a draft until all assets upload; failed jobs can be rerun.
The macOS bundle version is derived from Cargo's version. Public signing and
notarisation are not configured.
