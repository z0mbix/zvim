use anyhow::{Result, bail};
use rmpv::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub highlight: u64,
}
impl Default for Cell {
    fn default() -> Self {
        Self {
            text: " ".into(),
            highlight: 0,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Highlight {
    pub foreground: Option<u32>,
    pub background: Option<u32>,
    pub special: Option<u32>,
    pub bold: bool,
    pub italic: bool,
    pub reverse: bool,
    pub underline: bool,
    pub undercurl: bool,
    pub strikethrough: bool,
    pub blend: u8,
}
#[derive(Clone, Debug)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
    pub shape: String,
    pub percentage: f32,
    pub attr: u64,
}
impl Default for Cursor {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            shape: "block".into(),
            percentage: 100.,
            attr: 0,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Grid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
    pub highlights: HashMap<u64, Highlight>,
    pub foreground: u32,
    pub background: u32,
    pub special: u32,
    pub cursor: Cursor,
    pub guifont: String,
    pub linespace: i64,
    pub title: String,
    pub busy: bool,
    pub mouse: bool,
    pub mode: String,
    modes: Vec<Value>,
}
impl Default for Grid {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            cells: vec![],
            highlights: HashMap::new(),
            foreground: 0xd4d4d4,
            background: 0x1e1e1e,
            special: 0xd4d4d4,
            cursor: Cursor::default(),
            guifont: String::new(),
            linespace: 0,
            title: "Zvim".into(),
            busy: false,
            mouse: false,
            mode: "normal".into(),
            modes: vec![],
        }
    }
}
pub fn field<'a>(v: &'a Value, name: &str) -> Option<&'a Value> {
    v.as_map()?
        .iter()
        .find(|(k, _)| k.as_str() == Some(name))
        .map(|(_, v)| v)
}
fn num(a: &[Value], i: usize) -> Result<usize> {
    a.get(i)
        .and_then(Value::as_u64)
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| anyhow::anyhow!("missing unsigned argument {i}"))
}
fn color(v: Option<&Value>) -> Option<u32> {
    v.and_then(Value::as_i64)
        .filter(|n| *n >= 0)
        .map(|n| n as u32 & 0xffffff)
}
fn flag(v: &Value, k: &str) -> bool {
    field(v, k).and_then(Value::as_bool).unwrap_or(false)
}
impl Grid {
    pub fn cell(&self, row: usize, col: usize) -> Option<&Cell> {
        if row < self.height && col < self.width {
            self.cells.get(row * self.width + col)
        } else {
            None
        }
    }
    pub fn colors(&self, h: &Highlight) -> (u32, u32, u32) {
        let fg = h.foreground.unwrap_or(self.foreground);
        let bg = h.background.unwrap_or(self.background);
        if h.reverse {
            (bg, fg, h.special.unwrap_or(self.special))
        } else {
            (fg, bg, h.special.unwrap_or(self.special))
        }
    }
    /// Apply ordered events; only snapshots returned at flush may be displayed.
    pub fn redraw(&mut self, events: &[Value]) -> Result<Vec<Self>> {
        let mut frames = vec![];
        for event in events {
            let event = event
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("redraw event is not an array"))?;
            let Some(name) = event.first().and_then(Value::as_str) else {
                bail!("missing redraw event name")
            };
            for args in &event[1..] {
                let a = args
                    .as_array()
                    .ok_or_else(|| anyhow::anyhow!("invalid {name} arguments"))?;
                match name {
                    "flush" => frames.push(self.clone()),
                    "grid_resize" => {
                        if num(a, 0)? != 1 {
                            continue;
                        }
                        let (w, h) = (num(a, 1)?, num(a, 2)?);
                        if w.checked_mul(h).is_none_or(|n| n > 4_000_000) {
                            bail!("grid too large")
                        }
                        let mut cells = vec![Cell::default(); w * h];
                        for row in 0..h.min(self.height) {
                            for col in 0..w.min(self.width) {
                                cells[row * w + col] = self.cells[row * self.width + col].clone();
                            }
                        }
                        self.width = w;
                        self.height = h;
                        self.cells = cells;
                    }
                    "grid_clear" => {
                        if num(a, 0)? == 1 {
                            self.cells.fill(Cell::default());
                        }
                    }
                    "grid_line" => {
                        if num(a, 0)? != 1 {
                            continue;
                        }
                        let row = num(a, 1)?;
                        let mut col = num(a, 2)?;
                        let mut hl = 0;
                        let cells = a
                            .get(3)
                            .and_then(Value::as_array)
                            .ok_or_else(|| anyhow::anyhow!("missing cells"))?;
                        for cell in cells {
                            let c = cell
                                .as_array()
                                .ok_or_else(|| anyhow::anyhow!("invalid cell"))?;
                            let text = c
                                .first()
                                .and_then(Value::as_str)
                                .ok_or_else(|| anyhow::anyhow!("invalid cell text"))?;
                            if let Some(id) = c.get(1) {
                                hl = id
                                    .as_u64()
                                    .ok_or_else(|| anyhow::anyhow!("invalid highlight"))?;
                            }
                            let repeat = if c.len() > 2 { num(c, 2)? } else { 1 };
                            if row >= self.height || repeat > self.width.saturating_sub(col) {
                                bail!("grid_line outside grid")
                            }
                            for _ in 0..repeat {
                                self.cells[row * self.width + col] = Cell {
                                    text: text.into(),
                                    highlight: hl,
                                };
                                col += 1;
                            }
                        }
                    }
                    "grid_scroll" => {
                        if num(a, 0)? != 1 {
                            continue;
                        }
                        let (top, bot, left, right) =
                            (num(a, 1)?, num(a, 2)?, num(a, 3)?, num(a, 4)?);
                        let rows = a.get(5).and_then(Value::as_i64).unwrap_or(0);
                        let cols = a.get(6).and_then(Value::as_i64).unwrap_or(0);
                        if top > bot || left > right || bot > self.height || right > self.width {
                            bail!("invalid scroll region")
                        }
                        let old = self.cells.clone();
                        for r in top..bot {
                            for c in left..right {
                                let sr = r as i64 + rows;
                                let sc = c as i64 + cols;
                                self.cells[r * self.width + c] = if sr >= top as i64
                                    && sr < bot as i64
                                    && sc >= left as i64
                                    && sc < right as i64
                                {
                                    old[sr as usize * self.width + sc as usize].clone()
                                } else {
                                    Cell::default()
                                };
                            }
                        }
                    }
                    "grid_cursor_goto" => {
                        if num(a, 0)? == 1 {
                            self.cursor.row = num(a, 1)?;
                            self.cursor.col = num(a, 2)?;
                        }
                    }
                    "default_colors_set" => {
                        if let Some(c) = color(a.first()) {
                            self.foreground = c
                        }
                        if let Some(c) = color(a.get(1)) {
                            self.background = c
                        }
                        if let Some(c) = color(a.get(2)) {
                            self.special = c
                        }
                    }
                    "hl_attr_define" => {
                        let id = num(a, 0)? as u64;
                        let v = a
                            .get(1)
                            .ok_or_else(|| anyhow::anyhow!("missing highlight attributes"))?;
                        self.highlights.insert(
                            id,
                            Highlight {
                                foreground: color(field(v, "foreground")),
                                background: color(field(v, "background")),
                                special: color(field(v, "special")),
                                bold: flag(v, "bold"),
                                italic: flag(v, "italic"),
                                reverse: flag(v, "reverse"),
                                underline: flag(v, "underline")
                                    || flag(v, "underdouble")
                                    || flag(v, "underdotted")
                                    || flag(v, "underdashed"),
                                undercurl: flag(v, "undercurl"),
                                strikethrough: flag(v, "strikethrough"),
                                blend: field(v, "blend")
                                    .and_then(Value::as_u64)
                                    .unwrap_or(0)
                                    .min(100) as u8,
                            },
                        );
                    }
                    "option_set" => match a.first().and_then(Value::as_str) {
                        Some("guifont") => {
                            self.guifont =
                                a.get(1).and_then(Value::as_str).unwrap_or_default().into()
                        }
                        Some("linespace") => {
                            self.linespace = a.get(1).and_then(Value::as_i64).unwrap_or(0)
                        }
                        _ => {}
                    },
                    "mode_info_set" => {
                        self.modes = a
                            .get(1)
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default()
                    }
                    "mode_change" => {
                        self.mode = a.first().and_then(Value::as_str).unwrap_or("normal").into();
                        if let Some(m) = self.modes.get(num(a, 1)?) {
                            self.cursor.shape = field(m, "cursor_shape")
                                .and_then(Value::as_str)
                                .unwrap_or("block")
                                .into();
                            self.cursor.percentage = field(m, "cell_percentage")
                                .and_then(Value::as_f64)
                                .unwrap_or(100.)
                                as f32;
                            self.cursor.attr =
                                field(m, "attr_id").and_then(Value::as_u64).unwrap_or(0);
                        }
                    }
                    "set_title" => {
                        self.title = a.first().and_then(Value::as_str).unwrap_or("Zvim").into()
                    }
                    "busy_start" => self.busy = true,
                    "busy_stop" => self.busy = false,
                    "mouse_on" => self.mouse = true,
                    "mouse_off" => self.mouse = false,
                    _ => {} // Protocol permits additional events and trailing arguments.
                }
            }
        }
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ev(name: &str, args: Vec<Value>) -> Value {
        Value::Array(vec![name.into(), Value::Array(args)])
    }
    fn resize() -> Value {
        ev("grid_resize", vec![1.into(), 4.into(), 3.into()])
    }
    fn line(row: u64, text: &str) -> Value {
        ev(
            "grid_line",
            vec![
                1.into(),
                row.into(),
                0.into(),
                Value::Array(
                    text.chars()
                        .enumerate()
                        .map(|(i, c)| {
                            Value::Array(if i == 0 {
                                vec![c.to_string().into(), 7.into()]
                            } else {
                                vec![c.to_string().into()]
                            })
                        })
                        .collect(),
                ),
            ],
        )
    }
    #[test]
    fn batching_highlights_and_forward_compatibility() {
        let mut g = Grid::default();
        assert!(g.redraw(&[resize(), line(0, "abcd")]).unwrap().is_empty());
        let frames = g
            .redraw(&[ev("future", vec![]), ev("flush", vec![])])
            .unwrap();
        assert_eq!(frames[0].cell(0, 3).unwrap().highlight, 7);
        g.redraw(&[line(0, "xxxx")]).unwrap();
        assert_eq!(frames[0].cell(0, 0).unwrap().text, "a");
    }
    #[test]
    fn scroll_both_directions_and_partial_region() {
        let mut g = Grid::default();
        g.redraw(&[resize(), line(0, "abcd"), line(1, "efgh"), line(2, "ijkl")])
            .unwrap();
        g.redraw(&[ev(
            "grid_scroll",
            vec![
                1.into(),
                0.into(),
                3.into(),
                1.into(),
                3.into(),
                1.into(),
                0.into(),
            ],
        )])
        .unwrap();
        assert_eq!(g.cell(0, 0).unwrap().text, "a");
        assert_eq!(g.cell(0, 1).unwrap().text, "f");
        assert_eq!(g.cell(2, 1).unwrap().text, " ");
        g.redraw(&[ev(
            "grid_scroll",
            vec![
                1.into(),
                0.into(),
                3.into(),
                0.into(),
                4.into(),
                (-1).into(),
                0.into(),
            ],
        )])
        .unwrap();
        assert_eq!(g.cell(1, 0).unwrap().text, "a");
        assert_eq!(g.cell(0, 0).unwrap().text, " ");
    }
    #[test]
    fn wide_cells_repeats_and_combining_marks() {
        let mut g = Grid::default();
        g.redraw(&[
            resize(),
            ev(
                "grid_line",
                vec![
                    1.into(),
                    0.into(),
                    0.into(),
                    Value::Array(vec![
                        Value::Array(vec!["界".into(), 2.into()]),
                        Value::Array(vec!["".into()]),
                        Value::Array(vec!["e\u{301}".into(), 3.into(), 2.into()]),
                    ]),
                ],
            ),
        ])
        .unwrap();
        assert_eq!(g.cell(0, 1).unwrap().text, "");
        assert_eq!(g.cell(0, 3).unwrap().text, "e\u{301}");
        assert_eq!(g.cell(0, 1).unwrap().highlight, 2);
    }
    #[test]
    fn malformed_update_is_error_not_panic() {
        let mut g = Grid::default();
        assert!(g.redraw(&[line(99, "a")]).is_err());
    }
}
