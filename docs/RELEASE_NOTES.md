A focused Neovim frontend built with GPUI. This is an early development release.

- Bundled Neovim 0.12.5; loads your existing Neovim configuration.
- Native settings for appearance, window placement and the application icon.
- Command-line file/directory opening and Neovim layout flags.
- Official Neovim artwork with attribution included.

Download the archive for your OS and processor. Apple Silicon Macs use `macos-arm64`;
Intel Macs use `macos-x86_64`. macOS archives contain `Zvim.app`; move it to Applications.
`SHA256SUMS.txt` contains checksums for all archives.

macOS Apple Silicon has been exercised locally. Other platform builds require further
interactive runtime testing; compilation and automated tests do not establish full support.
macOS applications are ad-hoc signed, not Developer ID signed or notarised. Windows
executables are not Authenticode signed. Managed computers may require IT approval.
