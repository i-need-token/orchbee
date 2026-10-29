#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <winternl.h>
#include <stdint.h>
#include <stddef.h>
#include <string.h>

enum { OB_PROCESS_LIMIT = 256, OB_TEXT_LIMIT = 4096 };
#define OB_CONSOLE_UNKNOWN 0xffffffffu

/* This fixed ABI is mirrored and asserted in process_query_windows.zig. */
struct ob_process_facts {
    uint64_t creation_time;
    uint32_t parent_pid;
    uint32_t image_length;
    uint32_t command_length;
    uint32_t directory_length;
    /* Console host PID with flag bits removed; 0 means no console. */
    uint32_t console_host;
    uint32_t reserved;
    uint16_t image[OB_TEXT_LIMIT];
    uint16_t command[OB_TEXT_LIMIT];
    uint16_t directory[OB_TEXT_LIMIT];
};
_Static_assert(offsetof(struct ob_process_facts, image) == 32, "process facts ABI");
_Static_assert(sizeof(struct ob_process_facts) == 24608, "process facts size");

/* A full fixed-capacity list or an explicit error, never a truncated list.
 * Return 1 for success, 2 for capacity, 0 for an unavailable OS observation. */
int ob_windows_job_process_ids(HANDLE job, uint32_t *ids, uint32_t *count) {
    struct {
        DWORD assigned;
        DWORD listed;
        ULONG_PTR ids[OB_PROCESS_LIMIT];
    } list = {0};
    if (!QueryInformationJobObject(job, JobObjectBasicProcessIdList,
            &list, sizeof(list), NULL)) {
        return GetLastError() == ERROR_MORE_DATA ? 2 : 0;
    }
    if (list.assigned > OB_PROCESS_LIMIT || list.listed != list.assigned) return 2;
    for (DWORD i = 0; i < list.listed; ++i) {
        if (list.ids[i] == 0 || list.ids[i] > UINT32_MAX) return 0;
        ids[i] = (uint32_t)list.ids[i];
    }
    *count = list.listed;
    return 1;
}

/* Return 1 with a pinned member handle, 2 when the member denies limited
 * query rights (an opaque row), or 0 when the PID no longer names a live member.
 * The caller retains every opened handle until the sample is complete so the
 * numeric PID cannot be recycled under a published row. */
int ob_windows_open_job_process(HANDLE job, uint32_t pid, HANDLE *out) {
    *out = NULL;
    const DWORD rights = PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE;
    HANDLE process = OpenProcess(rights | PROCESS_VM_READ, FALSE, pid);
    if (process == NULL) process = OpenProcess(rights, FALSE, pid);
    if (process == NULL) return GetLastError() == ERROR_ACCESS_DENIED ? 2 : 0;
    BOOL member = FALSE;
    if (!IsProcessInJob(process, job, &member) || !member ||
            WaitForSingleObject(process, 0) != WAIT_TIMEOUT) {
        CloseHandle(process);
        return 0;
    }
    *out = process;
    return 1;
}

typedef LONG (NTAPI *ob_query_process)(HANDLE, ULONG, void *, ULONG, ULONG *);
struct ob_basic_process {
    LONG exit_status;
    void *peb;
    ULONG_PTR affinity;
    LONG priority;
    ULONG_PTR pid;
    ULONG_PTR parent_pid;
};

static FARPROC ntdll_function(const char *name) {
    HMODULE module = GetModuleHandleW(L"ntdll.dll");
    return module == NULL ? NULL : GetProcAddress(module, name);
}

static ob_query_process process_query(void) {
    FARPROC address = ntdll_function("NtQueryInformationProcess");
    ob_query_process query = NULL;
    _Static_assert(sizeof(query) == sizeof(address), "function pointer ABI");
    memcpy(&query, &address, sizeof(query));
    return query;
}

static BOOL read_exact(HANDLE process, uintptr_t address, void *buffer, SIZE_T size) {
    SIZE_T read = 0;
    return address != 0 && address <= UINTPTR_MAX - size &&
        ReadProcessMemory(process, (const void *)address, buffer, size, &read) && read == size;
}

static BOOL read_pointer(HANDLE process, uintptr_t address, BOOL narrow, uintptr_t *value) {
    if (narrow) {
        uint32_t pointer = 0;
        if (!read_exact(process, address, &pointer, sizeof(pointer))) return FALSE;
        *value = pointer;
        return TRUE;
    }
    return read_exact(process, address, value, sizeof(*value));
}

/* Remote UNICODE_STRING is data, not a local pointer. Read a bounded copy and
 * compare the descriptor again. Concurrent mutation is unavailable, not a
 * truncated or reconstructed command. These reads never write remote memory. */
static uint32_t read_text(HANDLE process, uintptr_t address, BOOL narrow, uint16_t *out) {
    unsigned char before[16] = {0};
    unsigned char after[16] = {0};
    const SIZE_T descriptor_size = narrow ? 8 : 16;
    if (!read_exact(process, address, before, descriptor_size)) return UINT32_MAX;
    uint16_t bytes = 0, maximum = 0;
    uintptr_t buffer = 0;
    memcpy(&bytes, before, 2);
    memcpy(&maximum, before + 2, 2);
    if (narrow) {
        uint32_t pointer = 0;
        memcpy(&pointer, before + 4, 4);
        buffer = pointer;
    } else {
        memcpy(&buffer, before + 8, sizeof(buffer));
    }
    if ((bytes & 1) != 0 || bytes > maximum || bytes / 2 > OB_TEXT_LIMIT) return UINT32_MAX;
    if (bytes != 0 && !read_exact(process, buffer, out, bytes)) return UINT32_MAX;
    if (!read_exact(process, address, after, descriptor_size) ||
            memcmp(before, after, descriptor_size) != 0) return UINT32_MAX;
    return bytes / 2;
}

/* ProcessCommandLineInformation needs only PROCESS_QUERY_LIMITED_INFORMATION.
 * The kernel copies the text into this buffer; an oversized or failed query is
 * unavailable, never truncated. */
static uint32_t limited_command(ob_query_process query, HANDLE process, uint16_t *out) {
    union {
        UNICODE_STRING text;
        unsigned char bytes[sizeof(UNICODE_STRING) + OB_TEXT_LIMIT * 2];
    } buffer;
    if (query(process, 60, &buffer, sizeof(buffer), NULL) < 0) return UINT32_MAX;
    const uintptr_t first = (uintptr_t)buffer.bytes;
    const uintptr_t text = (uintptr_t)buffer.text.Buffer;
    const USHORT bytes = buffer.text.Length;
    if ((bytes & 1) != 0 || bytes / 2 > OB_TEXT_LIMIT) return UINT32_MAX;
    if (bytes == 0) return 0;
    if (text < first || text - first > sizeof(buffer.bytes) - bytes) return UINT32_MAX;
    memcpy(out, buffer.text.Buffer, bytes);
    return bytes / 2;
}

/* ProcessConsoleHostProcess also needs only limited rights. Its low two bits
 * are flags; the rest is the console host PID, or 0 without a console. */
static uint32_t console_host(ob_query_process query, HANDLE process) {
    ULONG_PTR value = 0;
    if (query(process, 49, &value, sizeof(value), NULL) < 0) return OB_CONSOLE_UNKNOWN;
    value &= ~(ULONG_PTR)3;
    return value < OB_CONSOLE_UNKNOWN ? (uint32_t)value : OB_CONSOLE_UNKNOWN;
}

/* The console of a live process only; an exited root has no current console. */
uint32_t ob_windows_console_host(HANDLE process) {
    if (WaitForSingleObject(process, 0) != WAIT_TIMEOUT) return OB_CONSOLE_UNKNOWN;
    ob_query_process query = process_query();
    return query == NULL ? OB_CONSOLE_UNKNOWN : console_host(query, process);
}

static void read_parameters(ob_query_process query, HANDLE process, struct ob_process_facts *facts) {
    struct ob_basic_process basic = {0};
    if (query(process, 0, &basic, sizeof(basic), NULL) < 0) return;
    if (basic.parent_pid > 0 && basic.parent_pid <= UINT32_MAX)
        facts->parent_pid = (uint32_t)basic.parent_pid;
    uintptr_t peb = (uintptr_t)basic.peb;
    BOOL narrow = sizeof(uintptr_t) == 4;
    if (!narrow) {
        ULONG_PTR wow64 = 0;
        if (query(process, 26, &wow64, sizeof(wow64), NULL) < 0) return;
        if (wow64 != 0) { peb = wow64; narrow = TRUE; }
    }
    /* Native/WOW64 layouts are confined here. CurrentDirectory is an internal
     * RTL layout, so absence is supported. No field is inferred from launch. */
    const uintptr_t parameters_offset = narrow ? 0x10 : 0x20;
    const uintptr_t command_offset = narrow ? 0x40 : 0x70;
    const uintptr_t directory_offset = narrow ? 0x24 : 0x38;
    uintptr_t parameters = 0, after = 0;
    if (peb == 0 || peb > UINTPTR_MAX - parameters_offset ||
            !read_pointer(process, peb + parameters_offset, narrow, &parameters) ||
            parameters == 0 || parameters > UINTPTR_MAX - command_offset) return;
    /* The limited query already supplied the command line where available. */
    const BOOL command = facts->command_length == UINT32_MAX;
    if (command) facts->command_length = read_text(process, parameters + command_offset, narrow, facts->command);
    facts->directory_length = read_text(process, parameters + directory_offset, narrow, facts->directory);
    if (!read_pointer(process, peb + parameters_offset, narrow, &after) || after != parameters) {
        if (command) facts->command_length = UINT32_MAX;
        facts->directory_length = UINT32_MAX;
    }
}

/* Return 1 with facts, 0 when the process exited, or 2 when a live process
 * withholds its creation time and therefore becomes an opaque row. The image,
 * command line, and console use limited rights; only the current directory and
 * the command-line fallback read process memory. Each failure is absence. */
int ob_windows_process_facts(HANDLE process, struct ob_process_facts *facts) {
    ZeroMemory(facts, sizeof(*facts));
    facts->image_length = UINT32_MAX;
    facts->command_length = UINT32_MAX;
    facts->directory_length = UINT32_MAX;
    facts->console_host = OB_CONSOLE_UNKNOWN;
    FILETIME created = {0}, exited = {0}, kernel = {0}, user = {0};
    if (!GetProcessTimes(process, &created, &exited, &kernel, &user))
        return WaitForSingleObject(process, 0) == WAIT_TIMEOUT ? 2 : 0;
    facts->creation_time = ((uint64_t)created.dwHighDateTime << 32) | created.dwLowDateTime;
    if (facts->creation_time == 0) return 2;
    DWORD length = OB_TEXT_LIMIT;
    if (QueryFullProcessImageNameW(process, 0, (WCHAR *)facts->image, &length))
        facts->image_length = length;
    ob_query_process query = process_query();
    if (query != NULL) {
        facts->command_length = limited_command(query, process, facts->command);
        facts->console_host = console_host(query, process);
        read_parameters(query, process, facts);
    }
    return WaitForSingleObject(process, 0) == WAIT_TIMEOUT ? 1 : 0;
}

/* SystemProcessInformation as declared by winternl.h, with its reserved fields
 * named. It lists live processes without opening them. */
struct ob_system_process {
    ULONG next;
    ULONG threads;
    LARGE_INTEGER reserved[3];
    LARGE_INTEGER create_time;
    LARGE_INTEGER user_time;
    LARGE_INTEGER kernel_time;
    UNICODE_STRING image_name;
    LONG base_priority;
    HANDLE pid;
    HANDLE parent_pid;
};
#if defined(_WIN64)
_Static_assert(offsetof(struct ob_system_process, create_time) == 32, "system process ABI");
_Static_assert(offsetof(struct ob_system_process, pid) == 80, "system process ABI");
_Static_assert(offsetof(struct ob_system_process, parent_pid) == 88, "system process ABI");
#endif

typedef LONG (NTAPI *ob_query_system)(ULONG, void *, ULONG, ULONG *);

/* Identity for members that deny limited rights. One system snapshot supplies
 * creation time and parent PID for the requested PIDs only; everything else in
 * the snapshot is discarded. A PID absent from the snapshot keeps creation time
 * 0, meaning exited. Return FALSE on an OS failure. */
BOOL ob_windows_process_identities(
        const uint32_t *pids, uint32_t count, uint64_t *creation, uint32_t *parent) {
    FARPROC address = ntdll_function("NtQuerySystemInformation");
    ob_query_system query = NULL;
    _Static_assert(sizeof(query) == sizeof(address), "function pointer ABI");
    memcpy(&query, &address, sizeof(query));
    if (query == NULL) return FALSE;
    for (uint32_t i = 0; i < count; ++i) {
        creation[i] = 0;
        parent[i] = 0;
    }
    ULONG size = 1u << 20;
    for (int attempt = 0; attempt < 4; ++attempt) {
        unsigned char *buffer = HeapAlloc(GetProcessHeap(), 0, size);
        if (buffer == NULL) return FALSE;
        ULONG needed = 0;
        const LONG status = query(5, buffer, size, &needed);
        if (status == (LONG)0xC0000004L) {
            HeapFree(GetProcessHeap(), 0, buffer);
            if (size >= (1u << 27)) return FALSE;
            size = needed > size ? needed + (1u << 16) : size * 2;
            continue;
        }
        if (status < 0) {
            HeapFree(GetProcessHeap(), 0, buffer);
            return FALSE;
        }
        for (ULONG offset = 0; offset <= size - sizeof(struct ob_system_process);) {
            const struct ob_system_process *entry = (const void *)(buffer + offset);
            const ULONG_PTR pid = (ULONG_PTR)entry->pid;
            const ULONG_PTR ppid = (ULONG_PTR)entry->parent_pid;
            for (uint32_t i = 0; i < count; ++i) {
                if (pids[i] != pid) continue;
                creation[i] = (uint64_t)entry->create_time.QuadPart;
                parent[i] = ppid <= UINT32_MAX ? (uint32_t)ppid : 0;
            }
            if (entry->next == 0 || entry->next > size - offset) break;
            offset += entry->next;
        }
        HeapFree(GetProcessHeap(), 0, buffer);
        return TRUE;
    }
    return FALSE;
}
