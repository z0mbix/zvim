use gpui::{prelude::*, *};
use std::{collections::HashMap, ops::Range, path::PathBuf};
use zvim::{
    grid::{Grid, Highlight},
    input,
    session::{Event, Launch, Session},
    settings::Settings,
};

actions!(zvim, [Open, Close, Paste, NewWindow, Quit, OpenSettings]);

pub struct Editor {
    session: Option<Session>,
    grid: Grid,
    focus: FocusHandle,
    error: Option<String>,
    exited: bool,
    bounds: Bounds<Pixels>,
    cell_width: f32,
    cell_height: f32,
    font: Font,
    font_size: f32,
    font_spec: String,
    cache: HashMap<String, ShapedLine>,
    requested: (usize, usize),
    preedit: String,
    selection: Range<usize>,
    scroll: Point<f32>,
    started: std::time::Instant,
    first_paint: bool,
    font_resolution_ms: u128,
    applied_background: Option<bool>,
    appearance_ready: bool,
}
impl Editor {
    pub fn new(launch: Launch, window: &mut Window, cx: &mut Context<Self>) -> Self {
        zvim::startup::mark("editor_create");
        let started = std::time::Instant::now();
        let focus = cx.focus_handle();
        window.focus(&focus);
        let session = Session::spawn(&launch);
        zvim::startup::mark("editor_spawned");
        let error = session.as_ref().err().map(|e| format!("{e:#}"));
        let session = session.ok();
        if let Some(s) = &session {
            let events = s.events.clone();
            let rpc = s.rpc.clone();
            let (init_tx, init_rx) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let _ = init_tx.send_blocking(
                    zvim::session::initialize(&rpc, 100, 35).map_err(|e| format!("{e:#}")),
                );
            });
            cx.spawn_in(window, async move |view, cx| {
                if let Ok(Err(e)) = init_rx.recv().await {
                    let _ = view.update(cx, |v, cx| {
                        v.error = Some(e);
                        cx.notify();
                    });
                }
                while let Ok(event) = events.recv().await {
                    if view
                        .update_in(cx, |v, w, cx| v.event(event, w, cx))
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
        }
        cx.observe_global_in::<crate::preferences::AppPreferences>(window, |v, w, cx| {
            v.apply_appearance(w, cx)
        })
        .detach();
        cx.observe_window_appearance(window, |v, w, cx| v.apply_appearance(w, cx))
            .detach();
        cx.observe_window_activation(window, |_, _, cx| crate::preferences::refresh(cx))
            .detach();
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            weak.update(cx, |v, _| {
                v.save(window);
                if v.exited || v.session.is_none() {
                    true
                } else {
                    v.close();
                    false
                }
            })
            .unwrap_or(true)
        });
        Self {
            session,
            grid: Grid::default(),
            focus,
            error,
            exited: false,
            bounds: Bounds::default(),
            cell_width: 9.,
            cell_height: 22.,
            font: font(default_font()),
            font_size: 16.,
            font_spec: String::new(),
            cache: HashMap::new(),
            requested: (100, 35),
            preedit: String::new(),
            selection: 0..0,
            scroll: point(0., 0.),
            started,
            first_paint: true,
            font_resolution_ms: 0,
            applied_background: None,
            appearance_ready: false,
        }
    }
    fn apply_appearance(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !self.appearance_ready {
            return;
        }
        let p = &cx.global::<crate::preferences::AppPreferences>().0;
        let desired = p
            .sync_editor_appearance
            .then(|| crate::preferences::dark(window, cx));
        if desired != self.applied_background {
            if let Some(s) = &self.session {
                s.set_appearance(desired);
            }
            self.applied_background = desired;
        }
    }
    fn save(&self, window: &Window) {
        if !zvim::settings::Preferences::load().is_ok_and(|p| p.remember_window_geometry) {
            return;
        }
        let bounds = window.window_bounds().get_bounds();
        let s = bounds.size;
        Settings {
            width: f32::from(s.width),
            height: f32::from(s.height),
            x: Some(f32::from(bounds.origin.x)),
            y: Some(f32::from(bounds.origin.y)),
        }
        .save();
    }
    fn close(&self) {
        if let Some(s) = &self.session {
            s.close();
        }
    }
    fn event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            Event::Frame => {
                zvim::startup::mark("frame_received");
                if let Some(g) = self.session.as_ref().and_then(Session::take_frame) {
                    self.grid = g;
                    self.appearance_ready = true;
                    self.apply_appearance(window, cx);

                    let title = self.grid.title.trim();
                    window.set_window_title(if title.is_empty() || title == "Zvim" {
                        "Zvim"
                    } else {
                        title
                    });
                    cx.notify();
                }
            }
            Event::ClipboardCopy(text) => cx.write_to_clipboard(ClipboardItem::new_string(text)),
            Event::ClipboardPaste(id) => {
                let text = cx
                    .read_from_clipboard()
                    .and_then(|c| c.text())
                    .unwrap_or_default();
                let linewise = text.ends_with('\n');
                let text = if linewise {
                    &text[..text.len() - 1]
                } else {
                    &text
                };
                let lines = rmpv::Value::Array(text.split('\n').map(|s| s.into()).collect());
                if let Some(s) = &self.session {
                    s.rpc.reply(
                        id,
                        rmpv::Value::Array(vec![lines, if linewise { "V" } else { "v" }.into()]),
                    );
                }
            }
            Event::Error(e) => {
                self.error = Some(e);
                cx.notify();
            }
            Event::Exited(success) => {
                self.exited = true;
                self.save(window);
                if success {
                    window.remove_window();
                } else {
                    self.error=Some("Neovim exited unexpectedly. See neovim.log in the Zvim configuration directory.".into());
                    cx.notify();
                }
            }
        }
    }
    fn open(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
        let result = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: true,
            prompt: Some("Open in Neovim".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            if let Ok(Ok(Some(paths))) = result.await {
                let _ = view.update(cx, |v, _| {
                    if let Some(s) = &v.session {
                        s.open_files(&paths);
                    }
                });
            }
        })
        .detach();
    }
    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text())
            && let Some(s) = &self.session
        {
            s.paste(text);
        }
    }
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" && !self.exited {
            self.error = None;
            cx.notify();
        }
        let k = &event.keystroke;
        let m = k.modifiers;
        // Only desktop shortcuts occupy Command on macOS; Ctrl keys remain Neovim's.
        if cfg!(target_os = "macos") && m.platform && !m.control && !m.alt {
            match k.key.as_str() {
                "v" => {
                    self.paste(&Paste, window, cx);
                    cx.stop_propagation();
                    return;
                }
                "o" => {
                    self.open(&Open, window, cx);
                    cx.stop_propagation();
                    return;
                }
                "w" => {
                    self.close();
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }
        if !self.preedit.is_empty() {
            return;
        }
        // Option-generated printable text on macOS is delivered by the native input method.
        let alt = m.alt
            && !(cfg!(target_os = "macos") && k.key_char.is_some() && !m.control && !m.platform);
        if let Some(keys) = input::special_key(&k.key, m.control, alt, m.shift, m.platform) {
            if let Some(s) = &self.session {
                s.input(keys);
            }
            cx.stop_propagation();
        }
    }
    fn mouse(&self, button: &str, action: &str, pos: Point<Pixels>, mods: Modifiers) {
        if !self.grid.mouse {
            return;
        }
        let row = ((f32::from(pos.y - self.bounds.origin.y) / self.cell_height)
            .floor()
            .max(0.) as usize)
            .min(self.grid.height.saturating_sub(1));
        let col = ((f32::from(pos.x - self.bounds.origin.x) / self.cell_width)
            .floor()
            .max(0.) as usize)
            .min(self.grid.width.saturating_sub(1));
        let mods = format!(
            "{}{}{}{}",
            if mods.control { "C" } else { "" },
            if mods.alt { "A" } else { "" },
            if mods.shift { "S" } else { "" },
            if mods.platform { "D" } else { "" }
        );
        if let Some(s) = &self.session {
            s.rpc.send(
                "nvim_input_mouse",
                vec![
                    button.into(),
                    action.into(),
                    mods.into(),
                    0.into(),
                    (row as u64).into(),
                    (col as u64).into(),
                ],
            );
        }
    }
    fn wheel(&mut self, e: &ScrollWheelEvent, _: &mut Window, _: &mut Context<Self>) {
        let d = e.delta.pixel_delta(px(self.cell_height));
        self.scroll.y += f32::from(d.y) / self.cell_height;
        self.scroll.x += f32::from(d.x) / self.cell_width;
        for horizontal in [false, true] {
            let delta = if horizontal {
                &mut self.scroll.x
            } else {
                &mut self.scroll.y
            };
            let n = delta.trunc() as i32;
            *delta -= n as f32;
            for _ in 0..n.unsigned_abs().min(50) {
                self.mouse(
                    "wheel",
                    if horizontal {
                        if n > 0 { "left" } else { "right" }
                    } else if n > 0 {
                        "up"
                    } else {
                        "down"
                    },
                    e.position,
                    e.modifiers,
                );
            }
        }
    }
    fn layout_grid(&mut self, bounds: Bounds<Pixels>, window: &mut Window) {
        self.bounds = bounds;
        let spec = format!("{}:{}", self.grid.guifont, self.grid.linespace);
        if self.font_spec != spec {
            let font_started = std::time::Instant::now();
            let text_system = window.text_system();
            let (font, size) = resolve_editor_font(&self.grid.guifont, text_system);
            self.font = font;
            self.font_size = size;
            self.font_resolution_ms += font_started.elapsed().as_millis();
            self.font_spec = spec;
            self.cache.clear();
        }
        let run = TextRun {
            len: 1,
            font: self.font.clone(),
            color: rgb(self.grid.foreground).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let sample = window
            .text_system()
            .shape_line("M".into(), px(self.font_size), &[run], None);
        self.cell_width = f32::from(sample.width).max(1.);
        self.cell_height = (self.font_size * 1.4 + self.grid.linespace as f32).max(self.font_size);
        let cols = (f32::from(bounds.size.width) / self.cell_width)
            .floor()
            .max(2.) as usize;
        let rows = (f32::from(bounds.size.height) / self.cell_height)
            .floor()
            .max(2.) as usize;
        if self.requested != (cols, rows) && self.grid.width > 0 {
            self.requested = (cols, rows);
            if let Some(s) = &self.session {
                s.resize(cols, rows);
            }
        }
    }
    fn cell_bounds(&self, row: usize, col: usize, width: usize) -> Bounds<Pixels> {
        Bounds::new(
            self.bounds.origin
                + point(
                    px(col as f32 * self.cell_width),
                    px(row as f32 * self.cell_height),
                ),
            size(px(width as f32 * self.cell_width), px(self.cell_height)),
        )
    }
    fn text(
        &mut self,
        text: &str,
        hl: &Highlight,
        colors: (u32, u32),
        origin: Point<Pixels>,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let (fg, special) = colors;
        let key = format!(
            "{text}\0{fg}:{special}:{}:{}:{}:{}:{}",
            hl.bold, hl.italic, hl.underline, hl.undercurl, hl.strikethrough
        );
        if !self.cache.contains_key(&key) {
            if self.cache.len() > 8192 {
                self.cache.clear();
            }
            let mut font = self.font.clone();
            if hl.bold {
                font.weight = FontWeight::BOLD;
            }
            if hl.italic {
                font.style = FontStyle::Italic;
            }
            let run = TextRun {
                len: text.len(),
                font,
                color: rgb(fg).into(),
                background_color: None,
                underline: if hl.underline || hl.undercurl {
                    Some(UnderlineStyle {
                        thickness: px(1.),
                        color: Some(rgb(special).into()),
                        wavy: hl.undercurl,
                    })
                } else {
                    None
                },
                strikethrough: if hl.strikethrough {
                    Some(StrikethroughStyle {
                        thickness: px(1.),
                        color: Some(rgb(fg).into()),
                    })
                } else {
                    None
                },
            };
            let line = window.text_system().shape_line(
                text.to_owned().into(),
                px(self.font_size),
                &[run],
                None,
            );
            self.cache.insert(key.clone(), line);
        }
        if let Some(line) = self.cache.get(&key) {
            paint_cell_line(line, hl, colors, origin, px(self.cell_height), window);
        }
    }
    fn paint(&mut self, window: &mut Window, cx: &mut App) {
        let trace_first = self.first_paint && self.grid.width > 0;
        zvim::startup::mark(if trace_first {
            "first_grid_paint_begin"
        } else {
            "grid_paint_begin"
        });
        if self.first_paint && self.grid.width > 0 {
            self.first_paint = false;
            let timing = format!(
                "first_grid_paint_ms={}\nfont_resolution_ms={}\n",
                self.started.elapsed().as_millis(),
                self.font_resolution_ms
            );
            eprint!("{timing}");
            let dir = zvim::settings::data_dir();
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join("startup.log"), timing);
        }
        window.paint_quad(fill(self.bounds, rgb(self.grid.background)));
        let max_rows = self
            .grid
            .height
            .min((f32::from(self.bounds.size.height) / self.cell_height).ceil() as usize);
        let max_cols = self
            .grid
            .width
            .min((f32::from(self.bounds.size.width) / self.cell_width).ceil() as usize);
        // A layer per cell degenerates GPUI's bounds-ordering tree on a regular
        // terminal grid. Use separate, ordered grid-wide background and glyph layers.
        window.paint_layer(self.bounds, |window| {
            // Background pass precedes glyphs so wide glyphs are never covered by continuation cells.
            for r in 0..max_rows {
                for c in 0..max_cols {
                    let cell = &self.grid.cells[r * self.grid.width + c];
                    let h = self
                        .grid
                        .highlights
                        .get(&cell.highlight)
                        .cloned()
                        .unwrap_or_default();
                    let (_, bg, _) = self.grid.colors(&h);
                    if bg != self.grid.background {
                        window.paint_quad(fill(self.cell_bounds(r, c, 1), rgb(bg)));
                    }
                }
            }
        });
        window.paint_layer(self.bounds, |window| {
            for r in 0..max_rows {
                for c in 0..max_cols {
                    let cell = self.grid.cells[r * self.grid.width + c].clone();
                    if cell.text.is_empty() {
                        continue;
                    }
                    let h = self
                        .grid
                        .highlights
                        .get(&cell.highlight)
                        .cloned()
                        .unwrap_or_default();
                    if cell.text == " " && !h.underline && !h.undercurl && !h.strikethrough {
                        continue;
                    }
                    let (fg, _, sp) = self.grid.colors(&h);
                    self.text(
                        &cell.text,
                        &h,
                        (fg, sp),
                        self.cell_bounds(r, c, 1).origin,
                        window,
                        cx,
                    );
                }
            }
        });
        if !self.grid.busy && self.grid.cursor.row < max_rows && self.grid.cursor.col < max_cols {
            let cursor = self.grid.cursor.clone();
            let cell = self
                .grid
                .cell(cursor.row, cursor.col)
                .cloned()
                .unwrap_or_default();
            let width = if self
                .grid
                .cell(cursor.row, cursor.col + 1)
                .is_some_and(|c| c.text.is_empty())
            {
                2
            } else {
                1
            };
            let mut rect = self.cell_bounds(cursor.row, cursor.col, width);
            let h = self
                .grid
                .highlights
                .get(&cursor.attr)
                .cloned()
                .unwrap_or_default();
            let (fg, bg, sp) = if cursor.attr == 0 {
                let cell_h = self
                    .grid
                    .highlights
                    .get(&cell.highlight)
                    .cloned()
                    .unwrap_or_default();
                let (fg, bg, sp) = self.grid.colors(&cell_h);
                (bg, fg, sp)
            } else {
                self.grid.colors(&h)
            };
            let ratio = (cursor.percentage / 100.).clamp(0.05, 1.);
            if cursor.shape == "vertical" {
                rect.size.width = px(self.cell_width * ratio);
            } else if cursor.shape == "horizontal" {
                rect.origin.y += px(self.cell_height * (1. - ratio));
                rect.size.height = px(self.cell_height * ratio);
            }
            if self.focus.is_focused(window) {
                window.paint_quad(fill(rect, rgb(bg)));
                if cursor.shape == "block" {
                    self.text(&cell.text, &h, (fg, sp), rect.origin, window, cx);
                }
            }
        }
        if !self.preedit.is_empty() {
            let origin = self
                .cell_bounds(self.grid.cursor.row, self.grid.cursor.col, 1)
                .origin;
            let text = self.preedit.clone();
            window.paint_quad(fill(
                Bounds::new(
                    origin,
                    size(
                        px(self.cell_width * text.chars().count().max(1) as f32),
                        px(self.cell_height),
                    ),
                ),
                rgb(self.grid.background),
            ));
            self.text(
                &text,
                &Highlight {
                    underline: true,
                    ..Default::default()
                },
                (self.grid.foreground, self.grid.foreground),
                origin,
                window,
                cx,
            );
        }
        zvim::startup::mark(if trace_first {
            "first_grid_paint_end"
        } else {
            "grid_paint_end"
        });
    }
}
/// Paint cached shaped cells inside an already ordered layer, preserving fallback
/// runs, combining glyphs, colour emoji, and text decorations without a layer per cell.
fn paint_cell_line(
    line: &ShapedLine,
    highlight: &Highlight,
    colors: (u32, u32),
    origin: Point<Pixels>,
    height: Pixels,
    window: &mut Window,
) {
    let baseline = (height - line.ascent - line.descent) / 2. + line.ascent;
    for run in &line.runs {
        for glyph in &run.glyphs {
            let position = origin + glyph.position + point(px(0.), baseline);
            if glyph.is_emoji {
                let _ = window.paint_emoji(position, run.font_id, glyph.id, line.font_size);
            } else {
                let _ = window.paint_glyph(
                    position,
                    run.font_id,
                    glyph.id,
                    line.font_size,
                    rgb(colors.0).into(),
                );
            }
        }
    }
    if highlight.underline || highlight.undercurl {
        window.paint_underline(
            origin + point(px(0.), baseline + line.descent * 0.618),
            line.width,
            &UnderlineStyle {
                thickness: px(1.),
                color: Some(rgb(colors.1).into()),
                wavy: highlight.undercurl,
            },
        );
    }
    if highlight.strikethrough {
        window.paint_strikethrough(
            origin + point(px(0.), (line.ascent * 0.5 + baseline) * 0.5),
            line.width,
            &StrikethroughStyle {
                thickness: px(1.),
                color: Some(rgb(colors.0).into()),
            },
        );
    }
}
fn default_font() -> &'static str {
    if cfg!(target_os = "macos") {
        "Menlo"
    } else if cfg!(windows) {
        "Consolas"
    } else {
        "monospace"
    }
}
fn resolve_editor_font(spec: &str, text_system: &WindowTextSystem) -> (Font, f32) {
    // Probe only the requested families. Enumerating the entire OS font collection
    // here blocks the first frame, even when guifont is empty.
    let available = spec
        .split(',')
        .filter_map(|entry| {
            let family = entry.split(':').next()?.replace('_', " ");
            if family.is_empty() {
                return None;
            }
            let id = text_system.resolve_font(&font(family.clone()));
            text_system
                .get_font_for_id(id)
                .filter(|resolved| resolved.family.eq_ignore_ascii_case(&family))
                .map(|_| family)
        })
        .collect::<Vec<_>>();
    parse_font(spec, &available)
}
fn parse_font(spec: &str, available: &[String]) -> (Font, f32) {
    let choices: Vec<_> = spec
        .split(',')
        .map(|entry| {
            let mut parts = entry.split(':');
            let family = parts.next().unwrap_or_default().replace('_', " ");
            let options: Vec<_> = parts.collect();
            (family, options)
        })
        .collect();
    let selected = choices
        .iter()
        .find(|(name, _)| available.iter().any(|f| f.eq_ignore_ascii_case(name)));
    let (family, options) = selected
        .map(|(f, o)| (f.as_str(), o.as_slice()))
        .unwrap_or((default_font(), &[]));
    let size = options
        .iter()
        .find_map(|s| s.strip_prefix('h')?.parse::<f32>().ok())
        .filter(|s| s.is_finite())
        .unwrap_or(16.)
        .clamp(6., 96.);
    let mut font = font(family.to_owned());
    if options.contains(&"b") {
        font.weight = FontWeight::BOLD;
    }
    if options.contains(&"i") {
        font.style = FontStyle::Italic;
    }
    let mut fallbacks = choices
        .iter()
        .filter(|(f, _)| f != family && available.iter().any(|a| a.eq_ignore_ascii_case(f)))
        .map(|(f, _)| f.clone())
        .collect::<Vec<_>>();
    if let Some(symbols) = available
        .iter()
        .find(|f| f.contains("Symbols Nerd Font"))
        .or_else(|| {
            available
                .iter()
                .find(|f| f.contains("Nerd Font") && !f.contains("Propo"))
        })
    {
        fallbacks.push(symbols.clone());
    }
    if !fallbacks.is_empty() {
        font.fallbacks = Some(FontFallbacks::from_fonts(fallbacks));
    }
    (font, size)
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div()
            .size_full()
            .overflow_hidden()
            .track_focus(&self.focus)
            .key_context("Zvim")
            .on_key_down(cx.listener(Self::key))
            .on_action(cx.listener(Self::open))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(|v, _: &Close, w, cx| {
                if v.exited || v.session.is_none() {
                    w.remove_window();
                } else {
                    v.close();
                }
                cx.notify();
            }))
            .on_scroll_wheel(cx.listener(Self::wheel))
            .on_mouse_move(cx.listener(|v, e: &MouseMoveEvent, _, _| {
                if let Some(b) = e.pressed_button {
                    v.mouse(button_name(b), "drag", e.position, e.modifiers);
                }
            }))
            .on_drop(cx.listener(|v, p: &ExternalPaths, _, _| {
                if let Some(s) = &v.session {
                    s.open_files(p.paths());
                }
            }));
        for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            root = root
                .on_mouse_down(
                    button,
                    cx.listener(move |v, e: &MouseDownEvent, w, _| {
                        w.focus(&v.focus);
                        v.mouse(button_name(button), "press", e.position, e.modifiers);
                    }),
                )
                .on_mouse_up(
                    button,
                    cx.listener(move |v, e: &MouseUpEvent, _, _| {
                        v.mouse(button_name(button), "release", e.position, e.modifiers)
                    }),
                )
                .on_mouse_up_out(
                    button,
                    cx.listener(move |v, e: &MouseUpEvent, _, _| {
                        v.mouse(button_name(button), "release", e.position, e.modifiers)
                    }),
                );
        }
        root.child(GridElement {
            editor: cx.entity(),
        })
        .when_some(self.error.clone(), |el, error| {
            el.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .p_4()
                    .bg(rgb(0x52252c))
                    .text_color(rgb(0xffffff))
                    .child(error),
            )
        })
    }
}
fn button_name(b: MouseButton) -> &'static str {
    match b {
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
        _ => "left",
    }
}
struct GridElement {
    editor: Entity<Editor>,
}
impl IntoElement for GridElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for GridElement {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        w: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (w.request_layout(style, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        w: &mut Window,
        cx: &mut App,
    ) {
        self.editor.update(cx, |v, _| v.layout_grid(bounds, w));
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        w: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.editor.read(cx).focus.clone();
        w.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        self.editor.update(cx, |v, cx| v.paint(w, cx));
    }
}
impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        r: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let units = self.preedit.encode_utf16().collect::<Vec<_>>();
        let start = r.start.min(units.len());
        let end = r.end.min(units.len()).max(start);
        *actual = Some(start..end);
        Some(String::from_utf16_lossy(&units[start..end]))
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.selection.clone(),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        if self.preedit.is_empty() {
            None
        } else {
            Some(0..self.preedit.encode_utf16().count())
        }
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.preedit.clear();
        self.selection = 0..0;
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preedit.clear();
        self.selection = 0..0;
        if let Some(s) = &self.session {
            s.input(input::literal(text));
        }
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut units = self.preedit.encode_utf16().collect::<Vec<_>>();
        let r = range.unwrap_or(0..units.len());
        let start = r.start.min(units.len());
        let end = r.end.min(units.len()).max(start);
        units.splice(start..end, text.encode_utf16());
        self.preedit = String::from_utf16_lossy(&units);
        let n = self.preedit.encode_utf16().count();
        self.selection = selected
            .map(|r| (start + r.start).min(n)..(start + r.end).min(n))
            .unwrap_or(n..n);
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(self.cell_bounds(self.grid.cursor.row, self.grid.cursor.col, 1))
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}

pub fn open_window(launch: Launch, cx: &mut App) {
    zvim::startup::mark("window_open_begin");
    crate::preferences::refresh(cx);
    let settings = if cx
        .global::<crate::preferences::AppPreferences>()
        .0
        .remember_window_geometry
    {
        Settings::load()
    } else {
        Settings::default()
    };
    let mut bounds = Bounds::centered(
        None,
        size(
            px(settings.width.clamp(400., 4000.)),
            px(settings.height.clamp(300., 2400.)),
        ),
        cx,
    );
    if let (Some(x), Some(y)) = (settings.x, settings.y) {
        let candidate = Bounds::new(point(px(x), px(y)), bounds.size);
        if x.is_finite()
            && y.is_finite()
            && cx
                .displays()
                .iter()
                .any(|d| d.bounds().contains(&candidate.origin))
        {
            bounds = candidate;
        }
    }
    if let Err(e) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            app_id: Some("zvim".into()),
            window_min_size: Some(size(px(400.), px(300.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Zvim".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |w, cx| {
            zvim::startup::mark("native_window_ready");
            cx.new(|cx| Editor::new(launch, w, cx))
        },
    ) {
        eprintln!("Cannot open Zvim window: {e:#}");
    }
    zvim::startup::mark("window_open_end");
    cx.activate(true);
}
pub fn open_paths(paths: Vec<PathBuf>, cx: &mut App) {
    if let Some(window) = cx
        .windows()
        .into_iter()
        .find(|h| h.downcast::<Editor>().is_some())
    {
        let _ = window.update(cx, |root, _, cx| {
            if let Ok(editor) = root.downcast::<Editor>() {
                editor.update(cx, |v, _| {
                    if let Some(s) = &v.session {
                        s.open_files(&paths);
                    }
                });
            }
        });
    } else {
        open_window(
            Launch {
                files: paths,
                ..Default::default()
            },
            cx,
        );
    }
}

/// Request each session to quit directly; dispatching a focused action can miss inactive windows.
pub fn request_close_all(cx: &mut App) {
    for handle in cx.windows() {
        let _ = handle.update(cx, |root, window, cx| {
            if root
                .clone()
                .downcast::<crate::preferences::PreferencesView>()
                .is_ok()
            {
                window.remove_window();
            } else if let Ok(editor) = root.downcast::<Editor>() {
                editor.update(cx, |view, _| {
                    view.save(window);
                    if view.exited || view.session.is_none() {
                        window.remove_window();
                    } else {
                        view.close();
                    }
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{FontStyle, FontWeight, parse_font};
    #[test]
    fn chooses_installed_font_in_neovim_preference_list() {
        let (font, size) = parse_font("Missing Font,Menlo:h18,monospace", &["Menlo".into()]);
        assert_eq!(font.family.as_ref(), "Menlo");
        assert_eq!(size, 18.);
    }
    #[test]
    fn applies_font_style_and_rejects_nonfinite_size() {
        let (font, size) = parse_font("Example_Mono:hNaN:b:i", &["Example Mono".into()]);
        assert_eq!(font.family.as_ref(), "Example Mono");
        assert_eq!(size, 16.);
        assert_eq!(font.weight, FontWeight::BOLD);
        assert_eq!(font.style, FontStyle::Italic);
    }
}
