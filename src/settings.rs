use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub fn save(&self) {
        let dir = data_dir();
        if std::fs::create_dir_all(&dir).is_ok() {
            let path = dir.join("window.json");
            let _ = std::fs::write(path, serde_json::to_vec_pretty(self).unwrap_or_default());
        }
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
    pub theme: Theme,
    pub sync_editor_appearance: bool,
    pub terminal_follow_neovim: bool,
    pub terminal_toggle_key: String,
    pub terminal_maximize_key: String,
    pub app_icon: String,
    pub cli_bin_directory: PathBuf,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            remember_window_geometry: true,
            theme: Theme::System,
            sync_editor_appearance: false,
            terminal_follow_neovim: true,
            terminal_toggle_key: "ctrl-`".into(),
            terminal_maximize_key: "ctrl-shift-`".into(),
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
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
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

#[cfg(test)]
mod tests {
    use super::{Preferences, Theme};
    #[test]
    fn preferences_preserve_defaults_and_follow_system() {
        let p: Preferences = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert!(p.remember_window_geometry);
        assert_eq!(p.app_icon, crate::icons::DEFAULT_ICON_ID);
        assert!(!p.sync_editor_appearance);
        assert!(p.terminal_follow_neovim);
        assert_eq!(p.terminal_toggle_key, "ctrl-`");
        assert_eq!(p.terminal_maximize_key, "ctrl-shift-`");
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
            sync_editor_appearance: true,
            terminal_follow_neovim: false,
            terminal_toggle_key: "alt-t".into(),
            terminal_maximize_key: "alt-m".into(),
            app_icon: crate::icons::DEFAULT_ICON_ID.into(),
            cli_bin_directory: dir.join("custom bin"),
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
