#[cfg(not(any(target_os = "macos", target_os = "linux")))]
compile_error!("Zvim supports macOS and Linux only.");

pub mod cli;
pub mod grid;
pub mod icons;
pub mod input;
pub mod session;
pub mod settings;

pub mod startup;

pub mod terminal_theme;

pub mod cli_install;

pub mod terminal_tabs;
