/// Neovim's RGB colours. Missing entries preserve the original Ghostty setting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalTheme(pub [Option<u32>; 22]);
impl TerminalTheme {
    pub fn from_value(value: &rmpv::Value) -> Option<Self> {
        let values = value.as_array()?;
        if values.len() != 22 {
            return None;
        }
        let mut colors = [None; 22];
        for (slot, value) in colors.iter_mut().zip(values) {
            if !value.is_nil() {
                *slot = Some(
                    u32::try_from(value.as_u64()?)
                        .ok()
                        .filter(|c| *c <= 0xffffff)?,
                );
            }
        }
        Some(Self(colors))
    }

    pub fn config(&self, foreground: u32, background: u32) -> String {
        use std::fmt::Write;
        let mut colors = self.0;
        colors[0] = Some(colors[0].unwrap_or(foreground));
        colors[1] = Some(colors[1].unwrap_or(background));
        // An unspecified Cursor uses reverse video in Neovim.
        if colors[2].is_none() && colors[3].is_none() {
            colors[2] = colors[1];
            colors[3] = colors[0];
        } else {
            colors[2] = colors[2].or(colors[0]);
            colors[3] = colors[3].or(colors[1]);
        }
        if colors[4].is_some() || colors[5].is_some() {
            colors[4] = colors[4].or(colors[0]);
            colors[5] = colors[5].or(colors[1]);
        }
        let mut config = String::new();
        for (index, name) in [
            "foreground",
            "background",
            "cursor-text",
            "cursor-color",
            "selection-foreground",
            "selection-background",
        ]
        .iter()
        .enumerate()
        {
            if let Some(color) = colors[index] {
                writeln!(config, "{name} = #{color:06x}").unwrap();
            }
        }
        for (index, color) in colors[6..].iter().enumerate() {
            if let Some(color) = color {
                writeln!(config, "palette = {index}=#{color:06x}").unwrap();
            }
        }
        config
    }
}

/// Avoid formatting Ghostty configuration on unchanged Neovim frames.
#[derive(Default)]
pub struct ThemeConfigCache {
    input: Option<(Option<TerminalTheme>, u32, u32)>,
    pub config: Option<std::sync::Arc<str>>,
}
impl ThemeConfigCache {
    pub fn update(
        &mut self,
        theme: Option<&TerminalTheme>,
        foreground: u32,
        background: u32,
    ) -> bool {
        let input = (theme.cloned(), foreground, background);
        if self.input.as_ref() == Some(&input) {
            return false;
        }
        self.input = Some(input);
        let config =
            theme.map(|theme| std::sync::Arc::<str>::from(theme.config(foreground, background)));
        if self.config == config {
            return false;
        }
        self.config = config;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_theme_tracks_changes_and_restores_user_configuration() {
        let mut cache = ThemeConfigCache::default();
        let theme = TerminalTheme([None; 22]);
        assert!(cache.update(Some(&theme), 1, 2));
        let previous = cache.config.clone().unwrap();
        assert!(!cache.update(Some(&theme), 1, 2));
        assert!(std::sync::Arc::ptr_eq(
            &previous,
            cache.config.as_ref().unwrap()
        ));
        assert!(cache.update(Some(&theme), 1, 3));
        assert!(cache.update(None, 1, 3));
        assert!(cache.config.is_none());
        assert!(!cache.update(None, 4, 5));
        assert!(cache.update(Some(&theme), 4, 5));
    }
    #[test]
    fn missing_colours_use_editor_defaults_and_leave_ansi_palette_alone() {
        let config = TerminalTheme([None; 22]).config(0xeeeeee, 0x111111);
        assert!(config.contains("foreground = #eeeeee\n"));
        assert!(config.contains("cursor-color = #eeeeee\n"));
        assert!(config.contains("cursor-text = #111111\n"));
        assert!(!config.contains("palette"));
        assert!(!config.contains("selection"));
    }
    #[test]
    fn rejects_invalid_palette_payloads() {
        assert!(TerminalTheme::from_value(&rmpv::Value::Array(vec![])).is_none());
        let mut colors = vec![rmpv::Value::Nil; 22];
        colors[6] = "#abc\ncommand = bad".into();
        assert!(TerminalTheme::from_value(&rmpv::Value::Array(colors.clone())).is_none());
        colors[6] = 0x1000000u32.into();
        assert!(TerminalTheme::from_value(&rmpv::Value::Array(colors)).is_none());
    }
}
