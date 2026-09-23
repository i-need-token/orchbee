#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stddef.h>
#include <string.h>

enum { OB_PROCESS_LIMIT = 64, OB_TEXT_LIMIT = 4096 };

/* This fixed ABI is mirrored and asserted in process_query_windows.zig. */
struct ob_process_facts {
    uint64_t creation_time;
    uint32_t parent_pid;
    uint32_t image_length;
    uint32_t command_length;
    uint32_t directory_length;
    uint16_t image[OB_TEXT_LIMIT];
    uint16_t command[OB_TEXT_LIMIT];
    uint16_t directory[OB_TEXT_LIMIT];
};
_Static_assert(offsetof(struct ob_process_facts, image) == 24, "process facts ABI");
_Static_assert(sizeof(struct ob_process_facts) == 24600, "process facts size");

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

/* The caller retains every opened handle until the final membership check.
 * Retention pins process object identity; closing between passes would allow
 * the numeric PID to be recycled and invalidate that comparison. */
HANDLE ob_windows_open_job_process(HANDLE job, uint32_t pid) {
    const DWORD rights = PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE;
    HANDLE process = OpenProcess(rights | PROCESS_VM_READ, FALSE, pid);
    if (process == NULL) process = OpenProcess(rights, FALSE, pid);
    if (process == NULL) return NULL;
    BOOL member = FALSE;
    if (!IsProcessInJob(process, job, &member) || !member ||
            WaitForSingleObject(process, 0) != WAIT_TIMEOUT) {
        CloseHandle(process);
        return NULL;
    }
    return process;
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

static void read_parameters(HANDLE process, struct ob_process_facts *facts) {
    HMODULE module = GetModuleHandleW(L"ntdll.dll");
    if (module == NULL) return;
    FARPROC address = GetProcAddress(module, "NtQueryInformationProcess");
    ob_query_process query = NULL;
    _Static_assert(sizeof(query) == sizeof(address), "function pointer ABI");
    memcpy(&query, &address, sizeof(query));
    if (query == NULL) return;
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
    facts->command_length = read_text(process, parameters + command_offset, narrow, facts->command);
    facts->directory_length = read_text(process, parameters + directory_offset, narrow, facts->directory);
    if (!read_pointer(process, peb + parameters_offset, narrow, &after) || after != parameters) {
        facts->command_length = UINT32_MAX;
        facts->directory_length = UINT32_MAX;
    }
}

BOOL ob_windows_process_facts(HANDLE process, struct ob_process_facts *facts) {
    ZeroMemory(facts, sizeof(*facts));
    facts->image_length = UINT32_MAX;
    facts->command_length = UINT32_MAX;
    facts->directory_length = UINT32_MAX;
    FILETIME created = {0}, exited = {0}, kernel = {0}, user = {0};
    if (!GetProcessTimes(process, &created, &exited, &kernel, &user)) return FALSE;
    facts->creation_time = ((uint64_t)created.dwHighDateTime << 32) | created.dwLowDateTime;
    if (facts->creation_time == 0) return FALSE;
    DWORD length = OB_TEXT_LIMIT;
    if (QueryFullProcessImageNameW(process, 0, (WCHAR *)facts->image, &length))
        facts->image_length = length;
    read_parameters(process, facts);
    return WaitForSingleObject(process, 0) == WAIT_TIMEOUT;
}
