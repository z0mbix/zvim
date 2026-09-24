A focused Neovim frontend built with GPUI. This is an early development release.

- Bundled Neovim 0.12.5; loads your existing Neovim configuration.
- Native settings for appearance, window placement and the application icon.
- Command-line file/directory opening and Neovim layout flags.
- Official Neovim artwork with attribution included.

This release targets macOS. Apple Silicon Macs use `macos-arm64`;
Intel Macs use `macos-x86_64`. macOS archives contain `Zvim.app`; move it to Applications.
`SHA256SUMS.txt` contains checksums for all archives.

macOS Apple Silicon has been exercised locally. Intel macOS has passed CI builds and tests but still needs broader interactive testing.
Linux and Windows are not included in this release.
macOS applications are ad-hoc signed, not Developer ID signed or notarised. Managed computers may require IT approval.
