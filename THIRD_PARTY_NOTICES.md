# Third-party components

- GPUI 0.2.2: Apache-2.0, https://github.com/zed-industries/zed/tree/main/crates/gpui (package source: https://crates.io/crates/gpui/0.2.2). Zvim does not import Zed's editor, collaboration, or AI crates.
- Neovim 0.12.5: see `packaging/NEOVIM-LICENSE.txt`, also copied into packages. The full upstream runtime includes Vim notices in `share/nvim/runtime/doc/uganda.txt` and runtime component licences.
- Neovim logo: Jason Long, [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/). Source, hashes and adaptations are recorded in `packaging/NEOVIM-LOGO-NOTICE.txt`, included in every distribution. Zvim is an independent frontend, not an official Neovim application.
- `Cargo.lock` pins Rust dependencies. `scripts/licenses.py` copies supplied licence/notice files and generates `licenses/DEPENDENCIES.md` inside each package. That inventory includes build dependencies.

Declared package licences and copied notices are an inventory, not a blanket legal compatibility certification. Review the exact dependency tree when changing versions or preparing public releases. Do not use Zed branding or logos for this application.
