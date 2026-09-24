#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>
#include <string.h>
#include <wchar.h>

/* A Job-owned child whose user-code readiness and lifetime do not depend on
 * shell startup, console output, network utilities or an elapsed sleep. The
 * test owns both processes through the production kill-on-close Job. */
int main(int argc, char **argv) {
    if (argc == 4 && strcmp(argv[1], "child") == 0) {
        HANDLE begin = OpenEventA(SYNCHRONIZE, FALSE, argv[2]);
        if (begin == NULL) return 2;
        HANDLE ready = OpenEventA(EVENT_MODIFY_STATE, FALSE, argv[3]);
        if (ready == NULL) {
            CloseHandle(begin);
            return 2;
        }
        const BOOL started = WaitForSingleObject(begin, INFINITE) == WAIT_OBJECT_0;
        CloseHandle(begin);
        const BOOL announced = started && SetEvent(ready);
        CloseHandle(ready);
        if (!announced) return 2;
        Sleep(INFINITE);
        return 3;
    }
    if (argc != 3) return 2;
    wchar_t begin[128], ready[128];
    if (!MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, argv[1], -1, begin, 128) ||
        !MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, argv[2], -1, ready, 128)) return 2;
    wchar_t executable[32768], command[32768];
    const DWORD length = GetModuleFileNameW(NULL, executable, 32768);
    if (length == 0 || length >= 32768) return 2;
    const int written = swprintf(command, 32768, L"\"%ls\" child \"%ls\" \"%ls\"", executable, begin, ready);
    if (written < 0 || written >= 32768) return 2;
    STARTUPINFOW startup = {.cb = sizeof(startup)};
    PROCESS_INFORMATION child = {0};
    if (!CreateProcessW(executable, command, NULL, NULL, FALSE, 0, NULL, NULL, &startup, &child)) return 2;
    CloseHandle(child.hThread);
    /* The production Job, not this handle, owns descendant termination. */
    CloseHandle(child.hProcess);
    Sleep(INFINITE);
    return 3;
}
