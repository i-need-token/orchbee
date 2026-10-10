#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include <stdbool.h>
#include <stdio.h>
#include <string.h>

static bool write_all(HANDLE output, const char *bytes, size_t length) {
    size_t offset = 0;
    while (offset < length) {
        DWORD written = 0;
        const DWORD remaining = (DWORD)(length - offset);
        if (!WriteFile(output, bytes + offset, remaining, &written, NULL) || written == 0) {
            return false;
        }
        offset += written;
    }
    return true;
}

static bool write_text(HANDLE output, const char *text) {
    return write_all(output, text, strlen(text));
}

static bool write_numbered(HANDLE output, const char *prefix, unsigned int index) {
    char line[96];
    const int length = snprintf(
        line,
        sizeof(line),
        "%s-%03u|abcdefghijklmnopqrstuvwxyz0123456789\r\n",
        prefix,
        index
    );
    return length > 0 && (size_t)length < sizeof(line) &&
        write_all(output, line, (size_t)length);
}

static SHORT visible_rows(const CONSOLE_SCREEN_BUFFER_INFO *info) {
    return (SHORT)(info->srWindow.Bottom - info->srWindow.Top + 1);
}

static SHORT visible_columns(const CONSOLE_SCREEN_BUFFER_INFO *info) {
    return (SHORT)(info->srWindow.Right - info->srWindow.Left + 1);
}

static bool wait_for_rows(
    HANDLE output,
    SHORT expected_rows,
    CONSOLE_SCREEN_BUFFER_INFO *info
) {
    const ULONGLONG deadline = GetTickCount64() + 20000;
    do {
        if (!GetConsoleScreenBufferInfo(output, info)) {
            return false;
        }
        if (visible_rows(info) == expected_rows) {
            // ConPTY checks whether the cursor needs synchronization before taking the
            // console lock that reads geometry. A query already in flight can therefore
            // see the new size without taking that synchronization path. Begin a fresh
            // query after observing the resize; do not poll until the cursor matches.
            return GetConsoleScreenBufferInfo(output, info) &&
                visible_rows(info) == expected_rows;
        }
        Sleep(1);
    } while (GetTickCount64() < deadline);
    return false;
}

static bool write_cursor_report(
    HANDLE output,
    const char *phase,
    const CONSOLE_SCREEN_BUFFER_INFO *info
) {
    char line[96];
    const int length = snprintf(
        line,
        sizeof(line),
        "CURSOR-%s:%d,%d:%dx%d\r\n",
        phase,
        (int)info->dwCursorPosition.X,
        (int)info->dwCursorPosition.Y,
        (int)visible_columns(info),
        (int)visible_rows(info)
    );
    return length > 0 && (size_t)length < sizeof(line) &&
        write_all(output, line, (size_t)length);
}

static bool write_growth(HANDLE output, const char *prefix) {
    for (unsigned int index = 0; index < 31; ++index) {
        if (!write_numbered(output, prefix, index)) {
            return false;
        }
    }
    return true;
}

int main(void) {
    const HANDLE output = GetStdHandle(STD_OUTPUT_HANDLE);
    if (output == NULL || output == INVALID_HANDLE_VALUE) {
        return 10;
    }

    for (unsigned int index = 0; index < 400; ++index) {
        if (!write_numbered(output, "SEED", index)) {
            return 11;
        }
    }
    if (!write_text(output, "SEED-READY|\r\n\a")) {
        return 12;
    }

    CONSOLE_SCREEN_BUFFER_INFO info;
    if (!wait_for_rows(output, 37, &info)) {
        return 20;
    }
    if (!write_cursor_report(output, "GROW-1", &info) ||
        !write_growth(output, "GROW-A") ||
        !write_text(output, "PHASE-A-DONE|\r\n\a")) {
        return 21;
    }

    if (!wait_for_rows(output, 24, &info)) {
        return 30;
    }
    if (!write_cursor_report(output, "SHRINK", &info) ||
        !write_text(output, "SHRINK-DONE|\r\n\a")) {
        return 31;
    }

    if (!wait_for_rows(output, 37, &info)) {
        return 40;
    }
    if (!write_cursor_report(output, "GROW-2", &info) ||
        !write_growth(output, "GROW-B") ||
        !write_text(
            output,
            "\x1b_orchbee.agent.v3;"
            "eyJraW5kIjoiY29ycmVsYXRpb24iLCJ2ZXJzaW9uIjozfQ"
            "\x1b\\"
        ) ||
        !write_text(output, "PHASE-B-DONE|\r\n\a")) {
        return 41;
    }
    return 0;
}
