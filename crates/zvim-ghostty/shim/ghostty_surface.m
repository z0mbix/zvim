#import <AppKit/AppKit.h>
#import <CoreVideo/CoreVideo.h>
#import <IOSurface/IOSurface.h>
#import <stdatomic.h>
#import <stdlib.h>
#import <string.h>
#import <ghostty.h>

@interface ZvimGhosttyView : NSView
@end

@implementation ZvimGhosttyView
- (NSView *)hitTest:(NSPoint)point {
    (void)point;
    return nil;
}
@end

typedef void (*zvim_ghostty_wakeup_cb)(void *userdata);
// Adapter operation values: paste=0, read=1, write=2.
typedef bool (*zvim_ghostty_approve_clipboard_cb)(void *userdata, int operation, const char *text);

typedef struct zvim_ghostty_surface {
    ghostty_config_t config;
    ghostty_app_t app;
    ghostty_surface_t surface;
    NSView *parent;
    ZvimGhosttyView *view;
    void *wakeup_userdata;
    zvim_ghostty_wakeup_cb wakeup;
    zvim_ghostty_approve_clipboard_cb approve_clipboard;
    _Atomic bool alive;
    intptr_t search_total;
    intptr_t search_selected;
} zvim_ghostty_surface;

static void runtime_wakeup(void *userdata) {
    zvim_ghostty_surface *state = userdata;
    state->wakeup(state->wakeup_userdata);
}

static bool runtime_action(ghostty_app_t app, ghostty_target_s target, ghostty_action_s action) {
    (void)app;
    if (target.tag == GHOSTTY_TARGET_SURFACE && target.target.surface != NULL) {
        zvim_ghostty_surface *search = ghostty_surface_userdata(target.target.surface);
        if (search != NULL && action.tag == GHOSTTY_ACTION_SEARCH_TOTAL) {
            search->search_total = action.action.search_total.total;
            return true;
        }
        if (search != NULL && action.tag == GHOSTTY_ACTION_SEARCH_SELECTED) {
            search->search_selected = action.action.search_selected.selected;
            return true;
        }
    }

    if (action.tag == GHOSTTY_ACTION_RENDER &&
        target.tag == GHOSTTY_TARGET_SURFACE &&
        target.target.surface != NULL) {
        ghostty_surface_draw(target.target.surface);
        return true;
    }
    return false;
}

static bool runtime_read_clipboard(void *userdata, ghostty_clipboard_e location, void *request) {
    (void)location;
    zvim_ghostty_surface *state = userdata;
    if (state->surface == NULL) return false;
    NSString *text = [[NSPasteboard generalPasteboard] stringForType:NSPasteboardTypeString];
    if (text == nil) return false;
    ghostty_surface_complete_clipboard_request(state->surface, text.UTF8String, request, false);
    return true;
}

static void runtime_confirm_read_clipboard(
    void *userdata,
    const char *text,
    void *request,
    ghostty_clipboard_request_e kind
) {
    zvim_ghostty_surface *state = userdata;
    if (state->surface != NULL) {
        int operation = kind == GHOSTTY_CLIPBOARD_REQUEST_PASTE ? 0 : 1;
        bool approved = state->approve_clipboard != NULL &&
            state->approve_clipboard(state->wakeup_userdata, operation, text);
        // Completing with empty text releases Ghostty's request without exposing
        // clipboard contents or inserting an unsafe paste.
        ghostty_surface_complete_clipboard_request(state->surface, approved ? text : "", request, true);
    }
}

static void runtime_write_clipboard(
    void *userdata,
    ghostty_clipboard_e location,
    const ghostty_clipboard_content_s *content,
    size_t count,
    bool confirm
) {
    zvim_ghostty_surface *state = userdata;
    (void)location;
    for (size_t index = 0; index < count; index++) {
        if (strcmp(content[index].mime, "text/plain") != 0) continue;
        if (confirm && (state->approve_clipboard == NULL ||
            !state->approve_clipboard(state->wakeup_userdata, 2, content[index].data))) return;
        NSString *text = [NSString stringWithUTF8String:content[index].data];
        if (text == nil) return;
        NSPasteboard *pasteboard = [NSPasteboard generalPasteboard];
        [pasteboard clearContents];
        [pasteboard setString:text forType:NSPasteboardTypeString];
        return;
    }
}

static void runtime_close_surface(void *userdata, bool process_alive) {
    (void)process_alive;
    zvim_ghostty_surface *state = userdata;
    atomic_store_explicit(&state->alive, false, memory_order_release);
    runtime_wakeup(state);
}

zvim_ghostty_surface *zvim_ghostty_surface_new(
    void *parent_view,
    const char *working_directory,
    const char *command,
    bool load_user_config,
    const char *theme_config_path,
    void *wakeup_userdata,
    zvim_ghostty_wakeup_cb wakeup,
    zvim_ghostty_approve_clipboard_cb approve_clipboard
) {
    static dispatch_once_t once;
    static int init_result = -1;
    dispatch_once(&once, ^{
        setenv("GHOSTTY_LOG", "stderr", 0);
        init_result = ghostty_init(0, NULL);
    });
    if (init_result != GHOSTTY_SUCCESS || parent_view == NULL ||
        wakeup_userdata == NULL || wakeup == NULL) return NULL;

    zvim_ghostty_surface *state = calloc(1, sizeof(zvim_ghostty_surface));
    if (state == NULL) return NULL;
    atomic_init(&state->alive, true);
    state->search_total = -1;
    state->search_selected = -1;
    state->wakeup_userdata = wakeup_userdata;
    state->wakeup = wakeup;
    state->approve_clipboard = approve_clipboard;
    state->parent = (NSView *)parent_view;
    state->view = [[ZvimGhosttyView alloc] initWithFrame:NSMakeRect(0, 0, 800, 600)];
    [state->view setHidden:YES];
    [state->parent addSubview:state->view];

    state->config = ghostty_config_new();
    if (state->config == NULL) goto fail;
    if (load_user_config) {
        ghostty_config_load_default_files(state->config);
        ghostty_config_load_recursive_files(state->config);
    }
    if (theme_config_path != NULL) {
        ghostty_config_load_file(state->config, theme_config_path);
    }
    ghostty_config_finalize(state->config);

    ghostty_runtime_config_s runtime = {
        .userdata = state,
        .supports_selection_clipboard = false,
        .wakeup_cb = runtime_wakeup,
        .action_cb = runtime_action,
        .read_clipboard_cb = runtime_read_clipboard,
        .confirm_read_clipboard_cb = runtime_confirm_read_clipboard,
        .write_clipboard_cb = runtime_write_clipboard,
        .close_surface_cb = runtime_close_surface,
    };
    state->app = ghostty_app_new(&runtime, state->config);
    if (state->app == NULL) goto fail;

    ghostty_surface_config_s surface_config = ghostty_surface_config_new();
    surface_config.platform_tag = GHOSTTY_PLATFORM_MACOS;
    surface_config.platform.macos.nsview = state->view;
    surface_config.userdata = state;
    surface_config.scale_factor = state->parent.window.backingScaleFactor ?: NSScreen.mainScreen.backingScaleFactor;
    ghostty_env_var_s environment[] = {
        { .key = "TERM", .value = "xterm-256color" },
        { .key = "COLORTERM", .value = "truecolor" },
        { .key = "TERM_PROGRAM", .value = "gpui-ghostty" },
    };
    surface_config.working_directory = working_directory;
    surface_config.command = command;
    surface_config.env_vars = environment;
    surface_config.env_var_count = sizeof(environment) / sizeof(environment[0]);
    surface_config.wait_after_command = false;
    surface_config.context = GHOSTTY_SURFACE_CONTEXT_WINDOW;
    state->surface = ghostty_surface_new(state->app, &surface_config);
    if (state->surface == NULL) goto fail;

    ghostty_app_set_focus(state->app, false);
    ghostty_surface_set_focus(state->surface, false);
    return state;

fail:
    if (state->surface != NULL) ghostty_surface_free(state->surface);
    if (state->app != NULL) ghostty_app_free(state->app);
    if (state->config != NULL) ghostty_config_free(state->config);
    [state->view removeFromSuperview];
    [state->view release];
    free(state);
    return NULL;
}

void zvim_ghostty_surface_free(zvim_ghostty_surface *state) {
    if (state == NULL) return;
    [state->view removeFromSuperview];
    if (state->surface != NULL) ghostty_surface_free(state->surface);
    if (state->app != NULL) ghostty_app_free(state->app);
    if (state->config != NULL) ghostty_config_free(state->config);
    [state->view release];
    free(state);
}

void zvim_ghostty_surface_tick(zvim_ghostty_surface *state) {
    if (state == NULL || state->app == NULL) return;
    ghostty_app_tick(state->app);
}

bool zvim_ghostty_surface_is_alive(const zvim_ghostty_surface *state) {
    return state != NULL && atomic_load_explicit(&state->alive, memory_order_acquire)
        && !ghostty_surface_process_exited(state->surface);
}

bool zvim_ghostty_surface_snapshot(
    zvim_ghostty_surface *state,
    uint8_t **pixels,
    uint32_t *width,
    uint32_t *height,
    size_t *length
) {
    if (pixels == NULL || width == NULL || height == NULL || length == NULL) return false;
    *pixels = NULL;
    *width = 0;
    *height = 0;
    *length = 0;
    if (state == NULL || state->view == nil) return false;

    id contents = state->view.layer.contents;
    if (contents == nil || CFGetTypeID((CFTypeRef)contents) != IOSurfaceGetTypeID()) return false;
    IOSurfaceRef surface = (IOSurfaceRef)contents;
    CFRetain(surface);
    if (IOSurfaceLock(surface, kIOSurfaceLockReadOnly, NULL) != kIOReturnSuccess) {
        CFRelease(surface);
        return false;
    }

    size_t surface_width = IOSurfaceGetWidth(surface);
    size_t surface_height = IOSurfaceGetHeight(surface);
    OSType pixel_format = IOSurfaceGetPixelFormat(surface);
    size_t source_stride = IOSurfaceGetBytesPerRow(surface);
    const uint8_t *source = IOSurfaceGetBaseAddress(surface);
    bool valid = pixel_format == kCVPixelFormatType_32BGRA && surface_width > 0 &&
        surface_height > 0 && source != NULL && surface_width <= UINT32_MAX &&
        surface_height <= UINT32_MAX &&
        surface_width <= SIZE_MAX / 4 && surface_height <= SIZE_MAX / (surface_width * 4) &&
        source_stride >= surface_width * 4;
    size_t destination_stride = valid ? surface_width * 4 : 0;
    size_t byte_length = valid ? destination_stride * surface_height : 0;
    uint8_t *copy = valid ? malloc(byte_length) : NULL;
    if (copy != NULL) {
        for (size_t row = 0; row < surface_height; row++) {
            memcpy(copy + row * destination_stride, source + row * source_stride, destination_stride);
        }
        *pixels = copy;
        *width = (uint32_t)surface_width;
        *height = (uint32_t)surface_height;
        *length = byte_length;
    }

    IOSurfaceUnlock(surface, kIOSurfaceLockReadOnly, NULL);
    CFRelease(surface);
    return copy != NULL;
}

void zvim_ghostty_surface_snapshot_free(uint8_t *pixels) {
    free(pixels);
}

void zvim_ghostty_surface_set_frame(
    zvim_ghostty_surface *state,
    double x,
    double y,
    double width,
    double height
) {
    if (state == NULL || state->surface == NULL) return;
    double parent_height = NSHeight(state->parent.bounds);
    [state->view setFrame:NSMakeRect(x, parent_height - y - height, width, height)];
    double scale = state->parent.window.backingScaleFactor ?: NSScreen.mainScreen.backingScaleFactor;
    ghostty_surface_set_content_scale(state->surface, scale, scale);
    ghostty_surface_set_size(state->surface, (uint32_t)(width * scale), (uint32_t)(height * scale));
    ghostty_surface_refresh(state->surface);
}

void zvim_ghostty_surface_set_visible(zvim_ghostty_surface *state, bool visible) {
    if (state == NULL || state->surface == NULL) return;
    [state->view setHidden:!visible];
    ghostty_surface_set_occlusion(state->surface, visible);
    if (visible) ghostty_surface_refresh(state->surface);
}

void zvim_ghostty_surface_set_focus(zvim_ghostty_surface *state, bool focused) {
    if (state == NULL || state->surface == NULL) return;
    ghostty_app_set_focus(state->app, focused);
    ghostty_surface_set_focus(state->surface, focused);
}

bool zvim_ghostty_surface_key(
    zvim_ghostty_surface *state,
    int action,
    int modifiers,
    int consumed_modifiers,
    uint32_t keycode,
    const char *text,
    uint32_t unshifted_codepoint
) {
    if (state == NULL || state->surface == NULL) return false;
    ghostty_input_key_s event = {
        .action = (ghostty_input_action_e)action,
        .mods = (ghostty_input_mods_e)modifiers,
        .consumed_mods = (ghostty_input_mods_e)consumed_modifiers,
        .keycode = keycode,
        .text = text,
        .unshifted_codepoint = unshifted_codepoint,
        .composing = false,
    };
    return ghostty_surface_key(state->surface, event);
}

void zvim_ghostty_surface_text(zvim_ghostty_surface *state, const char *text, size_t length) {
    if (state != NULL && state->surface != NULL) ghostty_surface_text(state->surface, text, length);
}

void zvim_ghostty_surface_mouse_position(
    zvim_ghostty_surface *state,
    double x,
    double y,
    int modifiers
) {
    if (state != NULL && state->surface != NULL) {
        ghostty_surface_mouse_pos(state->surface, x, y, (ghostty_input_mods_e)modifiers);
    }
}

void zvim_ghostty_surface_mouse_button(
    zvim_ghostty_surface *state,
    int mouse_state,
    int button,
    int modifiers
) {
    if (state != NULL && state->surface != NULL) {
        ghostty_surface_mouse_button(
            state->surface,
            (ghostty_input_mouse_state_e)mouse_state,
            (ghostty_input_mouse_button_e)button,
            (ghostty_input_mods_e)modifiers
        );
    }
}

void zvim_ghostty_surface_mouse_scroll(
    zvim_ghostty_surface *state,
    double x,
    double y,
    int modifiers
) {
    if (state != NULL && state->surface != NULL) {
        ghostty_surface_mouse_scroll(state->surface, x, y, modifiers);
    }
}

// The baseline config remains owned by the surface. Only this surface is changed.
bool zvim_ghostty_surface_update_config(zvim_ghostty_surface *state, const char *path) {
    if (state == NULL || state->surface == NULL) return false;
    ghostty_config_t config = ghostty_config_clone(state->config);
    if (config == NULL) return false;
    if (path != NULL) ghostty_config_load_file(config, path);
    ghostty_config_finalize(config);
    ghostty_surface_update_config(state->surface, config);
    ghostty_config_free(config);
    return true;
}

bool zvim_ghostty_surface_needs_confirm_quit(const zvim_ghostty_surface *state) {
    return state != NULL && state->surface != NULL && ghostty_surface_needs_confirm_quit(state->surface);
}

bool zvim_ghostty_surface_binding_action(zvim_ghostty_surface *state, const char *action, size_t length) {
    if (state != NULL && length >= 7 && memcmp(action, "search:", 7) == 0) {
        state->search_total = -1;
        state->search_selected = -1;
    }
    return state != NULL && ghostty_surface_binding_action(state->surface, action, length);
}
void zvim_ghostty_surface_search_status(zvim_ghostty_surface *state, intptr_t *total, intptr_t *selected) {
    *total = state->search_total;
    *selected = state->search_selected;
}
