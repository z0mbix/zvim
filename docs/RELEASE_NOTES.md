A focused Neovim frontend built with GPUI. This is an early development release.

## Changes in 0.0.13

- Updated the embedded terminal to Ghostty 1.4.0, including upstream security fixes for Kitty graphics and shell integration. Zvim uses the native Metal and Wayland surfaces through `gpui-libghostty` 0.4.1-rc.1. [Ghostty 1.4 release notes](https://ghostty.org/docs/install/release-notes/1-4-0#security)
- Bundled Neovim 0.12.6. The bundle cache now separates Neovim archives by version so upgrades can reuse the correct download.
- Updated the development toolchains to Rust 1.99.1 and Zig 0.17.1, and refreshed the Rust dependency and GitHub Actions pins.

## Install

Requires an Apple Silicon Mac running macOS 13 or newer. Install with `brew install --cask z0mbix/tap/zvim`, or quit all existing Zvim instances, download the macos-arm64 archive, and replace `/Applications/Zvim.app`. Reopen Zvim to use the new application host. Use Settings → General → Command-line launcher to install the optional CLI. Keep the chosen bin directory on your shell's PATH.

The bundled Neovim loads your existing Neovim configuration. Zvim includes the Ghostty engine and its resources. `SHA256SUMS.txt` contains the archive checksum.

## Known limitations

The terminal remains experimental. Shell input IME support and interactive Wayland testing remain open. Remember terminal layouts restores pane geometry and shell directories by starting fresh shells. GitHub Actions builds and tests Linux x86_64, and the public release archive targets Apple Silicon Macs.

The macOS application uses ad-hoc signing. If macOS quarantines a manually installed copy, use `xattr -dr com.apple.quarantine /Applications/Zvim.app` after verifying the archive checksum.
