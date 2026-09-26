use gpui::*;
use std::ops::Range;

#[derive(Default)]
pub struct Search {
    pub text: String,
    pub selection: Range<usize>,
    pub marked: Option<Range<usize>>,
    pub bounds: Bounds<Pixels>,
    pub status: (isize, isize),
    pub select_first: bool,
}
impl Search {
    pub fn len(&self) -> usize {
        self.text.encode_utf16().count()
    }
    fn byte(&self, offset: usize) -> usize {
        let mut units = 0;
        for (index, ch) in self.text.char_indices() {
            if units >= offset {
                return index;
            }
            units += ch.len_utf16();
        }
        self.text.len()
    }
    pub fn slice(&self, range: Range<usize>) -> String {
        self.text[self.byte(range.start)..self.byte(range.end.max(range.start))].to_owned()
    }
    pub fn replace(&mut self, range: Option<Range<usize>>, text: &str) -> usize {
        let range = range
            .or(self.marked.take())
            .unwrap_or(self.selection.clone());
        let start = range.start.min(self.len());
        let bytes = self.byte(start)..self.byte(range.end.max(start));
        self.text.replace_range(bytes, text);
        let end = start + text.encode_utf16().count();
        self.selection = end..end;
        self.marked = None;
        start
    }
    pub fn backspace(&mut self) {
        if self.selection.is_empty() && self.selection.start > 0 {
            let byte = self.byte(self.selection.start);
            if let Some(ch) = self.text[..byte].chars().next_back() {
                self.selection.start -= ch.len_utf16();
            }
        }
        self.replace(None, "");
    }
    pub fn delete(&mut self) {
        if self.selection.is_empty() {
            self.selection.end += self.text[self.byte(self.selection.end)..]
                .chars()
                .next()
                .map_or(0, char::len_utf16);
        }
        self.replace(None, "");
    }
    pub fn move_cursor(&mut self, right: bool) {
        let pos = if !self.selection.is_empty() {
            if right {
                self.selection.end
            } else {
                self.selection.start
            }
        } else if right {
            self.selection.end
                + self.text[self.byte(self.selection.end)..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf16)
        } else {
            self.selection.start
                - self.text[..self.byte(self.selection.start)]
                    .chars()
                    .next_back()
                    .map_or(0, char::len_utf16)
        };
        self.selection = pos..pos;
        self.marked = None;
    }
}

#[cfg(test)]
mod tests {
    use super::Search;
    #[test]
    fn edits_unicode_selection_and_composition() {
        let mut s = Search::default();
        s.replace(None, "a😀中");
        assert_eq!(s.selection, 4..4);
        s.backspace();
        s.backspace();
        assert_eq!(s.text, "a");
        s.replace(Some(0..1), "かな");
        s.marked = Some(0..2);
        s.replace(None, "仮名");
        assert_eq!(s.text, "仮名");
        s.move_cursor(false);
        assert_eq!(s.selection, 1..1);
    }
}
