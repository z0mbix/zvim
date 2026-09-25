//! Bounded second-chance cache with borrowed text lookups on the paint path.
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextStyle {
    pub foreground: u32,
    pub special: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub undercurl: bool,
    pub strikethrough: bool,
}
struct Entry<T> {
    value: T,
    referenced: bool,
}
pub struct TextCache<T> {
    styles: HashMap<TextStyle, HashMap<String, Entry<T>>>,
    clock: VecDeque<(TextStyle, String)>,
    capacity: usize,
}
impl<T> TextCache<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self {
            styles: HashMap::new(),
            clock: VecDeque::new(),
            capacity,
        }
    }
    pub fn clear(&mut self) {
        self.styles.clear();
        self.clock.clear();
    }
    pub fn get(&mut self, style: TextStyle, text: &str) -> Option<&T> {
        let entry = self.styles.get_mut(&style)?.get_mut(text)?;
        entry.referenced = true;
        Some(&entry.value)
    }
    pub fn insert(&mut self, style: TextStyle, text: &str, value: T) {
        if let Some(entry) = self
            .styles
            .get_mut(&style)
            .and_then(|texts| texts.get_mut(text))
        {
            entry.value = value;
            entry.referenced = true;
            return;
        }
        while self.clock.len() >= self.capacity {
            let (old_style, old_text) = self.clock.pop_front().unwrap();
            let texts = self.styles.get_mut(&old_style).unwrap();
            let entry = texts.get_mut(&old_text).unwrap();
            if entry.referenced {
                entry.referenced = false;
                self.clock.push_back((old_style, old_text));
            } else {
                texts.remove(&old_text);
                if texts.is_empty() {
                    self.styles.remove(&old_style);
                }
            }
        }
        self.styles.entry(style).or_default().insert(
            text.to_owned(),
            Entry {
                value,
                referenced: false,
            },
        );
        self.clock.push_back((style, text.to_owned()));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn style() -> TextStyle {
        TextStyle {
            foreground: 1,
            special: 2,
            bold: false,
            italic: false,
            underline: false,
            undercurl: false,
            strikethrough: false,
        }
    }
    #[test]
    fn unicode_styles_and_font_invalidation() {
        let mut cache = TextCache::new(3);
        let normal = style();
        let bold = TextStyle {
            bold: true,
            ..normal
        };
        cache.insert(normal, "👩‍💻", 1);
        cache.insert(bold, "👩‍💻", 2);
        cache.insert(normal, "é", 3);
        assert_eq!(cache.get(normal, "👩‍💻"), Some(&1));
        assert_eq!(cache.get(bold, "👩‍💻"), Some(&2));
        assert_eq!(cache.get(normal, "é"), Some(&3));
        cache.clear();
        assert!(cache.get(normal, "👩‍💻").is_none());
        assert!(cache.clock.is_empty());
    }
    #[test]
    fn pressure_evicts_cold_entries_without_flushing_hot_entries() {
        let mut cache = TextCache::new(3);
        let style = style();
        cache.insert(style, "hot", 1);
        cache.insert(style, "cold", 2);
        cache.insert(style, "cold2", 3);
        assert_eq!(cache.get(style, "hot"), Some(&1));
        cache.insert(style, "new", 4);
        assert!(cache.get(style, "cold").is_none());
        assert_eq!(cache.get(style, "hot"), Some(&1));
        for i in 0..100 {
            cache.get(style, "hot");
            cache.insert(style, &i.to_string(), i);
            assert_eq!(cache.clock.len(), 3);
            assert!(cache.styles.len() <= 3);
        }
        assert_eq!(cache.get(style, "hot"), Some(&1));
        cache.insert(style, "hot", 99);
        assert_eq!(cache.get(style, "hot"), Some(&99));
        assert_eq!(cache.clock.len(), 3);
    }
}
