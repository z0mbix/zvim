//! Read only the smoke-test process's AppKit objects, on its main thread.
use std::ffi::{CStr, c_char, c_void};
type Id = *mut c_void;
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Id;
    fn objc_msgSend();
}
unsafe fn selector(name: &CStr) -> Id {
    unsafe { sel_registerName(name.as_ptr()) }
}
unsafe fn id(object: Id, name: &CStr) -> Id {
    unsafe {
        let send: unsafe extern "C" fn(Id, Id) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        send(object, selector(name))
    }
}
unsafe fn integer(object: Id, name: &CStr) -> isize {
    unsafe {
        let send: unsafe extern "C" fn(Id, Id) -> isize =
            std::mem::transmute(objc_msgSend as *const ());
        send(object, selector(name))
    }
}
unsafe fn send_index(object: Id, sel: Id, index: isize) -> Id {
    unsafe {
        let send: unsafe extern "C" fn(Id, Id, isize) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        send(object, sel, index)
    }
}
pub fn check() {
    // SAFETY: Each selector has the declared Objective-C ABI; objects belong to
    // this test application and remain alive on the calling AppKit main thread.
    unsafe {
        let app = id(
            objc_getClass(c"NSApplication".as_ptr()),
            c"sharedApplication",
        );
        let menu = id(app, c"windowsMenu");
        assert!(!menu.is_null());
        id(menu, c"update");
        let mut titles = vec![];
        let mut checked = 0;
        for i in 0..integer(menu, c"numberOfItems") {
            let item = send_index(menu, selector(c"itemAtIndex:"), i);
            let title = id(item, c"title");
            let text = CStr::from_ptr(id(title, c"UTF8String").cast())
                .to_string_lossy()
                .into_owned();
            if text.starts_with("Zvim test") {
                checked += usize::from(integer(item, c"state") == 1);
            }
            titles.push(text);
        }
        for expected in [
            "Minimise",
            "Bring All to Front",
            "Zvim test 0",
            "Zvim test 1",
            "Zvim test 2",
        ] {
            assert!(
                titles.iter().any(|s| s == expected),
                "Missing {expected}: {titles:?}"
            );
        }
        assert_eq!(checked, 1, "current window checkmark");
        eprintln!("PASS: AppKit Window menu titles, native commands and current-window checkmark");
    }
}

pub fn click(title: &str) {
    // SAFETY: Menu objects and selectors are inspected only inside the test process.
    unsafe {
        let app = id(
            objc_getClass(c"NSApplication".as_ptr()),
            c"sharedApplication",
        );
        let menu = id(app, c"windowsMenu");
        for i in 0..integer(menu, c"numberOfItems") {
            let item = send_index(menu, selector(c"itemAtIndex:"), i);
            let name =
                CStr::from_ptr(id(id(item, c"title"), c"UTF8String").cast()).to_string_lossy();
            if name == title {
                let send: unsafe extern "C" fn(Id, Id, isize) =
                    std::mem::transmute(objc_msgSend as *const ());
                send(menu, selector(c"performActionForItemAtIndex:"), i);
                return;
            }
        }
        panic!("missing menu item {title}");
    }
}
pub fn minimised_count() -> usize {
    // SAFETY: The return signatures match NSArray and NSWindow methods.
    unsafe {
        let app = id(
            objc_getClass(c"NSApplication".as_ptr()),
            c"sharedApplication",
        );
        let windows = id(app, c"windows");
        (0..integer(windows, c"count"))
            .filter(|index| {
                let w = send_index(windows, selector(c"objectAtIndex:"), *index);
                let send: unsafe extern "C" fn(Id, Id) -> bool =
                    std::mem::transmute(objc_msgSend as *const ());
                send(w, selector(c"isMiniaturized"))
            })
            .count()
    }
}

pub fn snapshot() {
    // SAFETY: Inspect only this test app's key window on the AppKit thread.
    let number = unsafe {
        let app = id(
            objc_getClass(c"NSApplication".as_ptr()),
            c"sharedApplication",
        );
        integer(id(app, c"keyWindow"), c"windowNumber")
    };
    if number > 0 {
        let _ = std::fs::create_dir_all(".cache");
        let result = std::process::Command::new("/usr/sbin/screencapture")
            .args([
                "-x",
                "-l",
                &number.to_string(),
                ".cache/desktop-features.png",
            ])
            .status();
        eprintln!("Test-window screenshot: {result:?}");
    }
}
