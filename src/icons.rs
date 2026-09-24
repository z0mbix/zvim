/// Stable IDs are persisted instead of file paths so preferences survive app moves and upgrades.
pub struct AppIcon {
    pub id: &'static str,
    pub label: &'static str,
    pub png: &'static [u8],
}

pub const DEFAULT_ICON_ID: &str = "neovim";
pub static APP_ICONS: &[AppIcon] = &[AppIcon {
    id: DEFAULT_ICON_ID,
    label: "Neovim",
    png: include_bytes!("../assets/icons/neovim-app.png"),
}];

/// Preserve an unknown saved ID, but display the default when this build lacks its artwork.
pub fn resolve(id: &str) -> &'static AppIcon {
    APP_ICONS
        .iter()
        .find(|icon| icon.id == id)
        .unwrap_or(&APP_ICONS[0])
}
