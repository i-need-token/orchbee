#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stdio.h>

typedef struct {
    HANDLE input;
    uint8_t bytes[64];
    DWORD length;
    BOOL complete;
} FocusInput;

static DWORD WINAPI read_focus_line(void *argument) {
    FocusInput *input = argument;
    while (input->length < sizeof(input->bytes)) {
        uint8_t byte = 0;
        DWORD received = 0;
        if (!ReadFile(input->input, &byte, 1, &received, NULL) || received != 1) return 1;
        if (byte == '\r' || byte == '\n') {
            input->complete = TRUE;
            return 0;
        }
        input->bytes[input->length++] = byte;
    }
    return 1;
}

int main(void) {
    const HANDLE input = GetStdHandle(STD_INPUT_HANDLE);
    const HANDLE output = GetStdHandle(STD_OUTPUT_HANDLE);
    DWORD input_mode = 0;
    DWORD output_mode = 0;
    if (!GetConsoleMode(input, &input_mode) || !GetConsoleMode(output, &output_mode)) return 2;
    int result = 2;
    HANDLE reader = NULL;
    FocusInput captured = {.input = input};
    // Cooked console reads discard focus events. VT input exposes the real
    // CSI bytes, including the initial report of the unfocused state.
    if (!SetConsoleMode(input, ENABLE_VIRTUAL_TERMINAL_INPUT) ||
        !SetConsoleMode(output, output_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) ||
        !FlushConsoleInputBuffer(input)) goto finished;
    reader = CreateThread(NULL, 0, read_focus_line, &captured, 0, NULL);
    if (reader == NULL) goto finished;
    if (fputs("\x1b[?1004hfocus-ready", stdout) == EOF || fflush(stdout) != 0) goto finished;
    if (WaitForSingleObject(reader, 5000) != WAIT_OBJECT_0 || !captured.complete) goto finished;
    if (fputs("focus-bytes:", stdout) == EOF) goto finished;
    for (DWORD index = 0; index < captured.length; index += 1) {
        if (printf("%02x", captured.bytes[index]) < 0) goto finished;
    }
    if (fputs(":end", stdout) == EOF || fflush(stdout) != 0) goto finished;
    result = 0;
finished:
    if (reader != NULL) {
        if (WaitForSingleObject(reader, 0) != WAIT_OBJECT_0) {
            (void)CancelSynchronousIo(reader);
            // The stack buffer remains owned until the reader has stopped.
            // Failure to cancel terminates this fixture process, never a peer.
            if (WaitForSingleObject(reader, 2000) != WAIT_OBJECT_0) ExitProcess(2);
        }
        if (!CloseHandle(reader)) result = 2;
    }
    if (!SetConsoleMode(input, input_mode)) result = 2;
    if (!SetConsoleMode(output, output_mode)) result = 2;
    return result;
}
