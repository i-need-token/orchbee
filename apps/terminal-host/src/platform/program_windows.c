#define WIN32_LEAN_AND_MEAN
#include <windows.h>

/* A background program's start: a process without a console window, created suspended inside a
 * fresh kill-on-close Job that the creation itself names, resumed only once it is in the Job, and
 * inheriting exactly its three standard handles: the null device as input, and for each output the
 * null device or the write end of a pipe whose read end stays with the Host. */

HANDLE ob_windows_terminal_job_create(void);
BOOL ob_windows_terminal_job_assign_and_resume(HANDLE job, HANDLE process, HANDLE thread);

typedef struct ob_windows_program {
    HANDLE process;
    HANDLE job;
    /* The read ends of the captured streams, or NULL for a stream sent to the null device. */
    HANDLE output;
    HANDLE errors;
    DWORD pid;
} ob_windows_program;

static void close_if(HANDLE *handle) {
    if (*handle != NULL && *handle != INVALID_HANDLE_VALUE) {
        CloseHandle(*handle);
    }
    *handle = NULL;
}

/* A pipe whose write end the child inherits and whose read end the Host keeps uninheritable. */
static BOOL captured_pipe(HANDLE *read_end, HANDLE *write_end) {
    SECURITY_ATTRIBUTES inherit = {sizeof(inherit), NULL, TRUE};
    if (!CreatePipe(read_end, write_end, &inherit, 0)) {
        return FALSE;
    }
    return SetHandleInformation(*read_end, HANDLE_FLAG_INHERIT, 0);
}

BOOL ob_windows_program_spawn(
    const wchar_t *application,
    wchar_t *command_line,
    const wchar_t *environment,
    const wchar_t *directory,
    BOOL capture_output,
    BOOL capture_errors,
    ob_windows_program *spawned
) {
    SECURITY_ATTRIBUTES inherit = {sizeof(inherit), NULL, TRUE};
    HANDLE null_device = NULL;
    HANDLE output_write = NULL;
    HANDLE errors_write = NULL;
    HANDLE inherited[3];
    DWORD inherited_count = 0;
    SIZE_T attribute_bytes = 0;
    LPPROC_THREAD_ATTRIBUTE_LIST attributes = NULL;
    STARTUPINFOEXW startup;
    PROCESS_INFORMATION created;
    DWORD error = ERROR_SUCCESS;
    BOOL attributes_ready = FALSE;

    ZeroMemory(spawned, sizeof(*spawned));
    ZeroMemory(&startup, sizeof(startup));
    ZeroMemory(&created, sizeof(created));

    spawned->job = ob_windows_terminal_job_create();
    if (spawned->job == NULL) {
        return FALSE;
    }
    null_device = CreateFileW(
        L"NUL",
        GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        &inherit,
        OPEN_EXISTING,
        0,
        NULL
    );
    if (null_device == INVALID_HANDLE_VALUE) {
        null_device = NULL;
        goto failed;
    }
    inherited[inherited_count++] = null_device;
    if (capture_output) {
        if (!captured_pipe(&spawned->output, &output_write)) {
            goto failed;
        }
        inherited[inherited_count++] = output_write;
    }
    if (capture_errors) {
        if (!captured_pipe(&spawned->errors, &errors_write)) {
            goto failed;
        }
        inherited[inherited_count++] = errors_write;
    }

    (void)InitializeProcThreadAttributeList(NULL, 2, 0, &attribute_bytes);
    attributes = (LPPROC_THREAD_ATTRIBUTE_LIST)HeapAlloc(GetProcessHeap(), 0, attribute_bytes);
    if (attributes == NULL) {
        SetLastError(ERROR_NOT_ENOUGH_MEMORY);
        goto failed;
    }
    if (!InitializeProcThreadAttributeList(attributes, 2, 0, &attribute_bytes)) {
        goto failed;
    }
    attributes_ready = TRUE;
    if (!UpdateProcThreadAttribute(
            attributes,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            inherited,
            inherited_count * sizeof(HANDLE),
            NULL,
            NULL
        ) ||
        !UpdateProcThreadAttribute(
            attributes,
            0,
            PROC_THREAD_ATTRIBUTE_JOB_LIST,
            &spawned->job,
            sizeof(HANDLE),
            NULL,
            NULL
        )) {
        goto failed;
    }

    startup.StartupInfo.cb = sizeof(startup);
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = null_device;
    startup.StartupInfo.hStdOutput = capture_output ? output_write : null_device;
    startup.StartupInfo.hStdError = capture_errors ? errors_write : null_device;
    startup.lpAttributeList = attributes;
    if (!CreateProcessW(
            application,
            command_line,
            NULL,
            NULL,
            TRUE,
            CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT |
                CREATE_NO_WINDOW,
            (LPVOID)environment,
            directory,
            &startup.StartupInfo,
            &created
        )) {
        goto failed;
    }
    /* Only the child holds the write ends now, so each stream ends when its last writer exits. */
    close_if(&output_write);
    close_if(&errors_write);
    close_if(&null_device);
    DeleteProcThreadAttributeList(attributes);
    HeapFree(GetProcessHeap(), 0, attributes);
    if (!ob_windows_terminal_job_assign_and_resume(spawned->job, created.hProcess, created.hThread)) {
        error = GetLastError();
        CloseHandle(created.hThread);
        CloseHandle(created.hProcess);
        close_if(&spawned->output);
        close_if(&spawned->errors);
        close_if(&spawned->job);
        SetLastError(error);
        return FALSE;
    }
    CloseHandle(created.hThread);
    spawned->process = created.hProcess;
    spawned->pid = created.dwProcessId;
    return TRUE;

failed:
    error = GetLastError();
    if (attributes_ready) {
        DeleteProcThreadAttributeList(attributes);
    }
    if (attributes != NULL) {
        HeapFree(GetProcessHeap(), 0, attributes);
    }
    close_if(&output_write);
    close_if(&errors_write);
    close_if(&null_device);
    close_if(&spawned->output);
    close_if(&spawned->errors);
    close_if(&spawned->job);
    SetLastError(error);
    return FALSE;
}
