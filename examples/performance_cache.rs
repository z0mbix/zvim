//! Isolated release benchmark of the old and current cache algorithms.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::HashMap,
    hint::black_box,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
use zvim::{
    terminal_theme::{TerminalTheme, ThemeConfigCache},
    text_cache::{TextCache, TextStyle},
};
struct Allocator;
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn style(i: usize) -> TextStyle {
    TextStyle {
        foreground: (i % 16) as u32,
        special: 0xff00,
        bold: i % 2 == 1,
        italic: false,
        underline: false,
        undercurl: false,
        strikethrough: false,
    }
}
fn old_key(text: &str, s: TextStyle) -> String {
    format!(
        "{text}\0{}:{}:{}:{}:{}:{}:{}",
        s.foreground, s.special, s.bold, s.italic, s.underline, s.undercurl, s.strikethrough
    )
}
fn measure(f: impl FnOnce()) -> serde_json::Value {
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    let time = Instant::now();
    f();
    let elapsed = time.elapsed();
    let allocations = ALLOCATIONS.load(Ordering::Relaxed) - before;
    serde_json::json!({"milliseconds":elapsed.as_secs_f64()*1000.,"allocation_calls":allocations})
}
fn main() {
    let texts: Vec<_> = (0..256)
        .map(|i| char::from_u32(0x400 + i).unwrap().to_string())
        .collect();
    let mut old = HashMap::new();
    let mut new = TextCache::new(8192);
    for (i, text) in texts.iter().enumerate() {
        old.insert(old_key(text, style(i)), i);
        new.insert(style(i), text, i);
    }
    let mut runs = vec![];
    for _ in 0..5 {
        let before = measure(|| {
            for i in 0..250_000 {
                let i = black_box(i % texts.len());
                let text = texts[i].clone();
                let key = old_key(&text, style(i));
                assert!(old.contains_key(&key));
                black_box(old.get(&key));
            }
        });
        let after = measure(|| {
            for i in 0..250_000 {
                let i = black_box(i % texts.len());
                black_box(new.get(style(i), &texts[i]));
            }
        });
        let theme = TerminalTheme([Some(0x123456); 22]);
        let mut cache = ThemeConfigCache::default();
        cache.update(Some(&theme), 1, 2);
        let theme_before = measure(|| {
            for _ in 0..10_000 {
                for _ in 0..8 {
                    black_box(theme.config(1, 2));
                }
            }
        });
        let theme_after = measure(|| {
            for _ in 0..10_000 {
                black_box(cache.update(Some(black_box(&theme)), 1, 2));
            }
        });
        runs.push(serde_json::json!({"text_before":before,"text_after":after,"theme_before":theme_before,"theme_after":theme_after}));
    }
    let mut old = HashMap::new();
    let mut new = TextCache::new(8192);
    let (mut old_misses, mut new_misses) = (0, 0);
    for iteration in 0..12_000 {
        // One new cold glyph followed by repeated use of the hot working set.
        for index in std::iter::once(iteration + 256).chain(0..256) {
            let text = index.to_string();
            let style = style(index);
            let key = old_key(&text, style);
            if !old.contains_key(&key) {
                if old.len() > 8192 {
                    old.clear();
                }
                old.insert(key, index);
                old_misses += 1;
            }
            if new.get(style, &text).is_none() {
                new.insert(style, &text, index);
                new_misses += 1;
            }
        }
    }
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"runs":runs,"pressure_misses":{"before":old_misses,"after":new_misses},"notes":"250000 warmed text lookups; 10000 unchanged frames with 8 terminals. Values stand in for shaped glyphs; this does not time shaping or GPU work. Allocation counting is enabled in both paths."})).unwrap());
}
