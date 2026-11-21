#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/*
 * The programs that the handover black-box run moves between Hosts. The first argument names one:
 *
 *   lines         a numbered line, `line-N`, every timer tick
 *   flood         colored numbered lines, `flood-N`, as fast as the console takes them
 *   fill COUNT    COUNT lines as wide as the screen, each in eight colored runs
 *   screen        a styled full screen on the alternate screen, with mouse and paste modes on
 *   stall         the start of a color sequence, which it completes only once a key arrives
 *
 * `x` ends the output of `lines` and `flood`. Each state also shows in the window title, which the
 * Host reports in `TerminalMetadataChanged`: `lines-end N`, `flood-end N`, `fill-end N`,
 * `screen-ready`, `stall-begin`, and `stall-end`. Afterwards the program shows each key K it reads
 * as `key-K` and each new console size as `size-CxR`, in the title and on the screen, and `q` ends
 * it.
 */

static HANDLE input_handle;
static HANDLE output_handle;

static BOOL write_all(const char *bytes, size_t length) {
    while (length > 0) {
        DWORD written = 0;
        const DWORD chunk = length > 65536 ? 65536 : (DWORD)length;
        if (!WriteFile(output_handle, bytes, chunk, &written, NULL) || written == 0) return FALSE;
        bytes += written;
        length -= written;
    }
    return TRUE;
}

static BOOL write_text(const char *text) {
    return write_all(text, strlen(text));
}

static BOOL console_size(int *columns, int *rows) {
    CONSOLE_SCREEN_BUFFER_INFO info;
    if (!GetConsoleScreenBufferInfo(output_handle, &info)) return FALSE;
    *columns = info.srWindow.Right - info.srWindow.Left + 1;
    *rows = info.srWindow.Bottom - info.srWindow.Top + 1;
    return TRUE;
}

/* The next key typed within `timeout_ms`, or 0; other input records are read and dropped. */
static WCHAR next_key(DWORD timeout_ms) {
    for (;;) {
        if (WaitForSingleObject(input_handle, timeout_ms) != WAIT_OBJECT_0) return 0;
        INPUT_RECORD record;
        DWORD count = 0;
        if (!ReadConsoleInputW(input_handle, &record, 1, &count) || count == 0) return 0;
        if (record.EventType == KEY_EVENT && record.Event.KeyEvent.bKeyDown &&
            record.Event.KeyEvent.uChar.UnicodeChar != 0) {
            return record.Event.KeyEvent.uChar.UnicodeChar;
        }
        timeout_ms = 0;
    }
}

/* Shows a state in the title, and on the screen: as a line, or at `row` of a full screen. */
static BOOL show(const char *text, int row) {
    char line[128];
    if (row == 0) {
        snprintf(line, sizeof line, "%s\r\n", text);
    } else {
        snprintf(line, sizeof line, "\x1b[%d;3H\x1b[2K\x1b[1;35m%s\x1b[0m\x1b[20;30H", row, text);
    }
    return write_text(line) && SetConsoleTitleA(text);
}

static BOOL draw_screen(int columns, int rows) {
    char text[512];
    if (!write_text("\x1b[2J\x1b[1;1H\x1b[1;37;44m probe-screen \x1b[0m")) return FALSE;
    for (int row = 3; row <= 8; row += 1) {
        int length = snprintf(text, sizeof text, "\x1b[%d;3H", row);
        for (int cell = 0; cell < 16 && length < 400; cell += 1) {
            length += snprintf(text + length, sizeof text - length, "\x1b[48;5;%dm  ", (row - 3) * 16 + cell + 16);
        }
        snprintf(text + length, sizeof text - length, "\x1b[0m");
        if (!write_text(text)) return FALSE;
    }
    snprintf(text, sizeof text,
             "\x1b[10;3H\x1b[4munderlined\x1b[0m \x1b[3mitalic\x1b[0m \x1b[7minverse\x1b[0m "
             "\x1b[38;2;255;128;0mtruecolor\x1b[0m \x1b[1;32mbold green\x1b[0m"
             "\x1b[12;3H\x1b[1;35mkey-none\x1b[0m\x1b[13;3H\x1b[1;35msize-%dx%d\x1b[0m"
             "\x1b[%d;1H\x1b[2mlast row\x1b[0m\x1b[20;30H",
             columns, rows, rows);
    return write_text(text);
}

/* Shows each key and each new console size until `q`. */
static int report_input(BOOL full_screen) {
    int columns = 0;
    int rows = 0;
    if (!console_size(&columns, &rows)) return 2;
    for (;;) {
        const WCHAR key = next_key(50);
        int now_columns = 0;
        int now_rows = 0;
        if (console_size(&now_columns, &now_rows) && (now_columns != columns || now_rows != rows)) {
            char text[32];
            columns = now_columns;
            rows = now_rows;
            snprintf(text, sizeof text, "size-%dx%d", columns, rows);
            if (full_screen && !draw_screen(columns, rows)) return 2;
            if (!show(text, full_screen ? 13 : 0)) return 2;
        }
        if (key == L'q') {
            if (full_screen && !write_text("\x1b[?1006l\x1b[?1000l\x1b[?2004l\x1b[?1049l")) return 2;
            return 0;
        }
        if (key >= 0x21 && key < 0x7f) {
            char text[16];
            snprintf(text, sizeof text, "key-%c", (char)key);
            if (!show(text, full_screen ? 12 : 0)) return 2;
        }
    }
}

static int run_lines(void) {
    char text[64];
    unsigned long number = 0;
    for (;;) {
        number += 1;
        const int length = snprintf(text, sizeof text, "line-%lu\r\n", number);
        if (!write_all(text, (size_t)length)) return 2;
        /* The wait paces the lines at the timer's tick and takes the stop key. */
        if (next_key(1) == L'x') break;
    }
    snprintf(text, sizeof text, "lines-end %lu", number);
    if (!show(text, 0)) return 2;
    return report_input(FALSE);
}

static int run_flood(void) {
    static const char padding[] = "abcdefghijklmnopqrstuvwxyz0123456789";
    char text[128];
    unsigned long number = 0;
    for (;;) {
        number += 1;
        const int pad = 1 + (int)(number % 29);
        const int length = snprintf(text, sizeof text, "\x1b[3%lum\x1b[1mflood-%lu\x1b[22m %.*s\x1b[0m\r\n",
                                    number % 8, number, pad, padding);
        if (!write_all(text, (size_t)length)) return 2;
        if (number % 64 == 0 && next_key(0) == L'x') break;
    }
    snprintf(text, sizeof text, "flood-end %lu", number);
    if (!show(text, 0)) return 2;
    return report_input(FALSE);
}

static int run_fill(unsigned long count) {
    int columns = 0;
    int rows = 0;
    if (!console_size(&columns, &rows) || columns < 24) return 2;
    const int run = (columns - 1) / 8;
    /* Lines are written in batches of about 64 KiB, in eight colors, as a colored listing is. */
    const size_t batch = 65536;
    char *buffer = malloc(batch + (size_t)columns * 2 + 128);
    if (buffer == NULL) return 2;
    size_t used = 0;
    for (unsigned long number = 1; number <= count; number += 1) {
        for (int index = 0; index < 8; index += 1) {
            used += (size_t)sprintf(buffer + used, "\x1b[%dm", index == 0 ? 39 : 30 + index);
            const size_t start = used;
            if (index == 0) {
                used += (size_t)sprintf(buffer + used, "%08lu", number);
            } else {
                buffer[used++] = (char)('A' + index);
            }
            while (used - start < (size_t)run) buffer[used++] = (char)('a' + (number + index) % 26);
        }
        used += (size_t)sprintf(buffer + used, "\x1b[0m\r\n");
        if (used >= batch || number == count) {
            if (!write_all(buffer, used)) {
                free(buffer);
                return 2;
            }
            used = 0;
        }
    }
    free(buffer);
    char text[64];
    snprintf(text, sizeof text, "fill-end %lu", count);
    if (!show(text, 0)) return 2;
    return report_input(FALSE);
}

static int run_screen(void) {
    int columns = 0;
    int rows = 0;
    if (!console_size(&columns, &rows)) return 2;
    if (!write_text("\x1b[?1049h\x1b[?2004h\x1b[?1000h\x1b[?1006h") || !draw_screen(columns, rows) ||
        !SetConsoleTitleA("screen-ready")) {
        return 2;
    }
    return report_input(TRUE);
}

static int run_stall(void) {
    if (!show("stall-begin", 0) || !write_text("\x1b[3")) return 2;
    while (next_key(INFINITE) == 0) {
    }
    if (!write_text("1mstall-red\x1b[0m\r\n") || !show("stall-end", 0)) return 2;
    return report_input(FALSE);
}

int main(int argc, char **argv) {
    input_handle = GetStdHandle(STD_INPUT_HANDLE);
    output_handle = GetStdHandle(STD_OUTPUT_HANDLE);
    DWORD output_mode = 0;
    if (argc < 2 || !GetConsoleMode(output_handle, &output_mode) ||
        !SetConsoleMode(output_handle, output_mode | ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING) ||
        !SetConsoleMode(input_handle, ENABLE_WINDOW_INPUT)) {
        return 2;
    }
    if (strcmp(argv[1], "lines") == 0) return run_lines();
    if (strcmp(argv[1], "flood") == 0) return run_flood();
    if (strcmp(argv[1], "fill") == 0 && argc == 3) return run_fill(strtoul(argv[2], NULL, 10));
    if (strcmp(argv[1], "screen") == 0) return run_screen();
    if (strcmp(argv[1], "stall") == 0) return run_stall();
    return 2;
}
