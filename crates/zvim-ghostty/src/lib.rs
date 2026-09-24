//! Native libghostty terminal component for GPUI.
//!
//! Rendering uses Ghostty's Metal embedded surface on macOS and a native
//! Wayland subsurface backed by its OpenGL renderer on Linux.
//!
//! Zvim carries a local bridge extension for live colour updates.

// Retain the upstream native library build and its linker metadata.
extern crate ghostty_upstream as _;
mod clipboard;
mod native;
mod terminal;

pub use clipboard::{ClipboardApproval, ClipboardApprovalCallback, ClipboardOperation};
pub use terminal::{TerminalColor, TerminalConfiguration, TerminalOptions, TerminalTheme};

/// Implementation details used by `bind_gpui!`; not a standalone public API.
#[doc(hidden)]
pub mod __private {
    pub use crate::native::{KeyAction, Modifiers, MouseButton, MouseState, NativeSurface};
    pub use crate::terminal::spawn_surface;
}
