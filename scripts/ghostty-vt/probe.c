/* Headless C ABI proof for the pinned libghostty-vt migration (US-0208).
 * This opens no PTY/process and is not the application engine cutover.
 */
#include <ghostty/vt.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Layouts mirrored by the owned Rust viewport projection (Windows x64). */
#define ABI_VALUE(name, value) _Static_assert((name) == (value), #name " changed")
#define ABI_LAYOUT(type, size, align) \
    _Static_assert(sizeof(type) == (size) && _Alignof(type) == (align), #type " layout changed")
#define ABI_OFFSET(type, member, offset) \
    _Static_assert(offsetof(type, member) == (offset), #type "." #member " moved")
ABI_LAYOUT(GhosttyColorRgb, 3, 1);
ABI_OFFSET(GhosttyColorRgb, r, 0);
ABI_OFFSET(GhosttyColorRgb, g, 1);
ABI_OFFSET(GhosttyColorRgb, b, 2);
ABI_VALUE(GHOSTTY_TERMINAL_OPT_COLOR_FOREGROUND, 12);
ABI_VALUE(GHOSTTY_TERMINAL_OPT_COLOR_BACKGROUND, 13);
ABI_VALUE(GHOSTTY_TERMINAL_OPT_COLOR_CURSOR, 14);
ABI_VALUE(GHOSTTY_TERMINAL_OPT_COLOR_PALETTE, 15);
ABI_VALUE(GHOSTTY_TERMINAL_OPT_SCROLLBACK_MAX_BYTES, 28);
ABI_VALUE(GHOSTTY_TERMINAL_OPT_SCROLLBACK_MAX_LINES, 29);
ABI_VALUE(GHOSTTY_TERMINAL_DATA_SCROLLBACK_MAX_BYTES, 34);
ABI_VALUE(GHOSTTY_TERMINAL_DATA_SCROLLBACK_MAX_LINES, 35);
ABI_LAYOUT(GhosttyStyleColor, 4, 1);
ABI_OFFSET(GhosttyStyleColor, tag, 0);
ABI_OFFSET(GhosttyStyleColor, value, 1);
ABI_LAYOUT(GhosttyStyle, 16, 2);
ABI_VALUE(GHOSTTY_STYLE_FLAG_BOLD, 1u << 0);
ABI_VALUE(GHOSTTY_STYLE_FLAG_ITALIC, 1u << 1);
ABI_VALUE(GHOSTTY_STYLE_FLAG_FAINT, 1u << 2);
ABI_VALUE(GHOSTTY_STYLE_FLAG_BLINK, 1u << 3);
ABI_VALUE(GHOSTTY_STYLE_FLAG_INVERSE, 1u << 4);
ABI_VALUE(GHOSTTY_STYLE_FLAG_INVISIBLE, 1u << 5);
ABI_VALUE(GHOSTTY_STYLE_FLAG_STRIKETHROUGH, 1u << 6);
ABI_VALUE(GHOSTTY_STYLE_FLAG_OVERLINE, 1u << 7);
ABI_VALUE(GHOSTTY_STYLE_UNDERLINE_SHIFT, 8);
ABI_VALUE(GHOSTTY_STYLE_UNDERLINE_MASK, 0x700);
ABI_VALUE(GHOSTTY_SGR_UNDERLINE_NONE, 0);
ABI_VALUE(GHOSTTY_SGR_UNDERLINE_SINGLE, 1);
ABI_VALUE(GHOSTTY_SGR_UNDERLINE_DOUBLE, 2);
ABI_VALUE(GHOSTTY_SGR_UNDERLINE_CURLY, 3);
ABI_VALUE(GHOSTTY_SGR_UNDERLINE_DOTTED, 4);
ABI_VALUE(GHOSTTY_SGR_UNDERLINE_DASHED, 5);
ABI_OFFSET(GhosttyStyle, fg_color, 0);
ABI_OFFSET(GhosttyStyle, bg_color, 4);
ABI_OFFSET(GhosttyStyle, underline_color, 8);
ABI_OFFSET(GhosttyStyle, flags, 12);
ABI_OFFSET(GhosttyStyle, reserved, 14);
ABI_LAYOUT(GhosttyRenderStateRowId, 16, 8);
ABI_OFFSET(GhosttyRenderStateRowId, bits, 0);
ABI_LAYOUT(GhosttyBuffer, 24, 8);
ABI_OFFSET(GhosttyBuffer, ptr, 0);
ABI_OFFSET(GhosttyBuffer, cap, 8);
ABI_OFFSET(GhosttyBuffer, len, 16);
ABI_LAYOUT(GhosttyCell, 8, 8);
ABI_LAYOUT(GhosttyRow, 8, 8);
ABI_LAYOUT(GhosttyCursorStyle, 4, 4);
ABI_LAYOUT(GhosttyCellWide, 4, 4);
ABI_LAYOUT(bool, 1, 1);
ABI_LAYOUT(GhosttyString, 16, 8);
ABI_OFFSET(GhosttyString, len, 8);
ABI_LAYOUT(GhosttyMousePosition, 8, 4);
ABI_OFFSET(GhosttyMousePosition, y, 4);
ABI_LAYOUT(GhosttyMouseEncoderSize, 40, 8);
ABI_OFFSET(GhosttyMouseEncoderSize, screen_width, 8);
ABI_OFFSET(GhosttyMouseEncoderSize, padding_left, 36);
ABI_LAYOUT(GhosttyMods, 2, 2);
ABI_VALUE(GHOSTTY_OUT_OF_SPACE, -3);
ABI_VALUE(GHOSTTY_INVALID_VALUE, -2);
ABI_VALUE(GHOSTTY_NO_VALUE, -4);
ABI_LAYOUT(GhosttyGridRef, 16, 8);
ABI_OFFSET(GhosttyGridRef, node, 0);
ABI_OFFSET(GhosttyGridRef, x, 8);
ABI_OFFSET(GhosttyGridRef, y, 10);
ABI_LAYOUT(GhosttySelection, 40, 8);
ABI_OFFSET(GhosttySelection, start, 0);
ABI_OFFSET(GhosttySelection, end, 16);
ABI_OFFSET(GhosttySelection, rectangle, 32);
ABI_VALUE(GHOSTTY_TERMINAL_OPT_SELECTION, 22);
ABI_VALUE(GHOSTTY_TERMINAL_DATA_SELECTION, 31);
ABI_VALUE(GHOSTTY_FORMATTER_FORMAT_PLAIN, 0);
ABI_VALUE(GHOSTTY_FORMATTER_OPT_EMIT, 1);
ABI_VALUE(GHOSTTY_FORMATTER_OPT_UNWRAP, 2);
ABI_VALUE(GHOSTTY_FORMATTER_OPT_TRIM, 3);
ABI_VALUE(GHOSTTY_FORMATTER_OPT_SELECTION, 4);
ABI_LAYOUT(GhosttySelectionBuffer, 24, 8);
ABI_OFFSET(GhosttySelectionBuffer, cap, 8);
ABI_OFFSET(GhosttySelectionBuffer, len, 16);
ABI_VALUE(GHOSTTY_SEARCH_STATUS_RUNNING, 0);
ABI_VALUE(GHOSTTY_SEARCH_STATUS_FEED_REQUIRED, 1);
ABI_VALUE(GHOSTTY_SEARCH_STATUS_COMPLETE, 2);
ABI_VALUE(GHOSTTY_SEARCH_OPT_NEEDLE, 1);
ABI_VALUE(GHOSTTY_SEARCH_DATA_MATCHES, 6);
ABI_VALUE(GHOSTTY_POINT_TAG_ACTIVE, 0);
ABI_VALUE(GHOSTTY_POINT_TAG_VIEWPORT, 1);
ABI_VALUE(GHOSTTY_POINT_TAG_SCREEN, 2);
ABI_VALUE(GHOSTTY_POINT_TAG_HISTORY, 3);
ABI_LAYOUT(GhosttyPointCoordinate, 8, 4);
ABI_OFFSET(GhosttyPointCoordinate, x, 0);
ABI_OFFSET(GhosttyPointCoordinate, y, 4);
ABI_LAYOUT(GhosttyPointValue, 16, 8);
ABI_OFFSET(GhosttyPointValue, coordinate, 0);
ABI_LAYOUT(GhosttyPoint, 24, 8);
ABI_OFFSET(GhosttyPoint, tag, 0);
ABI_OFFSET(GhosttyPoint, value, 8);
ABI_VALUE(GHOSTTY_KEY_ACTION_RELEASE, 0);
ABI_VALUE(GHOSTTY_KEY_ACTION_PRESS, 1);
ABI_VALUE(GHOSTTY_KEY_ACTION_REPEAT, 2);
ABI_VALUE(GHOSTTY_KEY_UNIDENTIFIED, 0);
ABI_VALUE(GHOSTTY_KEY_C, 22);
ABI_VALUE(GHOSTTY_KEY_BACKSPACE, 53);
ABI_VALUE(GHOSTTY_KEY_ENTER, 58);
ABI_VALUE(GHOSTTY_KEY_TAB, 64);
ABI_VALUE(GHOSTTY_KEY_ARROW_DOWN, 75);
ABI_VALUE(GHOSTTY_KEY_ARROW_LEFT, 76);
ABI_VALUE(GHOSTTY_KEY_ARROW_RIGHT, 77);
ABI_VALUE(GHOSTTY_KEY_ARROW_UP, 78);
ABI_VALUE(GHOSTTY_KEY_ESCAPE, 120);
ABI_VALUE(GHOSTTY_KEY_PASTE, 175);
ABI_VALUE(GHOSTTY_KEY_ENCODER_OPT_MACOS_OPTION_AS_ALT, 7);
ABI_VALUE(GHOSTTY_OPTION_AS_ALT_FALSE, 0);
ABI_VALUE(GHOSTTY_OPTION_AS_ALT_TRUE, 1);
ABI_VALUE(GHOSTTY_MODS_SHIFT, 1);
ABI_VALUE(GHOSTTY_MODS_CTRL, 2);
ABI_VALUE(GHOSTTY_MODS_ALT, 4);
ABI_VALUE(GHOSTTY_MODS_SUPER, 8);
ABI_VALUE(GHOSTTY_MODS_CAPS_LOCK, 16);
ABI_VALUE(GHOSTTY_MODS_NUM_LOCK, 32);
ABI_VALUE(GHOSTTY_MOUSE_ACTION_PRESS, 0);
ABI_VALUE(GHOSTTY_MOUSE_ACTION_RELEASE, 1);
ABI_VALUE(GHOSTTY_MOUSE_ACTION_MOTION, 2);
ABI_VALUE(GHOSTTY_MOUSE_BUTTON_LEFT, 1);
ABI_VALUE(GHOSTTY_MOUSE_BUTTON_RIGHT, 2);
ABI_VALUE(GHOSTTY_MOUSE_BUTTON_MIDDLE, 3);
ABI_VALUE(GHOSTTY_MOUSE_BUTTON_FOUR, 4);
ABI_VALUE(GHOSTTY_MOUSE_BUTTON_FIVE, 5);
ABI_VALUE(GHOSTTY_MOUSE_BUTTON_SIX, 6);
ABI_VALUE(GHOSTTY_MOUSE_BUTTON_SEVEN, 7);
ABI_VALUE(GHOSTTY_MOUSE_BUTTON_EIGHT, 8);
ABI_VALUE(GHOSTTY_MOUSE_BUTTON_NINE, 9);
ABI_VALUE(GHOSTTY_MOUSE_BUTTON_TEN, 10);
ABI_VALUE(GHOSTTY_MOUSE_BUTTON_ELEVEN, 11);
ABI_VALUE(GHOSTTY_MOUSE_ENCODER_OPT_SIZE, 3);
ABI_VALUE(GHOSTTY_MOUSE_ENCODER_OPT_ANY_BUTTON_PRESSED, 4);
ABI_LAYOUT(GhosttyTerminalModeConfig, 4, 2);
ABI_OFFSET(GhosttyTerminalModeConfig, value, 2);
ABI_LAYOUT(GhosttyTerminalScrollbar, 24, 8);
ABI_OFFSET(GhosttyTerminalScrollbar, offset, 8);
ABI_OFFSET(GhosttyTerminalScrollbar, len, 16);
ABI_LAYOUT(GhosttyTerminalScrollViewport, 24, 8);
ABI_OFFSET(GhosttyTerminalScrollViewport, value, 8);
ABI_VALUE(GHOSTTY_TERMINAL_DATA_ACTIVE_SCREEN, 6);
ABI_VALUE(GHOSTTY_TERMINAL_DATA_KITTY_KEYBOARD_FLAGS, 8);
ABI_VALUE(GHOSTTY_TERMINAL_DATA_SCROLLBAR, 9);
ABI_VALUE(GHOSTTY_TERMINAL_DATA_MODE, 37);
ABI_VALUE(GHOSTTY_TERMINAL_SCREEN_PRIMARY, 0);
ABI_VALUE(GHOSTTY_TERMINAL_SCREEN_ALTERNATE, 1);
ABI_VALUE(GHOSTTY_TERMINAL_SCROLL_VIEWPORT_TOP, 0);
ABI_VALUE(GHOSTTY_TERMINAL_SCROLL_VIEWPORT_BOTTOM, 1);
ABI_VALUE(GHOSTTY_TERMINAL_SCROLL_VIEWPORT_DELTA, 2);
ABI_VALUE(GHOSTTY_TERMINAL_SCROLL_VIEWPORT_ROW, 3);
ABI_LAYOUT(GhosttyString, 16, 8);
ABI_OFFSET(GhosttyString, ptr, 0);
ABI_OFFSET(GhosttyString, len, 8);
ABI_VALUE(GHOSTTY_TERMINAL_OPT_BELL, 3);
ABI_VALUE(GHOSTTY_TERMINAL_OPT_TITLE_CHANGED, 6);
ABI_VALUE(GHOSTTY_TERMINAL_OPT_PWD_CHANGED, 26);
ABI_VALUE(GHOSTTY_TERMINAL_DATA_TITLE, 12);
ABI_VALUE(GHOSTTY_TERMINAL_DATA_PWD, 13);
ABI_VALUE(GHOSTTY_TERMINAL_DATA_COLS, 1);
ABI_VALUE(GHOSTTY_TERMINAL_DATA_ROWS, 2);
ABI_VALUE(GHOSTTY_OUT_OF_SPACE, -3);
ABI_VALUE(GHOSTTY_NO_VALUE, -4);
ABI_VALUE(GHOSTTY_STYLE_COLOR_NONE, 0);
ABI_VALUE(GHOSTTY_STYLE_COLOR_PALETTE, 1);
ABI_VALUE(GHOSTTY_STYLE_COLOR_RGB, 2);
ABI_VALUE(GHOSTTY_RENDER_STATE_DATA_COLS, 1);
ABI_VALUE(GHOSTTY_RENDER_STATE_DATA_ROWS, 2);
ABI_VALUE(GHOSTTY_RENDER_STATE_DATA_ROW_ITERATOR, 4);
ABI_VALUE(GHOSTTY_RENDER_STATE_DATA_COLOR_BACKGROUND, 5);
ABI_VALUE(GHOSTTY_RENDER_STATE_DATA_COLOR_FOREGROUND, 6);
ABI_VALUE(GHOSTTY_RENDER_STATE_DATA_COLOR_CURSOR, 7);
ABI_VALUE(GHOSTTY_RENDER_STATE_DATA_COLOR_PALETTE, 9);
ABI_VALUE(GHOSTTY_RENDER_STATE_DATA_CURSOR_VISUAL_STYLE, 10);
ABI_VALUE(GHOSTTY_RENDER_STATE_DATA_CURSOR_VISIBLE, 11);
ABI_VALUE(GHOSTTY_RENDER_STATE_DATA_CURSOR_BLINKING, 12);
ABI_VALUE(GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_HAS_VALUE, 14);
ABI_VALUE(GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_X, 15);
ABI_VALUE(GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_Y, 16);
ABI_VALUE(GHOSTTY_RENDER_STATE_ROW_DATA_RAW, 2);
ABI_VALUE(GHOSTTY_RENDER_STATE_ROW_DATA_CELLS, 3);
ABI_VALUE(GHOSTTY_RENDER_STATE_ROW_DATA_ID, 7);
ABI_VALUE(GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_RAW, 1);
ABI_VALUE(GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_STYLE, 2);
ABI_VALUE(GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_BG_COLOR, 5);
ABI_VALUE(GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_FG_COLOR, 6);
ABI_VALUE(GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_SELECTED, 7);
ABI_VALUE(GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_UTF8, 9);
ABI_VALUE(GHOSTTY_CELL_DATA_WIDE, 3);
ABI_VALUE(GHOSTTY_ROW_DATA_WRAP, 1);
ABI_VALUE(GHOSTTY_ROW_DATA_WRAP_CONTINUATION, 2);
ABI_VALUE(GHOSTTY_CELL_WIDE_NARROW, 0);
ABI_VALUE(GHOSTTY_CELL_WIDE_WIDE, 1);
ABI_VALUE(GHOSTTY_CELL_WIDE_SPACER_TAIL, 2);
ABI_VALUE(GHOSTTY_CELL_WIDE_SPACER_HEAD, 3);
ABI_VALUE(GHOSTTY_CURSOR_STYLE_BAR, 0);
ABI_VALUE(GHOSTTY_CURSOR_STYLE_BLOCK, 1);
ABI_VALUE(GHOSTTY_CURSOR_STYLE_UNDERLINE, 2);
ABI_VALUE(GHOSTTY_CURSOR_STYLE_BLOCK_HOLLOW, 3);

/* Keep the standalone Rust proof's narrow declarations matched to this ABI. */
_Static_assert(sizeof(GhosttyResult) == sizeof(int), "result ABI mismatch");
_Static_assert(GHOSTTY_SUCCESS == 0, "success discriminant mismatch");
_Static_assert(GHOSTTY_TERMINAL_OPT_USERDATA == 1, "userdata discriminant mismatch");
_Static_assert(GHOSTTY_TERMINAL_OPT_WRITE_PTY == 2, "reply discriminant mismatch");
_Static_assert(GHOSTTY_TERMINAL_DATA_CURSOR_X == 3, "cursor x discriminant mismatch");
_Static_assert(GHOSTTY_TERMINAL_DATA_CURSOR_Y == 4, "cursor y discriminant mismatch");
_Static_assert(GHOSTTY_TERMINAL_DATA_VT_PROCESSING_ERROR == 33, "processing error discriminant mismatch");

typedef struct {
    uint8_t bytes[4096];
    size_t len;
    int overflow;
} Replies;

static void require(int condition, const char *message) {
    if (!condition) {
        fprintf(stderr, "ghostty-vt probe: %s\n", message);
        exit(EXIT_FAILURE);
    }
}

static void write_reply(GhosttyTerminal terminal, void *userdata,
                        const uint8_t *data, size_t len) {
    (void)terminal;
    Replies *replies = userdata;
    if (len > sizeof(replies->bytes) - replies->len) {
        replies->overflow = 1;
        return;
    }
    memcpy(replies->bytes + replies->len, data, len);
    replies->len += len;
}

static GhosttyTerminal new_terminal(Replies *replies) {
    GhosttyTerminal terminal = NULL;
    require(ghostty_terminal_new(NULL, &terminal, 80, 24) == GHOSTTY_SUCCESS,
            "terminal allocation failed");
    require(terminal != NULL, "allocation returned a null handle");
    require(ghostty_terminal_set(terminal, GHOSTTY_TERMINAL_OPT_USERDATA,
                                 replies) == GHOSTTY_SUCCESS,
            "userdata registration failed");
    require(ghostty_terminal_set(terminal, GHOSTTY_TERMINAL_OPT_WRITE_PTY,
                                 write_reply) == GHOSTTY_SUCCESS,
            "reply callback registration failed");
    return terminal;
}

static uint16_t get_u16(GhosttyTerminal terminal, GhosttyTerminalData key) {
    uint16_t value = UINT16_MAX;
    require(ghostty_terminal_get(terminal, key, &value) == GHOSTTY_SUCCESS,
            "terminal query failed");
    return value;
}

static void check_screen(GhosttyTerminal terminal) {
    GhosttyRenderState state = NULL;
    GhosttyRenderStateRowIterator rows = NULL;
    GhosttyRenderStateRowCells cells = NULL;
    require(ghostty_render_state_new(NULL, &state) == GHOSTTY_SUCCESS,
            "render state allocation failed");
    require(ghostty_render_state_row_iterator_new(NULL, &rows) == GHOSTTY_SUCCESS,
            "row iterator allocation failed");
    require(ghostty_render_state_row_cells_new(NULL, &cells) == GHOSTTY_SUCCESS,
            "cell iterator allocation failed");
    require(ghostty_render_state_update(state, terminal) == GHOSTTY_SUCCESS,
            "render snapshot update failed");

    /* The snapshot must remain readable after terminal state changes. */
    static const uint8_t clear[] = "\x1b[2J";
    ghostty_terminal_vt_write(terminal, clear, sizeof(clear) - 1);
    require(ghostty_render_state_get(state,
                GHOSTTY_RENDER_STATE_DATA_ROW_ITERATOR, &rows) == GHOSTTY_SUCCESS,
            "row snapshot query failed");
    size_t y = 0;
    size_t checked = 0;
    while (ghostty_render_state_row_iterator_next(rows)) {
        require(ghostty_render_state_row_get(rows,
                    GHOSTTY_RENDER_STATE_ROW_DATA_CELLS, &cells) == GHOSTTY_SUCCESS,
                "cell snapshot query failed");
        size_t x = 0;
        while (ghostty_render_state_row_cells_next(cells)) {
            uint32_t expected = 0;
            if (y == 0 && x == 0) expected = 'A';
            if (y == 0 && x == 1) expected = 0x4e2d;
            if (y == 2 && x == 4) expected = 'x';
            if (y == 2 && x == 5) expected = 'y';
            if (expected != 0) {
                uint32_t codepoints[4] = {0};
                GhosttyCodepointBuffer grapheme = {
                    .ptr = codepoints, .cap = 4,
                };
                require(ghostty_render_state_row_cells_get(cells,
                            GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_BUF,
                            &grapheme) == GHOSTTY_SUCCESS,
                        "grapheme query failed");
                require(grapheme.len == 1 && codepoints[0] == expected,
                        "render snapshot text differs after split writes");
                GhosttyStyle style = {0};
                require(ghostty_render_state_row_cells_get(cells,
                            GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_STYLE,
                            &style) == GHOSTTY_SUCCESS, "style query failed");
                if (y == 0) {
                    require(style.fg_color.tag == GHOSTTY_STYLE_COLOR_PALETTE &&
                            style.fg_color.value.palette == 1,
                            "red SGR style was not retained in the snapshot");
                } else {
                    require(style.fg_color.tag == GHOSTTY_STYLE_COLOR_NONE,
                            "SGR reset was not retained in the snapshot");
                }
                ++checked;
            }
            ++x;
        }
        require(x == 80, "render snapshot has an unexpected row width");
        ++y;
    }
    require(y == 24 && checked == 4, "render snapshot is missing expected cells");
    ghostty_render_state_row_cells_free(cells);
    ghostty_render_state_row_iterator_free(rows);
    ghostty_render_state_free(state);
}

static void check_terminal(GhosttyTerminal terminal, const Replies *replies) {
    static const char expected[] = "\x1b[1;4R\x1b[3;7R";
    require(!replies->overflow, "reply capture exceeded its bound");
    require(replies->len == sizeof(expected) - 1 &&
            memcmp(replies->bytes, expected, sizeof(expected) - 1) == 0,
            "DSR reply did not reach the caller's byte queue");
    require(get_u16(terminal, GHOSTTY_TERMINAL_DATA_CURSOR_X) == 6,
            "cursor column differs after split CSI/UTF-8 writes");
    require(get_u16(terminal, GHOSTTY_TERMINAL_DATA_CURSOR_Y) == 2,
            "cursor row differs after split CSI/UTF-8 writes");
    bool ground = false;
    require(ghostty_terminal_get(terminal, GHOSTTY_TERMINAL_DATA_VT_GROUND,
                                 &ground) == GHOSTTY_SUCCESS && ground,
            "parser did not return to ground");
    bool processing_error = true;
    require(ghostty_terminal_get(terminal,
                                 GHOSTTY_TERMINAL_DATA_VT_PROCESSING_ERROR,
                                 &processing_error) == GHOSTTY_SUCCESS &&
            !processing_error, "VT processing reported an error");
    check_screen(terminal);
}

int main(void) {
    /* SGR, a split two-column UTF-8 scalar, CUP, text and DSR. */
    static const uint8_t stream[] =
        "\x1b[31mA\xe4\xb8\xad\x1b[0m\x1b[6n\x1b[3;5Hxy\x1b[6n";
    const size_t len = sizeof(stream) - 1;
    for (size_t split = 0; split <= len; ++split) {
        Replies replies = {0};
        GhosttyTerminal terminal = new_terminal(&replies);
        ghostty_terminal_vt_write(terminal, stream, split);
        ghostty_terminal_vt_write(terminal, stream + split, len - split);
        check_terminal(terminal, &replies);
        ghostty_terminal_free(terminal);
    }

    Replies replies = {0};
    GhosttyTerminal terminal = new_terminal(&replies);
    for (size_t i = 0; i < len; ++i) {
        ghostty_terminal_vt_write(terminal, stream + i, 1);
    }
    check_terminal(terminal, &replies);
    require(ghostty_terminal_resize(terminal, 100, 30, 9, 18) == GHOSTTY_SUCCESS,
            "resize failed");
    require(get_u16(terminal, GHOSTTY_TERMINAL_DATA_COLS) == 100 &&
            get_u16(terminal, GHOSTTY_TERMINAL_DATA_ROWS) == 30,
            "resize dimensions were not retained");
    ghostty_terminal_free(terminal);
    puts("ghostty-vt probe: split writes, replies, render snapshots/styles and resize passed");
    return EXIT_SUCCESS;
}
