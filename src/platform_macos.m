#import <AppKit/AppKit.h>

void zvim_configure_window_menu(void) {
    NSMenu *menu = [[NSApp.mainMenu itemWithTitle:@"Window"] submenu];
    if (menu == nil) return;
    NSMenuItem *minimise = [[NSMenuItem alloc] initWithTitle:@"Minimise"
        action:@selector(performMiniaturize:) keyEquivalent:@"m"];
    [menu insertItem:minimise atIndex:0];
    [minimise release];
    [menu insertItem:[NSMenuItem separatorItem] atIndex:1];
    [menu addItem:[NSMenuItem separatorItem]];
    NSMenuItem *front = [[NSMenuItem alloc] initWithTitle:@"Bring All to Front"
        action:@selector(arrangeInFront:) keyEquivalent:@""];
    [front setTarget:NSApp];
    [menu addItem:front];
    [front release];
    [NSApp setWindowsMenu:menu];
    for (NSWindow *window in NSApp.windows) {
        if (window.canBecomeMainWindow && !window.excludedFromWindowsMenu) {
            [NSApp addWindowsItem:window title:window.title filename:NO];
        }
    }
}
