use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub width: f32,
    pub height: f32,
    pub x: Option<f32>,
    pub y: Option<f32>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            width: 1100.,
            height: 760.,
            x: None,
            y: None,
        }
    }
}
pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("dev", "zvim", "Zvim")
        .map(|d| d.config_dir().to_owned())
        .unwrap_or_else(std::env::temp_dir)
}
impl Settings {
    pub fn load() -> Self {
        std::fs::read(data_dir().join("window.json"))
            .ok()
            .and_then(|s| serde_json::from_slice(&s).ok())
            .unwrap_or_default()
    }
    pub fn save_to(&self, dir: &std::path::Path) -> anyhow::Result<()> {
        use std::io::Write;
        std::fs::create_dir_all(dir)?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        file.flush()?;
        file.persist(dir.join("window.json"))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}
impl Theme {
    pub fn is_dark(self, system_dark: bool) -> bool {
        match self {
            Self::System => system_dark,
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

/// App preferences are separate from geometry so closing an older window cannot overwrite them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub remember_window_geometry: bool,
    pub focus_follows_mouse: bool,
    pub theme: Theme,
    pub sync_editor_appearance: bool,
    pub terminal_follow_neovim: bool,
    pub terminal_toggle_key: String,
    pub terminal_maximize_key: String,
    pub terminal_focus_key: String,
    pub terminal_new_key: String,
    pub terminal_close_key: String,
    pub terminal_previous_key: String,
    pub terminal_next_key: String,
    pub terminal_split_right_key: String,
    pub terminal_split_down_key: String,
    pub terminal_pane_left_key: String,
    pub terminal_pane_right_key: String,
    #[serde(default)]
    pub terminal_keymap_version: u8,
    pub app_icon: String,
    pub cli_bin_directory: PathBuf,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            remember_window_geometry: true,
            focus_follows_mouse: false,
            theme: Theme::System,
            sync_editor_appearance: false,
            terminal_follow_neovim: true,
            terminal_toggle_key: "cmd-shift-.".into(),
            terminal_maximize_key: "cmd-shift-enter".into(),
            terminal_focus_key: "cmd-shift-,".into(),
            terminal_new_key: "cmd-n".into(),
            terminal_close_key: "cmd-w".into(),
            terminal_previous_key: "cmd-shift-[".into(),
            terminal_next_key: "cmd-shift-]".into(),
            terminal_split_right_key: "cmd-d".into(),
            terminal_split_down_key: "cmd-shift-d".into(),
            terminal_pane_left_key: "cmd-[".into(),
            terminal_pane_right_key: "cmd-]".into(),
            terminal_keymap_version: 1,
            app_icon: crate::icons::DEFAULT_ICON_ID.into(),
            cli_bin_directory: crate::cli_install::default_bin_directory(),
        }
    }
}
impl Preferences {
    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(&data_dir())
    }
    fn load_from(dir: &std::path::Path) -> anyhow::Result<Self> {
        match std::fs::read(dir.join("preferences.json")) {
            Ok(bytes) => {
                let mut preferences: Self = serde_json::from_slice(&bytes)?;
                preferences.migrate_terminal_keys();
                Ok(preferences)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }
    fn migrate_terminal_keys(&mut self) {
        if self.terminal_keymap_version == 0 {
            let defaults = Self::default();
            if self.terminal_toggle_key == "ctrl-`" {
                self.terminal_toggle_key = defaults.terminal_toggle_key;
            }
            if self.terminal_maximize_key == "ctrl-shift-`" {
                self.terminal_maximize_key = defaults.terminal_maximize_key;
            }
            self.terminal_keymap_version = 1;
        }
    }
    pub fn save(&self) -> anyhow::Result<()> {
        self.save_to(&data_dir())
    }
    fn save_to(&self, dir: &std::path::Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(dir)?;
        let temporary = dir.join(format!("preferences.{}.tmp", std::process::id()));
        std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(temporary, dir.join("preferences.json"))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalShortcut {
    Toggle,
    Focus,
    Maximize,
    New,
    Close,
    Previous,
    Next,
    SplitRight,
    SplitDown,
    PaneLeft,
    PaneRight,
}

impl TerminalShortcut {
    pub const ALL: [Self; 11] = [
        Self::Toggle,
        Self::Focus,
        Self::Maximize,
        Self::New,
        Self::Close,
        Self::Previous,
        Self::Next,
        Self::SplitRight,
        Self::SplitDown,
        Self::PaneLeft,
        Self::PaneRight,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Toggle => "Show / hide",
            Self::Focus => "Focus terminal / editor",
            Self::Maximize => "Maximise / restore",
            Self::New => "New terminal tab",
            Self::Close => "Close terminal tab",
            Self::Previous => "Previous terminal tab",
            Self::Next => "Next terminal tab",
            Self::SplitRight => "Split terminal right",
            Self::SplitDown => "Split terminal down",
            Self::PaneLeft => "Focus terminal pane left",
            Self::PaneRight => "Focus terminal pane right",
        }
    }
    pub fn key(self, p: &Preferences) -> &str {
        match self {
            Self::Toggle => &p.terminal_toggle_key,
            Self::Focus => &p.terminal_focus_key,
            Self::Maximize => &p.terminal_maximize_key,
            Self::New => &p.terminal_new_key,
            Self::Close => &p.terminal_close_key,
            Self::Previous => &p.terminal_previous_key,
            Self::Next => &p.terminal_next_key,
            Self::SplitRight => &p.terminal_split_right_key,
            Self::SplitDown => &p.terminal_split_down_key,
            Self::PaneLeft => &p.terminal_pane_left_key,
            Self::PaneRight => &p.terminal_pane_right_key,
        }
    }
    pub fn set(self, p: &mut Preferences, key: String) {
        *match self {
            Self::Toggle => &mut p.terminal_toggle_key,
            Self::Focus => &mut p.terminal_focus_key,
            Self::Maximize => &mut p.terminal_maximize_key,
            Self::New => &mut p.terminal_new_key,
            Self::Close => &mut p.terminal_close_key,
            Self::Previous => &mut p.terminal_previous_key,
            Self::Next => &mut p.terminal_next_key,
            Self::SplitRight => &mut p.terminal_split_right_key,
            Self::SplitDown => &mut p.terminal_split_down_key,
            Self::PaneLeft => &mut p.terminal_pane_left_key,
            Self::PaneRight => &mut p.terminal_pane_right_key,
        } = key;
    }
}

#[cfg(test)]
mod tests {
    use super::{Preferences, Theme};
    #[test]
    fn upgrades_old_defaults_but_preserves_custom_shortcuts() {
        let mut old: Preferences = serde_json::from_str(
            r#"{"terminal_toggle_key":"ctrl-`","terminal_maximize_key":"ctrl-shift-`"}"#,
        )
        .unwrap();
        old.migrate_terminal_keys();
        assert_eq!(
            old.terminal_toggle_key,
            Preferences::default().terminal_toggle_key
        );
        assert_eq!(
            old.terminal_maximize_key,
            Preferences::default().terminal_maximize_key
        );
        let mut custom: Preferences = serde_json::from_str(
            r#"{"terminal_toggle_key":"alt-t","terminal_maximize_key":"alt-m"}"#,
        )
        .unwrap();
        custom.migrate_terminal_keys();
        assert_eq!(custom.terminal_toggle_key, "alt-t");
        assert_eq!(custom.terminal_maximize_key, "alt-m");
        custom.terminal_toggle_key = "ctrl-`".into();
        custom.migrate_terminal_keys();
        assert_eq!(custom.terminal_toggle_key, "ctrl-`");
    }
    #[test]
    fn preferences_preserve_defaults_and_follow_system() {
        let p: Preferences = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert!(p.remember_window_geometry);
        assert_eq!(p.app_icon, crate::icons::DEFAULT_ICON_ID);
        assert!(!p.sync_editor_appearance);
        assert!(p.terminal_follow_neovim);
        assert!(!p.focus_follows_mouse);
        assert_eq!(p.terminal_toggle_key, "cmd-shift-.");
        assert_eq!(p.terminal_maximize_key, "cmd-shift-enter");
        assert_eq!(
            p.cli_bin_directory,
            crate::cli_install::default_bin_directory()
        );
        assert!(p.theme.is_dark(false));
        assert!(!Theme::Light.is_dark(true));
        assert!(Theme::System.is_dark(true));
        assert!(!Theme::System.is_dark(false));
    }
    #[test]
    fn unknown_icon_preserves_other_preferences_and_falls_back() {
        let p: Preferences = serde_json::from_str(
            r#"{"app_icon":"future-icon","theme":"light","remember_window_geometry":false}"#,
        )
        .unwrap();
        assert_eq!(
            crate::icons::resolve(&p.app_icon).id,
            crate::icons::DEFAULT_ICON_ID
        );
        assert_eq!(p.theme, Theme::Light);
        assert!(!p.remember_window_geometry);
        let roundtrip: Preferences =
            serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(roundtrip.app_icon, "future-icon");
    }
    #[test]
    fn preferences_roundtrip_independently_of_geometry() {
        let dir = std::env::temp_dir().join(format!("zvim-preferences-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("window.json"), "legacy geometry").unwrap();
        let p = Preferences {
            theme: Theme::Light,
            remember_window_geometry: false,
            focus_follows_mouse: true,
            sync_editor_appearance: true,
            terminal_follow_neovim: false,
            terminal_toggle_key: "alt-t".into(),
            terminal_maximize_key: "alt-m".into(),
            app_icon: crate::icons::DEFAULT_ICON_ID.into(),
            cli_bin_directory: dir.join("custom bin"),
            ..Preferences::default()
        };
        p.save_to(&dir).unwrap();
        assert_eq!(Preferences::load_from(&dir).unwrap(), p);
        Preferences::default().save_to(&dir).unwrap();
        assert_eq!(
            Preferences::load_from(&dir).unwrap(),
            Preferences::default()
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("window.json")).unwrap(),
            "legacy geometry"
        );
        std::fs::write(dir.join("preferences.json"), "broken").unwrap();
        assert!(Preferences::load_from(&dir).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
