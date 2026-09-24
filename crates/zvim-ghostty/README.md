# Zvim native Ghostty bridge

This is the Rust native wrapper and platform shims from `gpui-libghostty`
revision `bfa3771f0ef2290e54acf9f1fc11e05d42a882ee`, under its MIT licence.
The matching `ghostty.h` is from its vendored Ghostty revision
`9f0e1719dc918368367d368bfe300f59bb68b5a4` (MIT; see `GHOSTTY-LICENSE`).

The upstream Cargo dependency still builds and links the pinned Ghostty engine.
Only the small embedding layer is copied here, avoiding a second copy of the
Ghostty source or changes to Cargo's dependency checkout. Native shim symbols and
the AppKit view class use a Zvim prefix to avoid collisions with upstream.

Changes from upstream:

- Omit the generated GPUI adapter; Zvim has its own GPUI 0.2.2 adapter.
- Compile the platform shim with `cc`; reuse upstream's native engine build.
- Expose `NativeSurface::set_color_config` and a platform shim function that
  clones the initial configuration, applies a temporary colour overlay and calls
  `ghostty_surface_update_config`. Passing `None` restores the original config.
- Expose Ghostty’s `needs_confirm_quit` check for idle-shell aware close prompts.
- Temporary files are privately created by `tempfile` and removed after parsing.
  The application supplies only validated RGB colour values.

Surface lifetime, focus, clipboard and platform handling remain upstream code.
On upgrades, compare these files against upstream, update the header with the
engine, and repeat the macOS rendering test and Linux runtime checks.
