# Application artwork

`neovim.png` is the unmodified `neovim-mark.png` from the official logo archive.
See ../../packaging/NEOVIM-LOGO-NOTICE.txt for credit, licence, source and hashes.

`neovim-app.png` is the transparent 1024-pixel square master. Regenerate on macOS:

```sh
swift scripts/render-icon.swift assets/icons/neovim.png assets/icons/neovim-app.png
```

Packaging uses `scripts/macos-icon.py` with Apple's `sips` and `iconutil` to build
16–1024 pixel representations into `Zvim.icns` before signing the app bundle.
No download or third-party graphics tool is needed during packaging.

`src/icons.rs` is the UI catalogue; `Preferences.app_icon` stores a stable string
ID. Unknown IDs resolve to the bundled default while remaining preserved on disk.
There is intentionally only one choice today. Adding alternatives also requires
platform application logic (e.g. macOS Dock/app-switcher overrides); changing a
runtime preference must not mutate the signed bundle. Finder uses the bundle icon.
