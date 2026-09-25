/// Owns every shell, including inactive tabs. Selection never drops a shell.
pub struct TerminalTabs<T> {
    entries: Vec<(u64, T)>,
    active: usize,
    next_id: u64,
}

impl<T> Default for TerminalTabs<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            active: 0,
            next_id: 1,
        }
    }
}

impl<T> TerminalTabs<T> {
    pub fn active(&self) -> Option<&T> {
        self.entries.get(self.active).map(|(_, value)| value)
    }

    pub fn active_id(&self) -> Option<u64> {
        self.entries.get(self.active).map(|(id, _)| *id)
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn iter(&self) -> impl Iterator<Item = &(u64, T)> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn push_with_id(&mut self, id: u64, value: T) {
        self.entries.push((id, value));
        self.next_id = self.next_id.max(id + 1);
        self.active = self.entries.len() - 1;
    }

    pub fn push(&mut self, value: T) {
        self.entries.push((self.next_id, value));
        self.next_id += 1;
        self.active = self.entries.len() - 1;
    }

    pub fn select(&mut self, index: usize) -> bool {
        if index >= self.entries.len() {
            return false;
        }
        self.active = index;
        true
    }

    pub fn adjacent(&self, next: bool) -> usize {
        let len = self.entries.len();
        if len == 0 {
            return 0;
        }
        if next {
            (self.active + 1) % len
        } else {
            (self.active + len - 1) % len
        }
    }

    pub fn remove(&mut self, id: u64) -> Option<T> {
        let index = self.entries.iter().position(|(entry, _)| *entry == id)?;
        let (_, value) = self.entries.remove(index);
        if index < self.active {
            self.active -= 1;
        } else {
            self.active = self.active.min(self.entries.len().saturating_sub(1));
        }
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn switching_retains_shells_and_wraps() {
        let mut tabs = TerminalTabs::default();
        tabs.push("first");
        tabs.push("second");
        tabs.push("third");
        assert_eq!(tabs.adjacent(true), 0);
        assert!(tabs.select(0));
        assert_eq!(tabs.adjacent(false), 2);
        assert!(!tabs.select(9));
        assert_eq!(tabs.active(), Some(&"first"));
        assert_eq!(tabs.len(), 3);
    }
    #[test]
    fn closing_preserves_selection_and_ids() {
        let mut tabs = TerminalTabs::default();
        tabs.push("first");
        let first = tabs.active_id().unwrap();
        tabs.push("second");
        let second = tabs.active_id().unwrap();
        tabs.push("third");
        let third = tabs.active_id().unwrap();
        assert_eq!(tabs.remove(first), Some("first"));
        assert_eq!(tabs.active_id(), Some(third));
        tabs.select(0);
        tabs.remove(second);
        assert_eq!(tabs.active(), Some(&"third"));
        tabs.remove(third);
        assert!(tabs.is_empty());
        assert_eq!(tabs.active(), None);
        tabs.push("fourth");
        assert!(tabs.active_id().unwrap() > third);
        assert_eq!(tabs.remove(second), None);
    }
}
