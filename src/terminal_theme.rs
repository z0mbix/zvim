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

#[cfg(test)]
mod tests {
    use super::*;
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
