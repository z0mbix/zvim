/// Key names are GPUI names; printable text is committed through the OS input handler.
pub fn special_key(
    key: &str,
    ctrl: bool,
    alt: bool,
    shift: bool,
    super_key: bool,
) -> Option<String> {
    let name = match key {
        "enter" => "CR",
        "escape" => "Esc",
        "backspace" => "BS",
        "delete" => "Del",
        "tab" => "Tab",
        "space" if ctrl || alt || super_key => "Space",
        "up" => "Up",
        "down" => "Down",
        "left" => "Left",
        "right" => "Right",
        "home" => "Home",
        "end" => "End",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        k if k.starts_with('f') && k[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)) => k,
        k if ctrl || alt || super_key => k,
        _ => return None,
    };
    let name = if name == "<" { "LT" } else { name };
    Some(format!(
        "<{}{}{}{}{}>",
        if ctrl { "C-" } else { "" },
        if alt { "M-" } else { "" },
        if super_key { "D-" } else { "" },
        if shift { "S-" } else { "" },
        name
    ))
}
pub fn literal(text: &str) -> String {
    text.replace('<', "<LT>")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controls_and_literals() {
        assert_eq!(
            special_key("[", true, false, false, false).as_deref(),
            Some("<C-[>")
        );
        assert_eq!(
            special_key("tab", false, false, true, false).as_deref(),
            Some("<S-Tab>")
        );
        assert_eq!(special_key("é", false, false, false, false), None);
        assert_eq!(literal("<Esc>"), "<LT>Esc>");
    }
}
