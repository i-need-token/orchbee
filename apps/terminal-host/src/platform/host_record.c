/* The operating-system half of the Host's fatal record and diagnostic log. Every function that
 * runs after a fault (write, clock, process id, stderr, wait, exit) takes no lock, allocates
 * nothing and calls no Zig code, so a crashing thread can still use it. */
#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#else
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#if defined(__APPLE__)
#include <mach-o/dyld.h>
#include <mach-o/loader.h>
#include <string.h>
#else
#include <link.h>
#endif
#endif
#include <stdint.h>

int ob_host_record_write(
    const void *path,
    const uint8_t *bytes,
    uint32_t length,
    int append,
    uint32_t maximum_bytes
);
void ob_host_record_capture_executable(uintptr_t *base, uintptr_t *end);
uint64_t ob_host_record_unix_milliseconds(void);
uint32_t ob_host_record_process_id(void);
int ob_host_record_stderr_attached(void);
void ob_host_record_write_stderr(const uint8_t *bytes, uint32_t length);
void ob_host_record_wait_milliseconds(uint32_t milliseconds);
void ob_host_record_exit(uint32_t code);

/* Replaces the file at `path`, or appends to it; an append that would take the file past
 * `maximum_bytes` starts the file over instead. `path` is NUL-terminated UTF-16 on Windows and
 * NUL-terminated bytes elsewhere. Returns 1 when every byte was written. */
int ob_host_record_write(
    const void *path,
    const uint8_t *bytes,
    uint32_t length,
    int append,
    uint32_t maximum_bytes
) {
#if defined(_WIN32)
    const wchar_t *wide_path = path;
    HANDLE file = CreateFileW(
        wide_path,
        append ? FILE_APPEND_DATA : GENERIC_WRITE,
        FILE_SHARE_READ,
        NULL,
        append ? OPEN_ALWAYS : CREATE_ALWAYS,
        FILE_ATTRIBUTE_NORMAL,
        NULL
    );
    if (file == INVALID_HANDLE_VALUE) return 0;
    if (append) {
        LARGE_INTEGER size;
        if (!GetFileSizeEx(file, &size)) {
            CloseHandle(file);
            return 0;
        }
        if ((uint64_t)size.QuadPart + length > maximum_bytes) {
            CloseHandle(file);
            file = CreateFileW(
                wide_path,
                GENERIC_WRITE,
                FILE_SHARE_READ,
                NULL,
                CREATE_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                NULL
            );
            if (file == INVALID_HANDLE_VALUE) return 0;
        }
    }
    DWORD written = 0;
    const BOOL complete = WriteFile(file, bytes, length, &written, NULL) && written == length;
    CloseHandle(file);
    return complete ? 1 : 0;
#else
    const int flags = O_WRONLY | O_CREAT | O_CLOEXEC | O_NOFOLLOW | (append ? O_APPEND : O_TRUNC);
    const int file = open((const char *)path, flags, 0600);
    if (file < 0) return 0;
    if (append) {
        struct stat status;
        if (fstat(file, &status) != 0) {
            close(file);
            return 0;
        }
        if ((uint64_t)status.st_size + length > maximum_bytes && ftruncate(file, 0) != 0) {
            close(file);
            return 0;
        }
    }
    uint32_t offset = 0;
    while (offset < length) {
        const ssize_t written = write(file, bytes + offset, length - offset);
        if (written < 0 && errno == EINTR) continue;
        if (written <= 0) break;
        offset += (uint32_t)written;
    }
    close(file);
    return offset == length ? 1 : 0;
#endif
}

#if !defined(_WIN32) && !defined(__APPLE__)
struct ObExecutableRange {
    uintptr_t base;
    uintptr_t end;
};

/* The first object `dl_iterate_phdr` reports is the executable itself. */
static int ob_capture_first_object(struct dl_phdr_info *info, size_t size, void *data) {
    (void)size;
    struct ObExecutableRange *range = data;
    uintptr_t end = 0;
    for (ElfW(Half) index = 0; index < info->dlpi_phnum; index += 1) {
        const ElfW(Phdr) *header = &info->dlpi_phdr[index];
        if (header->p_type != PT_LOAD) continue;
        const uintptr_t segment_end = info->dlpi_addr + header->p_vaddr + header->p_memsz;
        if (segment_end > end) end = segment_end;
    }
    range->base = info->dlpi_addr;
    range->end = end;
    return 1;
}
#endif

/* Reports where this executable's image lies, so a fault address can be named as an offset into
 * it. Called once before any record can be written; a range it cannot find stays empty. */
void ob_host_record_capture_executable(uintptr_t *base, uintptr_t *end) {
    *base = 0;
    *end = 0;
#if defined(_WIN32)
    const uintptr_t image = (uintptr_t)GetModuleHandleW(NULL);
    if (image == 0) return;
    const IMAGE_DOS_HEADER *dos = (const IMAGE_DOS_HEADER *)image;
    if (dos->e_magic != IMAGE_DOS_SIGNATURE) return;
    const IMAGE_NT_HEADERS *nt = (const IMAGE_NT_HEADERS *)(image + (uintptr_t)dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE) return;
    *base = image;
    *end = image + nt->OptionalHeader.SizeOfImage;
#elif defined(__APPLE__)
    const struct mach_header_64 *header = (const struct mach_header_64 *)_dyld_get_image_header(0);
    if (header == NULL || header->magic != MH_MAGIC_64) return;
    const uint8_t *command = (const uint8_t *)(header + 1);
    for (uint32_t index = 0; index < header->ncmds; index += 1) {
        const struct load_command *load = (const struct load_command *)command;
        if (load->cmd == LC_SEGMENT_64) {
            const struct segment_command_64 *segment = (const struct segment_command_64 *)command;
            if (strncmp(segment->segname, "__TEXT", sizeof(segment->segname)) == 0) {
                *base = (uintptr_t)header;
                *end = (uintptr_t)header + segment->vmsize;
                return;
            }
        }
        command += load->cmdsize;
    }
#else
    struct ObExecutableRange range = {0, 0};
    dl_iterate_phdr(ob_capture_first_object, &range);
    *base = range.base;
    *end = range.end;
#endif
}

uint64_t ob_host_record_unix_milliseconds(void) {
#if defined(_WIN32)
    FILETIME now;
    GetSystemTimeAsFileTime(&now);
    const uint64_t ticks = ((uint64_t)now.dwHighDateTime << 32) | now.dwLowDateTime;
    /* 100 ns ticks since 1601-01-01; the Unix epoch is 11644473600 s later. */
    const uint64_t unix_epoch_ticks = 116444736000000000ULL;
    return ticks < unix_epoch_ticks ? 0 : (ticks - unix_epoch_ticks) / 10000;
#else
    struct timespec now;
    if (clock_gettime(CLOCK_REALTIME, &now) != 0 || now.tv_sec < 0) return 0;
    return (uint64_t)now.tv_sec * 1000 + (uint64_t)now.tv_nsec / 1000000;
#endif
}

uint32_t ob_host_record_process_id(void) {
#if defined(_WIN32)
    return GetCurrentProcessId();
#else
    return (uint32_t)getpid();
#endif
}

/* Whether standard error leads anywhere; a Host its Worker started has none. */
int ob_host_record_stderr_attached(void) {
#if defined(_WIN32)
    const HANDLE handle = GetStdHandle(STD_ERROR_HANDLE);
    return handle != NULL && handle != INVALID_HANDLE_VALUE;
#else
    return fcntl(STDERR_FILENO, F_GETFD) != -1;
#endif
}

void ob_host_record_write_stderr(const uint8_t *bytes, uint32_t length) {
#if defined(_WIN32)
    const HANDLE handle = GetStdHandle(STD_ERROR_HANDLE);
    if (handle == NULL || handle == INVALID_HANDLE_VALUE) return;
    DWORD written = 0;
    (void)WriteFile(handle, bytes, length, &written, NULL);
#else
    uint32_t offset = 0;
    while (offset < length) {
        const ssize_t written = write(STDERR_FILENO, bytes + offset, length - offset);
        if (written < 0 && errno == EINTR) continue;
        if (written <= 0) return;
        offset += (uint32_t)written;
    }
#endif
}

void ob_host_record_wait_milliseconds(uint32_t milliseconds) {
#if defined(_WIN32)
    Sleep(milliseconds);
#else
    struct timespec remaining = {
        .tv_sec = milliseconds / 1000,
        .tv_nsec = (long)(milliseconds % 1000) * 1000000L,
    };
    while (nanosleep(&remaining, &remaining) != 0 && errno == EINTR) {}
#endif
}

/* Ends the process at once: no exit handlers or library teardown run on a faulted process. On
 * Windows the closed handles still close the Host's kill-on-close Job, which ends every program. */
void ob_host_record_exit(uint32_t code) {
#if defined(_WIN32)
    (void)TerminateProcess(GetCurrentProcess(), code);
    ExitProcess(code);
#else
    _exit((int)(code & 0xff));
#endif
}
