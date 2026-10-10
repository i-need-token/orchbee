#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <aclapi.h>
#include <sddl.h>
#include <stdio.h>

#include "local_ipc_windows.h"

_Static_assert(
    sizeof(DWORD) == sizeof(uint32_t),
    "Windows transfer counts must fit the terminal Host C boundary"
);

#ifndef PIPE_REJECT_REMOTE_CLIENTS
#define PIPE_REJECT_REMOTE_CLIENTS 0x00000008
#endif

typedef enum {
    OB_WINDOWS_READ_IDLE = 0,
    OB_WINDOWS_READ_PENDING = 1,
    OB_WINDOWS_READ_PREFETCHED = 2,
} ObWindowsReadState;

struct ObWindowsLocalServer {
    HANDLE mutex;
    HANDLE pipe;
    BYTE *token_user;
    BYTE prefetched_byte;
    ObWindowsReadState read_state;
    OVERLAPPED readiness_read;
};

typedef struct {
    ULONGLONG started_ms;
    DWORD timeout_ms;
} ObWindowsDeadline;

static int ob_connection_status(DWORD error) {
    switch (error) {
        case ERROR_BROKEN_PIPE:
        case ERROR_NO_DATA:
        case ERROR_PIPE_NOT_CONNECTED:
            return OB_WINDOWS_IPC_CONNECTION_CLOSED;
        default:
            return OB_WINDOWS_IPC_SYSTEM_ERROR;
    }
}

static ObWindowsDeadline ob_deadline_start(DWORD timeout_ms) {
    ObWindowsDeadline deadline = {
        .started_ms = GetTickCount64(),
        .timeout_ms = timeout_ms,
    };
    return deadline;
}

static BOOL ob_deadline_remaining(
    const ObWindowsDeadline *deadline,
    DWORD *remaining_ms
) {
    if (deadline->timeout_ms == INFINITE) {
        *remaining_ms = INFINITE;
        return TRUE;
    }
    const ULONGLONG elapsed_ms = GetTickCount64() - deadline->started_ms;
    if (elapsed_ms >= deadline->timeout_ms) {
        *remaining_ms = 0;
        return FALSE;
    }
    *remaining_ms = deadline->timeout_ms - (DWORD)elapsed_ms;
    return TRUE;
}

static void ob_cancel_overlapped(HANDLE pipe, OVERLAPPED *overlapped) {
    // ERROR_NOT_FOUND only means that completion won the cancellation race.
    (void)CancelIoEx(pipe, overlapped);
    DWORD ignored = 0;
    (void)GetOverlappedResult(pipe, overlapped, &ignored, TRUE);
}

static int ob_wait_for_overlapped(
    HANDLE pipe,
    OVERLAPPED *overlapped,
    DWORD timeout_ms,
    DWORD *transferred
) {
    const DWORD wait_result = WaitForSingleObject(overlapped->hEvent, timeout_ms);
    if (wait_result == WAIT_TIMEOUT) {
        // The caller's buffer and OVERLAPPED storage cannot be released until
        // the kernel has completed cancellation.
        ob_cancel_overlapped(pipe, overlapped);
        return OB_WINDOWS_IPC_WRITE_TIMEOUT;
    }
    if (wait_result != WAIT_OBJECT_0) {
        ob_cancel_overlapped(pipe, overlapped);
        return OB_WINDOWS_IPC_SYSTEM_ERROR;
    }
    if (!GetOverlappedResult(pipe, overlapped, transferred, FALSE)) {
        return ob_connection_status(GetLastError());
    }
    return OB_WINDOWS_IPC_OK;
}

static int ob_wait_for_overlapped_before(
    HANDLE pipe,
    OVERLAPPED *overlapped,
    const ObWindowsDeadline *deadline,
    DWORD *transferred
) {
    DWORD remaining_ms = 0;
    if (!ob_deadline_remaining(deadline, &remaining_ms)) {
        ob_cancel_overlapped(pipe, overlapped);
        return OB_WINDOWS_IPC_ACCEPT_TIMEOUT;
    }
    const int status = ob_wait_for_overlapped(
        pipe,
        overlapped,
        remaining_ms,
        transferred
    );
    if (status == OB_WINDOWS_IPC_WRITE_TIMEOUT) {
        return OB_WINDOWS_IPC_ACCEPT_TIMEOUT;
    }
    if (status != OB_WINDOWS_IPC_OK) return status;
    if (!ob_deadline_remaining(deadline, &remaining_ms)) {
        return OB_WINDOWS_IPC_ACCEPT_TIMEOUT;
    }
    return OB_WINDOWS_IPC_OK;
}

static void ob_windows_local_server_reset_connection(
    ObWindowsLocalServer *server
) {
    if (server->read_state == OB_WINDOWS_READ_PENDING) {
        ob_cancel_overlapped(server->pipe, &server->readiness_read);
    }
    server->read_state = OB_WINDOWS_READ_IDLE;
    server->prefetched_byte = 0;
    (void)DisconnectNamedPipe(server->pipe);
}

static int ob_overlapped_transfer_result(
    HANDLE pipe,
    OVERLAPPED *overlapped,
    BOOL started,
    DWORD timeout_ms,
    DWORD *transferred
) {
    if (started) {
        if (!GetOverlappedResult(pipe, overlapped, transferred, FALSE)) {
            return ob_connection_status(GetLastError());
        }
        return OB_WINDOWS_IPC_OK;
    }
    const DWORD start_error = GetLastError();
    if (start_error != ERROR_IO_PENDING) return ob_connection_status(start_error);
    return ob_wait_for_overlapped(pipe, overlapped, timeout_ms, transferred);
}

static BYTE *ob_token_user(HANDLE token) {
    DWORD length = 0;
    GetTokenInformation(token, TokenUser, NULL, 0, &length);
    if (length == 0 || GetLastError() != ERROR_INSUFFICIENT_BUFFER) return NULL;
    BYTE *buffer = HeapAlloc(GetProcessHeap(), 0, length);
    if (buffer == NULL) return NULL;
    if (!GetTokenInformation(token, TokenUser, buffer, length, &length)) {
        HeapFree(GetProcessHeap(), 0, buffer);
        return NULL;
    }
    return buffer;
}

static BOOL ob_trusted_acl_sid(PSID candidate, PSID user, PSID system, PSID administrators) {
    return EqualSid(candidate, user) || EqualSid(candidate, system) ||
           EqualSid(candidate, administrators);
}

static BOOL ob_private_runtime_directory(const wchar_t *path, PSID user) {
    HANDLE directory = CreateFileW(
        path,
        READ_CONTROL,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        NULL,
        OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
        NULL
    );
    if (directory == INVALID_HANDLE_VALUE) return FALSE;

    FILE_ATTRIBUTE_TAG_INFO tag = {0};
    if (!GetFileInformationByHandleEx(
            directory,
            FileAttributeTagInfo,
            &tag,
            sizeof(tag)
        ) ||
        !(tag.FileAttributes & FILE_ATTRIBUTE_DIRECTORY) ||
        (tag.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT)) {
        CloseHandle(directory);
        return FALSE;
    }

    PSID owner = NULL;
    PACL dacl = NULL;
    PSECURITY_DESCRIPTOR descriptor = NULL;
    DWORD security_result = GetSecurityInfo(
        directory,
        SE_FILE_OBJECT,
        OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
        &owner,
        NULL,
        &dacl,
        NULL,
        &descriptor
    );
    CloseHandle(directory);
    if (security_result != ERROR_SUCCESS || descriptor == NULL || owner == NULL ||
        dacl == NULL || !EqualSid(owner, user)) {
        if (descriptor != NULL) LocalFree(descriptor);
        return FALSE;
    }

    BYTE system_buffer[SECURITY_MAX_SID_SIZE];
    BYTE administrators_buffer[SECURITY_MAX_SID_SIZE];
    DWORD system_length = sizeof(system_buffer);
    DWORD administrators_length = sizeof(administrators_buffer);
    if (!CreateWellKnownSid(WinLocalSystemSid, NULL, system_buffer, &system_length) ||
        !CreateWellKnownSid(
            WinBuiltinAdministratorsSid,
            NULL,
            administrators_buffer,
            &administrators_length
        )) {
        LocalFree(descriptor);
        return FALSE;
    }

    BOOL private_directory = TRUE;
    for (DWORD index = 0; index < dacl->AceCount; ++index) {
        ACE_HEADER *header = NULL;
        if (!GetAce(dacl, index, (void **)&header)) {
            private_directory = FALSE;
            break;
        }
        if (header->AceFlags & INHERIT_ONLY_ACE) continue;
        if (header->AceType == ACCESS_ALLOWED_ACE_TYPE) {
            ACCESS_ALLOWED_ACE *ace = (ACCESS_ALLOWED_ACE *)header;
            PSID sid = (PSID)&ace->SidStart;
            if (!ob_trusted_acl_sid(sid, user, system_buffer, administrators_buffer)) {
                private_directory = FALSE;
                break;
            }
            continue;
        }
        if (header->AceType == ACCESS_DENIED_ACE_TYPE) continue;
        if (header->AceType == ACCESS_ALLOWED_OBJECT_ACE_TYPE ||
            header->AceType == ACCESS_ALLOWED_CALLBACK_ACE_TYPE ||
            header->AceType == ACCESS_ALLOWED_CALLBACK_OBJECT_ACE_TYPE) {
            private_directory = FALSE;
            break;
        }
    }
    LocalFree(descriptor);
    return private_directory;
}

static PSECURITY_DESCRIPTOR ob_current_user_descriptor(PSID user) {
    LPWSTR sid_text = NULL;
    if (!ConvertSidToStringSidW(user, &sid_text)) return NULL;
    size_t characters = wcslen(sid_text) + 32;
    wchar_t *sddl = HeapAlloc(GetProcessHeap(), 0, characters * sizeof(wchar_t));
    if (sddl == NULL) {
        LocalFree(sid_text);
        return NULL;
    }
    _snwprintf_s(sddl, characters, _TRUNCATE, L"D:P(A;;GA;;;%ls)", sid_text);
    LocalFree(sid_text);

    PSECURITY_DESCRIPTOR descriptor = NULL;
    if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl,
            SDDL_REVISION_1,
            &descriptor,
            NULL
        )) {
        descriptor = NULL;
    }
    HeapFree(GetProcessHeap(), 0, sddl);
    return descriptor;
}

ObWindowsLocalServer *ob_windows_local_server_create(
    const wchar_t *runtime_directory,
    const wchar_t *endpoint_id,
    int *status
) {
    *status = OB_WINDOWS_IPC_SYSTEM_ERROR;
    HANDLE process_token = NULL;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &process_token)) return NULL;
    BYTE *token_user = ob_token_user(process_token);
    CloseHandle(process_token);
    if (token_user == NULL) return NULL;
    PSID user = ((TOKEN_USER *)token_user)->User.Sid;

    if (!ob_private_runtime_directory(runtime_directory, user)) {
        HeapFree(GetProcessHeap(), 0, token_user);
        *status = OB_WINDOWS_IPC_INVALID_RUNTIME_DIRECTORY;
        return NULL;
    }

    PSECURITY_DESCRIPTOR descriptor = ob_current_user_descriptor(user);
    if (descriptor == NULL) {
        HeapFree(GetProcessHeap(), 0, token_user);
        return NULL;
    }
    SECURITY_ATTRIBUTES attributes = {
        .nLength = sizeof(SECURITY_ATTRIBUTES),
        .lpSecurityDescriptor = descriptor,
        .bInheritHandle = FALSE,
    };

    wchar_t mutex_name[96];
    wchar_t pipe_name[96];
    _snwprintf_s(
        mutex_name,
        sizeof(mutex_name) / sizeof(mutex_name[0]),
        _TRUNCATE,
        L"Global\\orchbee-terminal-host-%ls",
        endpoint_id
    );
    _snwprintf_s(
        pipe_name,
        sizeof(pipe_name) / sizeof(pipe_name[0]),
        _TRUNCATE,
        L"\\\\.\\pipe\\orchbee-terminal-host-%ls",
        endpoint_id
    );

    HANDLE mutex = CreateMutexW(&attributes, TRUE, mutex_name);
    DWORD mutex_error = GetLastError();
    if (mutex == NULL || mutex_error == ERROR_ALREADY_EXISTS) {
        if (mutex != NULL) CloseHandle(mutex);
        LocalFree(descriptor);
        HeapFree(GetProcessHeap(), 0, token_user);
        *status = mutex_error == ERROR_ALREADY_EXISTS
                      ? OB_WINDOWS_IPC_ALREADY_RUNNING
                      : OB_WINDOWS_IPC_SYSTEM_ERROR;
        return NULL;
    }

    HANDLE pipe = CreateNamedPipeW(
        pipe_name,
        PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED,
        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
        1,
        256 * 1024,
        256 * 1024,
        0,
        &attributes
    );
    LocalFree(descriptor);
    if (pipe == INVALID_HANDLE_VALUE) {
        ReleaseMutex(mutex);
        CloseHandle(mutex);
        HeapFree(GetProcessHeap(), 0, token_user);
        return NULL;
    }

    ObWindowsLocalServer *server = HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, sizeof(*server));
    if (server == NULL) {
        CloseHandle(pipe);
        ReleaseMutex(mutex);
        CloseHandle(mutex);
        HeapFree(GetProcessHeap(), 0, token_user);
        return NULL;
    }
    server->mutex = mutex;
    server->pipe = pipe;
    server->token_user = token_user;
    server->readiness_read.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (server->readiness_read.hEvent == NULL) {
        HeapFree(GetProcessHeap(), 0, server);
        CloseHandle(pipe);
        ReleaseMutex(mutex);
        CloseHandle(mutex);
        HeapFree(GetProcessHeap(), 0, token_user);
        return NULL;
    }
    *status = OB_WINDOWS_IPC_OK;
    return server;
}

void ob_windows_local_server_destroy(ObWindowsLocalServer *server) {
    if (server == NULL) return;
    ob_windows_local_server_reset_connection(server);
    CloseHandle(server->readiness_read.hEvent);
    CloseHandle(server->pipe);
    ReleaseMutex(server->mutex);
    CloseHandle(server->mutex);
    HeapFree(GetProcessHeap(), 0, server->token_user);
    HeapFree(GetProcessHeap(), 0, server);
}

static int ob_windows_local_server_accept_with_timeout(
    ObWindowsLocalServer *server,
    DWORD timeout_ms
) {
    if (server->read_state != OB_WINDOWS_READ_IDLE) {
        ob_windows_local_server_reset_connection(server);
    }
    const ObWindowsDeadline deadline = ob_deadline_start(timeout_ms);
    DWORD remaining_ms = 0;
    if (!ob_deadline_remaining(&deadline, &remaining_ms)) {
        ob_windows_local_server_reset_connection(server);
        return OB_WINDOWS_IPC_ACCEPT_TIMEOUT;
    }

    OVERLAPPED overlapped = {0};
    overlapped.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (overlapped.hEvent == NULL) {
        ob_windows_local_server_reset_connection(server);
        return OB_WINDOWS_IPC_SYSTEM_ERROR;
    }
    const BOOL connected = ConnectNamedPipe(server->pipe, &overlapped);
    if (!connected) {
        const DWORD connect_error = GetLastError();
        if (connect_error == ERROR_NO_DATA) {
            CloseHandle(overlapped.hEvent);
            ob_windows_local_server_reset_connection(server);
            return OB_WINDOWS_IPC_CONNECTION_CLOSED;
        }
        if (connect_error == ERROR_IO_PENDING) {
            DWORD ignored = 0;
            const int status = ob_wait_for_overlapped_before(
                server->pipe,
                &overlapped,
                &deadline,
                &ignored
            );
            CloseHandle(overlapped.hEvent);
            if (status != OB_WINDOWS_IPC_OK) {
                ob_windows_local_server_reset_connection(server);
                return status;
            }
        } else {
            CloseHandle(overlapped.hEvent);
            if (connect_error != ERROR_PIPE_CONNECTED) {
                ob_windows_local_server_reset_connection(server);
                return ob_connection_status(connect_error);
            }
        }
    } else {
        CloseHandle(overlapped.hEvent);
    }

    if (!ob_deadline_remaining(&deadline, &remaining_ms)) {
        ob_windows_local_server_reset_connection(server);
        return OB_WINDOWS_IPC_ACCEPT_TIMEOUT;
    }

    BYTE first_byte = 0;
    OVERLAPPED read_overlapped = {0};
    read_overlapped.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (read_overlapped.hEvent == NULL) {
        ob_windows_local_server_reset_connection(server);
        return OB_WINDOWS_IPC_SYSTEM_ERROR;
    }
    const BOOL read_started = ReadFile(
        server->pipe,
        &first_byte,
        1,
        NULL,
        &read_overlapped
    );
    const DWORD read_error = read_started ? ERROR_SUCCESS : GetLastError();
    DWORD read_bytes = 0;
    int read_status = OB_WINDOWS_IPC_OK;
    if (read_started) {
        if (!GetOverlappedResult(
                server->pipe,
                &read_overlapped,
                &read_bytes,
                FALSE
            )) {
            read_status = ob_connection_status(GetLastError());
        } else if (!ob_deadline_remaining(&deadline, &remaining_ms)) {
            read_status = OB_WINDOWS_IPC_ACCEPT_TIMEOUT;
        }
    } else if (read_error == ERROR_IO_PENDING) {
        read_status = ob_wait_for_overlapped_before(
            server->pipe,
            &read_overlapped,
            &deadline,
            &read_bytes
        );
    } else {
        read_status = ob_connection_status(read_error);
    }
    CloseHandle(read_overlapped.hEvent);
    if (read_status != OB_WINDOWS_IPC_OK || read_bytes != 1) {
        ob_windows_local_server_reset_connection(server);
        return read_status == OB_WINDOWS_IPC_OK
                   ? OB_WINDOWS_IPC_CONNECTION_CLOSED
                   : read_status;
    }

    BOOL impersonating = ImpersonateNamedPipeClient(server->pipe);
    HANDLE client_token = NULL;
    BYTE *client_user = NULL;
    BOOL same_user = FALSE;
    if (impersonating &&
        OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, TRUE, &client_token)) {
        client_user = ob_token_user(client_token);
        if (client_user != NULL) {
            same_user = EqualSid(
                ((TOKEN_USER *)server->token_user)->User.Sid,
                ((TOKEN_USER *)client_user)->User.Sid
            );
        }
    }
    if (client_user != NULL) HeapFree(GetProcessHeap(), 0, client_user);
    if (client_token != NULL) CloseHandle(client_token);
    if (impersonating && !RevertToSelf()) {
        DWORD revert_error = GetLastError();
        DWORD exit_code = revert_error == ERROR_SUCCESS
                              ? ERROR_CANNOT_IMPERSONATE
                              : revert_error;
        TerminateProcess(GetCurrentProcess(), exit_code);
        ExitProcess(exit_code);
    }
    if (!same_user) {
        ob_windows_local_server_reset_connection(server);
        return OB_WINDOWS_IPC_PEER_IDENTITY_MISMATCH;
    }
    if (!ob_deadline_remaining(&deadline, &remaining_ms)) {
        ob_windows_local_server_reset_connection(server);
        return OB_WINDOWS_IPC_ACCEPT_TIMEOUT;
    }
    server->prefetched_byte = first_byte;
    server->read_state = OB_WINDOWS_READ_PREFETCHED;
    return OB_WINDOWS_IPC_OK;
}

int ob_windows_local_server_accept(ObWindowsLocalServer *server) {
    return ob_windows_local_server_accept_with_timeout(server, INFINITE);
}

int ob_windows_local_server_accept_before(
    ObWindowsLocalServer *server,
    uint32_t timeout_ms
) {
    return ob_windows_local_server_accept_with_timeout(server, timeout_ms);
}

int ob_windows_local_server_peek(ObWindowsLocalServer *server, uint32_t *available) {
    if (server->read_state == OB_WINDOWS_READ_PENDING) {
        uint32_t readable = 0;
        const int status = ob_windows_local_server_wait_readable(
            server,
            0,
            &readable
        );
        if (status != OB_WINDOWS_IPC_OK) return status;
        if (!readable) {
            *available = 0;
            return OB_WINDOWS_IPC_OK;
        }
    }
    if (server->read_state == OB_WINDOWS_READ_PREFETCHED) {
        *available = 1;
        return OB_WINDOWS_IPC_OK;
    }
    DWORD available_bytes = 0;
    if (!PeekNamedPipe(server->pipe, NULL, 0, NULL, &available_bytes, NULL)) {
        return ob_connection_status(GetLastError());
    }
    *available = (uint32_t)available_bytes;
    return OB_WINDOWS_IPC_OK;
}

static int ob_windows_local_server_finish_readiness_read(
    ObWindowsLocalServer *server,
    DWORD timeout_ms,
    uint32_t *readable
) {
    const DWORD wait_result = WaitForSingleObject(
        server->readiness_read.hEvent,
        timeout_ms
    );
    if (wait_result == WAIT_TIMEOUT) return OB_WINDOWS_IPC_OK;
    if (wait_result != WAIT_OBJECT_0) return OB_WINDOWS_IPC_SYSTEM_ERROR;

    DWORD transferred = 0;
    if (!GetOverlappedResult(
            server->pipe,
            &server->readiness_read,
            &transferred,
            FALSE
        )) {
        const DWORD result_error = GetLastError();
        if (result_error != ERROR_IO_INCOMPLETE) {
            server->read_state = OB_WINDOWS_READ_IDLE;
        }
        return ob_connection_status(result_error);
    }
    server->read_state = OB_WINDOWS_READ_IDLE;
    if (transferred != 1) return OB_WINDOWS_IPC_CONNECTION_CLOSED;
    server->read_state = OB_WINDOWS_READ_PREFETCHED;
    *readable = 1;
    return OB_WINDOWS_IPC_OK;
}

int ob_windows_local_server_wait_readable(
    ObWindowsLocalServer *server,
    uint32_t timeout_ms,
    uint32_t *readable
) {
    *readable = 0;
    if (server->read_state == OB_WINDOWS_READ_PREFETCHED) {
        *readable = 1;
        return OB_WINDOWS_IPC_OK;
    }
    if (server->read_state == OB_WINDOWS_READ_PENDING) {
        return ob_windows_local_server_finish_readiness_read(
            server,
            timeout_ms,
            readable
        );
    }

    DWORD available_bytes = 0;
    if (!PeekNamedPipe(server->pipe, NULL, 0, NULL, &available_bytes, NULL)) {
        return ob_connection_status(GetLastError());
    }
    if (available_bytes > 0) {
        *readable = 1;
        return OB_WINDOWS_IPC_OK;
    }

    HANDLE readiness_event = server->readiness_read.hEvent;
    ZeroMemory(&server->readiness_read, sizeof(server->readiness_read));
    server->readiness_read.hEvent = readiness_event;
    if (!ResetEvent(readiness_event)) return OB_WINDOWS_IPC_SYSTEM_ERROR;

    const BOOL started = ReadFile(
        server->pipe,
        &server->prefetched_byte,
        1,
        NULL,
        &server->readiness_read
    );
    if (started) {
        DWORD transferred = 0;
        if (!GetOverlappedResult(
                server->pipe,
                &server->readiness_read,
                &transferred,
                FALSE
            )) {
            return ob_connection_status(GetLastError());
        }
        if (transferred != 1) return OB_WINDOWS_IPC_CONNECTION_CLOSED;
        server->read_state = OB_WINDOWS_READ_PREFETCHED;
        *readable = 1;
        return OB_WINDOWS_IPC_OK;
    }

    const DWORD start_error = GetLastError();
    if (start_error != ERROR_IO_PENDING) {
        return ob_connection_status(start_error);
    }
    server->read_state = OB_WINDOWS_READ_PENDING;
    return ob_windows_local_server_finish_readiness_read(
        server,
        timeout_ms,
        readable
    );
}

int ob_windows_local_server_read(
    ObWindowsLocalServer *server,
    uint8_t *buffer,
    uint32_t capacity,
    uint32_t *read_bytes
) {
    if (server->read_state == OB_WINDOWS_READ_PENDING) {
        uint32_t readable = 0;
        const int status = ob_windows_local_server_wait_readable(
            server,
            INFINITE,
            &readable
        );
        if (status != OB_WINDOWS_IPC_OK) return status;
    }
    if (server->read_state == OB_WINDOWS_READ_PREFETCHED && capacity > 0) {
        buffer[0] = server->prefetched_byte;
        server->read_state = OB_WINDOWS_READ_IDLE;
        *read_bytes = 1;
        return OB_WINDOWS_IPC_OK;
    }
    OVERLAPPED overlapped = {0};
    overlapped.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (overlapped.hEvent == NULL) {
        return OB_WINDOWS_IPC_SYSTEM_ERROR;
    }
    const BOOL started = ReadFile(server->pipe, buffer, capacity, NULL, &overlapped);
    DWORD transferred = 0;
    const int status = ob_overlapped_transfer_result(
        server->pipe,
        &overlapped,
        started,
        INFINITE,
        &transferred
    );
    CloseHandle(overlapped.hEvent);
    if (status != OB_WINDOWS_IPC_OK) {
        return status;
    }
    *read_bytes = (uint32_t)transferred;
    if (transferred == 0) {
        return OB_WINDOWS_IPC_CONNECTION_CLOSED;
    }
    return OB_WINDOWS_IPC_OK;
}

int ob_windows_local_server_write(
    ObWindowsLocalServer *server,
    const uint8_t *buffer,
    uint32_t length,
    uint32_t *written_bytes
) {
    OVERLAPPED overlapped = {0};
    overlapped.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (overlapped.hEvent == NULL) {
        return OB_WINDOWS_IPC_SYSTEM_ERROR;
    }
    const BOOL started = WriteFile(server->pipe, buffer, length, NULL, &overlapped);
    DWORD transferred = 0;
    const int status = ob_overlapped_transfer_result(
        server->pipe,
        &overlapped,
        started,
        INFINITE,
        &transferred
    );
    CloseHandle(overlapped.hEvent);
    if (status != OB_WINDOWS_IPC_OK) {
        return status;
    }
    *written_bytes = (uint32_t)transferred;
    if (transferred == 0) {
        return OB_WINDOWS_IPC_CONNECTION_CLOSED;
    }
    return OB_WINDOWS_IPC_OK;
}

int ob_windows_local_server_write_before(
    ObWindowsLocalServer *server,
    const uint8_t *buffer,
    uint32_t length,
    uint32_t timeout_ms,
    uint32_t *written_bytes
) {
    OVERLAPPED overlapped = {0};
    overlapped.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (overlapped.hEvent == NULL) {
        return OB_WINDOWS_IPC_SYSTEM_ERROR;
    }
    const BOOL started = WriteFile(server->pipe, buffer, length, NULL, &overlapped);
    DWORD transferred = 0;
    const int status = ob_overlapped_transfer_result(
        server->pipe,
        &overlapped,
        started,
        timeout_ms,
        &transferred
    );
    CloseHandle(overlapped.hEvent);
    if (status != OB_WINDOWS_IPC_OK) {
        return status;
    }
    *written_bytes = (uint32_t)transferred;
    if (transferred == 0) {
        return OB_WINDOWS_IPC_CONNECTION_CLOSED;
    }
    return OB_WINDOWS_IPC_OK;
}

void ob_windows_local_server_disconnect(ObWindowsLocalServer *server) {
    ob_windows_local_server_reset_connection(server);
}

void *ob_windows_local_test_client_connect(const wchar_t *endpoint_id, int *status) {
    wchar_t pipe_name[96];
    _snwprintf_s(
        pipe_name,
        sizeof(pipe_name) / sizeof(pipe_name[0]),
        _TRUNCATE,
        L"\\\\.\\pipe\\orchbee-terminal-host-%ls",
        endpoint_id
    );
    if (!WaitNamedPipeW(pipe_name, 5000)) {
        *status = ob_connection_status(GetLastError());
        return NULL;
    }
    HANDLE client = CreateFileW(
        pipe_name,
        GENERIC_READ | GENERIC_WRITE,
        0,
        NULL,
        OPEN_EXISTING,
        0,
        NULL
    );
    if (client == INVALID_HANDLE_VALUE) {
        *status = ob_connection_status(GetLastError());
        return NULL;
    }
    *status = OB_WINDOWS_IPC_OK;
    return client;
}

int ob_windows_local_test_client_read(
    void *client,
    uint8_t *buffer,
    uint32_t capacity,
    uint32_t *read_bytes
) {
    DWORD transferred = 0;
    if (!ReadFile((HANDLE)client, buffer, capacity, &transferred, NULL)) {
        return ob_connection_status(GetLastError());
    }
    *read_bytes = (uint32_t)transferred;
    return transferred == 0 ? OB_WINDOWS_IPC_CONNECTION_CLOSED : OB_WINDOWS_IPC_OK;
}

int ob_windows_local_test_client_peek(void *client, uint32_t *available) {
    DWORD available_bytes = 0;
    if (!PeekNamedPipe((HANDLE)client, NULL, 0, NULL, &available_bytes, NULL)) {
        return ob_connection_status(GetLastError());
    }
    *available = (uint32_t)available_bytes;
    return OB_WINDOWS_IPC_OK;
}

int ob_windows_local_test_client_write(
    void *client,
    const uint8_t *buffer,
    uint32_t length,
    uint32_t *written_bytes
) {
    DWORD transferred = 0;
    if (!WriteFile((HANDLE)client, buffer, length, &transferred, NULL)) {
        return ob_connection_status(GetLastError());
    }
    *written_bytes = (uint32_t)transferred;
    return transferred == 0 ? OB_WINDOWS_IPC_CONNECTION_CLOSED : OB_WINDOWS_IPC_OK;
}

void ob_windows_local_test_client_close(void *client) {
    if (client != NULL) CloseHandle((HANDLE)client);
}

int ob_windows_local_test_make_directory_private(const wchar_t *path) {
    HANDLE process_token = NULL;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &process_token)) {
        return OB_WINDOWS_IPC_SYSTEM_ERROR;
    }
    BYTE *token_user = ob_token_user(process_token);
    CloseHandle(process_token);
    if (token_user == NULL) return OB_WINDOWS_IPC_SYSTEM_ERROR;

    PSECURITY_DESCRIPTOR descriptor = ob_current_user_descriptor(
        ((TOKEN_USER *)token_user)->User.Sid
    );
    HeapFree(GetProcessHeap(), 0, token_user);
    if (descriptor == NULL) return OB_WINDOWS_IPC_SYSTEM_ERROR;

    BOOL dacl_present = FALSE;
    BOOL dacl_defaulted = FALSE;
    PACL dacl = NULL;
    BOOL descriptor_valid = GetSecurityDescriptorDacl(
        descriptor,
        &dacl_present,
        &dacl,
        &dacl_defaulted
    );
    DWORD result = descriptor_valid && dacl_present && dacl != NULL
                       ? SetNamedSecurityInfoW(
                             (LPWSTR)path,
                             SE_FILE_OBJECT,
                             DACL_SECURITY_INFORMATION |
                                 PROTECTED_DACL_SECURITY_INFORMATION,
                             NULL,
                             NULL,
                             dacl,
                             NULL
                         )
                       : ERROR_INVALID_SECURITY_DESCR;
    LocalFree(descriptor);
    return result == ERROR_SUCCESS ? OB_WINDOWS_IPC_OK : OB_WINDOWS_IPC_SYSTEM_ERROR;
}
