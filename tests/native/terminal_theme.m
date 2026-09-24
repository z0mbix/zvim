// macOS integration test of the actual native shim, renderer and child process.
#import "../../crates/zvim-ghostty/shim/ghostty_surface.m"
#include <assert.h>
#include <stdio.h>

static void wake(void *data) { (void)data; }
static bool approve(void *data, int op, const char *text) {
    (void)data; (void)op; (void)text; return false;
}
static void pump(zvim_ghostty_surface *state) {
    NSDate *until = [NSDate dateWithTimeIntervalSinceNow:0.5];
    while ([until timeIntervalSinceNow] > 0) {
        zvim_ghostty_surface_tick(state);
        [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
    }
}
static bool near(uint32_t actual, uint32_t expected) {
    for (int shift = 0; shift <= 16; shift += 8) {
        if (abs((int)((actual >> shift) & 255) - (int)((expected >> shift) & 255)) > 8) return false;
    }
    return true;
}
static uint32_t background(zvim_ghostty_surface *state) {
    pump(state);
    uint8_t *pixels = NULL;
    uint32_t width = 0, height = 0;
    size_t length = 0;
    assert(zvim_ghostty_surface_snapshot(state, &pixels, &width, &height, &length));
    size_t i = ((height / 2) * width + width / 2) * 4;
    uint32_t rgb = (pixels[i + 2] << 16) | (pixels[i + 1] << 8) | pixels[i];
    fprintf(stderr, "background pixel: #%06x\n", rgb);
    zvim_ghostty_surface_snapshot_free(pixels);
    return rgb;
}
static void contains(zvim_ghostty_surface *state, const char *needle) {
    ghostty_selection_s selection = {
        .top_left = { .tag = GHOSTTY_POINT_SCREEN, .coord = GHOSTTY_POINT_COORD_TOP_LEFT },
        .bottom_right = { .tag = GHOSTTY_POINT_SCREEN, .coord = GHOSTTY_POINT_COORD_BOTTOM_RIGHT },
    };
    ghostty_text_s text = {0};
    assert(ghostty_surface_read_text(state->surface, selection, &text));
    assert(strstr(text.text, needle) != NULL);
    ghostty_surface_free_text(state->surface, &text);
}
int main(int argc, char **argv) {
    assert(argc == 3);
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
        [NSApp finishLaunching];
        NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(100, 100, 640, 400)
            styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
        [window setTitle:@"Zvim terminal theme test"];
        [window orderFrontRegardless];
        zvim_ghostty_surface *state = zvim_ghostty_surface_new(window.contentView, "/tmp",
            "/bin/sh -c 'echo THEME_SENTINEL; echo PID=$$; exec /bin/cat'", false, argv[1], window, wake, approve);
        assert(state != NULL);
        zvim_ghostty_surface_set_frame(state, 0, 0, 640, 400);
        zvim_ghostty_surface_set_visible(state, true);
        ghostty_surface_t original = state->surface;
        uint32_t initial = background(state);
        assert(near(initial, 0x112233));
        contains(state, "THEME_SENTINEL");
        assert(zvim_ghostty_surface_needs_confirm_quit(state));
        assert(zvim_ghostty_surface_update_config(state, argv[2]));
        assert(near(background(state), 0x334455));
        assert(state->surface == original && zvim_ghostty_surface_is_alive(state));
        contains(state, "THEME_SENTINEL");
        zvim_ghostty_surface_text(state, "AFTER_THEME\n", 12);
        pump(state);
        contains(state, "AFTER_THEME");
        assert(zvim_ghostty_surface_update_config(state, NULL));
        assert(background(state) == initial);
        contains(state, "THEME_SENTINEL");
        assert(state->surface == original && zvim_ghostty_surface_is_alive(state));
        zvim_ghostty_surface_free(state);
        // Verify real shell integration distinguishes a prompt from a job.
        state = zvim_ghostty_surface_new(window.contentView, "/tmp", "/bin/zsh",
            false, argv[1], window, wake, approve);
        assert(state != NULL);
        zvim_ghostty_surface_set_frame(state, 0, 0, 640, 400);
        zvim_ghostty_surface_set_visible(state, true);
        pump(state);
        assert(zvim_ghostty_surface_is_alive(state));
        assert(!zvim_ghostty_surface_needs_confirm_quit(state));
        zvim_ghostty_surface_text(state, "sleep 30", 8);
        zvim_ghostty_surface_key(state, GHOSTTY_ACTION_PRESS, 0, 0, 36, "\r", 13);
        zvim_ghostty_surface_key(state, GHOSTTY_ACTION_RELEASE, 0, 0, 36, NULL, 13);
        pump(state);
        assert(zvim_ghostty_surface_needs_confirm_quit(state));
        zvim_ghostty_surface_free(state);
        [window orderOut:nil];
        puts("PASS: native colour update and restore; same live surface; output and input preserved; idle prompt and running job distinguished");
    }
    return 0;
}
