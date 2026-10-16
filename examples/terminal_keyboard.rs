//! Opt-in macOS keyboard regression test; requires the British keyboard layout.
//! Sends native Option+3 events only to its own window and checks actual PTY bytes.
#![allow(dead_code)]
#[path = "../src/terminal.rs"]
mod terminal;
#[path = "../src/terminal_search.rs"]
mod terminal_search;

extern crate zvim as _;
use gpui::*;
#[cfg(target_os = "macos")]
use std::time::Duration;

struct Harness {
    terminal: Entity<terminal::Terminal>,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.terminal.clone()
    }
}

#[cfg(target_os = "macos")]
fn option_three() {
    use std::ffi::{c_char, c_void};
    type Id = *mut c_void;
    #[repr(C)]
    struct Point {
        x: f64,
        y: f64,
    }
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Id;
        fn objc_msgSend();
    }
    // SAFETY: These are AppKit's documented Objective-C method signatures.
    // Send events only to this test process's own key window on its UI thread.
    unsafe {
        let get: unsafe extern "C" fn(Id, Id) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        let number: unsafe extern "C" fn(Id, Id) -> isize =
            std::mem::transmute(objc_msgSend as *const ());
        let string: unsafe extern "C" fn(Id, Id, *const c_char) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        let event: unsafe extern "C" fn(
            Id,
            Id,
            usize,
            Point,
            usize,
            f64,
            isize,
            Id,
            Id,
            Id,
            bool,
            u16,
        ) -> Id = std::mem::transmute(objc_msgSend as *const ());
        let send: unsafe extern "C" fn(Id, Id, Id) = std::mem::transmute(objc_msgSend as *const ());
        let app = get(
            objc_getClass(c"NSApplication".as_ptr()),
            sel_registerName(c"sharedApplication".as_ptr()),
        );
        let window = get(app, sel_registerName(c"keyWindow".as_ptr()));
        assert!(!window.is_null());
        let ns_string = objc_getClass(c"NSString".as_ptr());
        let characters = string(
            ns_string,
            sel_registerName(c"stringWithUTF8String:".as_ptr()),
            c"#".as_ptr(),
        );
        let ignoring = string(
            ns_string,
            sel_registerName(c"stringWithUTF8String:".as_ptr()),
            c"3".as_ptr(),
        );
        let event_selector = sel_registerName(c"keyEventWithType:location:modifierFlags:timestamp:windowNumber:context:characters:charactersIgnoringModifiers:isARepeat:keyCode:".as_ptr());
        for (kind, held) in [(10, false), (10, true), (11, false)] {
            let key = event(
                objc_getClass(c"NSEvent".as_ptr()),
                event_selector,
                kind,
                Point { x: 0., y: 0. },
                1 << 19,
                0.,
                number(window, sel_registerName(c"windowNumber".as_ptr())),
                std::ptr::null_mut(),
                characters,
                ignoring,
                held,
                20,
            );
            let responder = get(window, sel_registerName(c"firstResponder".as_ptr()));
            send(
                responder,
                sel_registerName(if kind == 10 { c"keyDown:" } else { c"keyUp:" }.as_ptr()),
                key,
            );
        }
    }
}

#[cfg(target_os = "macos")]
fn main() {
    terminal::configure_resources();
    let protocol = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "kitty".to_owned());
    let enable = match protocol.as_str() {
        "legacy" => "",
        "kitty" => r"\033[>1u",
        "modify-other-keys" => r"\033[>4;2m",
        _ => panic!("Choose legacy, kitty or modify-other-keys"),
    };
    let directory = tempfile::tempdir().unwrap();
    Application::new().run(move |cx| {
        let path = directory.path().to_owned();
        let command = format!(
            r#"/bin/sh -c 'stty raw -echo; printf "{enable}"; printf ready > ready; exec /bin/cat > output'"#
        );
        let _handle = cx
            .open_window(WindowOptions::default(), |w, cx| {
                cx.new(|cx| Harness {
                    terminal: terminal::Terminal::spawn(
                        gpui_libghostty::TerminalOptions::new(command, &path),
                        w,
                        cx,
                    )
                    .unwrap(),
                })
            })
            .unwrap();
        cx.activate(true);
        cx.spawn(async move |cx| {
            let _directory = directory;
            for _ in 0..100 {
                if path.join("ready").exists() { break; }
                cx.background_executor().timer(Duration::from_millis(50)).await;
            }
            assert!(path.join("ready").exists(), "test shell ready");
            cx.background_executor().timer(Duration::from_millis(300)).await;
            // Native callbacks re-enter GPUI: send outside cx.update's App borrow.
            option_three();
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let actual = std::fs::read_to_string(path.join("output")).unwrap();
            // Quit before asserting so a failure also closes the disposable window.
            let passed = actual == "##";
            eprintln!("{protocol}: Option+3 press/repeat/release received {actual:?}");
            cx.update(|cx| cx.quit()).unwrap();
            assert!(passed, "Expected two literal # characters; use the British keyboard layout");
        }).detach();
    });
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This keyboard test requires macOS");
}
