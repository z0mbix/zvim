//! Opt-in real-window smoke test; only its own windows and shells are closed.
#![allow(dead_code)]
#[path = "../src/terminal.rs"]
mod terminal;
#[path = "../src/terminal_search.rs"]
mod terminal_search;
extern crate zvim as _;
#[cfg(target_os = "macos")]
#[path = "support/menu_probe.rs"]
mod menu_probe;
use gpui::*;
use std::time::Duration;
struct Harness {
    terminal: Entity<terminal::Terminal>,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.terminal.clone()
    }
}
fn main() {
    terminal::configure_resources();
    Application::new().run(|cx| {
        cx.bind_keys([KeyBinding::new("cmd-f",terminal::Find,Some("Terminal"))]);
        cx.set_menus(vec![Menu{name:"Window".into(),items:vec![]}]);
        #[cfg(target_os="macos")]
        unsafe { unsafe extern "C" {fn zvim_configure_window_menu();} zvim_configure_window_menu(); }
        let mut handles=vec![];
        for index in 0..3 {
            let h=cx.open_window(WindowOptions { titlebar:Some(TitlebarOptions {title:Some(format!("Zvim test {index}").into()),..Default::default()}),..Default::default()}, |w,cx| cx.new(|cx| Harness {terminal:terminal::Terminal::spawn(gpui_libghostty::TerminalOptions::new("/bin/sh -c 'printf \"needle first\\n\"; /usr/bin/seq 1 200; printf \"needle last\\n\"; sleep 60'", "/tmp"),w,cx).unwrap()})).unwrap();
            handles.push(h);
        }
        let h=handles[2];
        cx.activate(true);
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            #[cfg(target_os="macos")]
            {
                cx.update(|_| {menu_probe::check();menu_probe::click("Minimise");}).unwrap();
                cx.background_executor().timer(Duration::from_millis(700)).await;
                cx.update(|_| {assert_eq!(menu_probe::minimised_count(),1);menu_probe::click("Zvim test 2");}).unwrap();
                cx.background_executor().timer(Duration::from_millis(700)).await;
                cx.update(|_| {assert_eq!(menu_probe::minimised_count(),0);menu_probe::click("Bring All to Front");}).unwrap();
                eprintln!("PASS: minimise and restore by selecting window from native menu");
            }
            h.update(cx, |_,w,cx| w.dispatch_action(Box::new(terminal::Find),cx)).unwrap();
            cx.background_executor().timer(Duration::from_millis(200)).await;
            h.update(cx, |view,w,cx| {
                view.terminal.update(cx, |t,cx| t.replace_text_in_range(None,"needle",w,cx));
            }).unwrap();
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let first=h.read_with(cx, |v,cx| v.terminal.read(cx).search_status()).unwrap().expect("search UI opened");
            assert_eq!(first.0,2,"both visible and scrollback matches");
            #[cfg(target_os="macos")]
            cx.update(|_| menu_probe::snapshot()).unwrap();
            h.update(cx, |_,w,cx| w.dispatch_action(Box::new(terminal::NextMatch),cx)).unwrap();
            cx.background_executor().timer(Duration::from_millis(300)).await;
            let next=h.read_with(cx, |v,cx| v.terminal.read(cx).search_status()).unwrap().unwrap();
            assert_ne!(first.1,next.1,"next match changes selection");
            h.update(cx, |_,w,cx| w.dispatch_action(Box::new(terminal::PreviousMatch),cx)).unwrap();
            cx.background_executor().timer(Duration::from_millis(300)).await;
            let prev=h.read_with(cx, |v,cx| v.terminal.read(cx).search_status()).unwrap().unwrap();
            assert_eq!(prev.1,first.1,"previous match restores selection");
            h.update(cx, |v,w,cx| v.terminal.update(cx, |t,cx| t.replace_text_in_range(Some(0..6),"missing",w,cx))).unwrap();
            cx.background_executor().timer(Duration::from_millis(500)).await;
            assert_eq!(h.read_with(cx, |v,cx| v.terminal.read(cx).search_status()).unwrap().unwrap().0,0);
            eprintln!("PASS: native scrollback search, match counts, next/previous, no matches");
            cx.update(|cx| cx.quit()).unwrap();
        }).detach();
    });
}
