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

/* One Worker session plus at most four control connections (runtime-model TH-003). Every
 * instance of a pipe must declare the same limit. */
#define OB_WINDOWS_PIPE_INSTANCES 5
#define OB_WINDOWS_PIPE_NAME_CHARACTERS 96

typedef enum {
    OB_WINDOWS_READ_IDLE = 0,
    OB_WINDOWS_READ_PENDING = 1,
    OB_WINDOWS_READ_PREFETCHED = 2,
} ObWindowsReadState;

struct ObWindowsLocalServer {
    /* Held only by the primary instance, which owns the endpoint lifetime. */
    HANDLE mutex;
    /* Owned by the primary: a manual-reset event that ends its instances' accept waits. */
    HANDLE instances_stop;
    /* An instance borrows its primary's `instances_stop`; the primary outlives its instances. */
    HANDLE accept_wake;
    HANDLE pipe;
    BYTE *token_user;
    BYTE prefetched_byte;
    ObWindowsReadState read_state;
    OVERLAPPED readiness_read;
    /* The listen for the next client stays armed across accept timeouts, so a client that
     * connects between two waits is accepted by the next one instead of being disconnected. */
    OVERLAPPED listen;
    BOOL listening;
    BOOL connected;
    /* A pipe server can impersonate its client only after reading from the pipe, so the peer's
     * identity is checked when its first byte arrives; nothing is written to it before. */
    BOOL peer_verified;
    wchar_t pipe_name[OB_WINDOWS_PIPE_NAME_CHARACTERS];
};

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

/* Ends the accepted connection, if any; an armed listen stays armed. */
static void ob_windows_local_server_end_connection(ObWindowsLocalServer *server) {
    if (server->read_state == OB_WINDOWS_READ_PENDING) {
        ob_cancel_overlapped(server->pipe, &server->readiness_read);
    }
    server->read_state = OB_WINDOWS_READ_IDLE;
    server->prefetched_byte = 0;
    server->peer_verified = FALSE;
    if (server->connected) {
        (void)DisconnectNamedPipe(server->pipe);
        server->connected = FALSE;
    }
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

/* Takes ownership of `token_user`, which is freed on failure. */
static ObWindowsLocalServer *ob_windows_local_server_open_instance(
    const wchar_t *pipe_name,
    BYTE *token_user,
    SECURITY_ATTRIBUTES *attributes,
    DWORD first_instance_flag
) {
    HANDLE pipe = CreateNamedPipeW(
        pipe_name,
        PIPE_ACCESS_DUPLEX | first_instance_flag | FILE_FLAG_OVERLAPPED,
        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
        OB_WINDOWS_PIPE_INSTANCES,
        256 * 1024,
        256 * 1024,
        0,
        attributes
    );
    if (pipe == INVALID_HANDLE_VALUE) {
        HeapFree(GetProcessHeap(), 0, token_user);
        return NULL;
    }
    ObWindowsLocalServer *server = HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, sizeof(*server));
    if (server == NULL) {
        CloseHandle(pipe);
        HeapFree(GetProcessHeap(), 0, token_user);
        return NULL;
    }
    server->readiness_read.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    server->listen.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (server->readiness_read.hEvent == NULL || server->listen.hEvent == NULL) {
        if (server->readiness_read.hEvent != NULL) CloseHandle(server->readiness_read.hEvent);
        if (server->listen.hEvent != NULL) CloseHandle(server->listen.hEvent);
        HeapFree(GetProcessHeap(), 0, server);
        CloseHandle(pipe);
        HeapFree(GetProcessHeap(), 0, token_user);
        return NULL;
    }
    server->pipe = pipe;
    server->token_user = token_user;
    wcscpy_s(server->pipe_name, OB_WINDOWS_PIPE_NAME_CHARACTERS, pipe_name);
    return server;
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
    wchar_t pipe_name[OB_WINDOWS_PIPE_NAME_CHARACTERS];
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

    ObWindowsLocalServer *server = ob_windows_local_server_open_instance(
        pipe_name,
        token_user,
        &attributes,
        FILE_FLAG_FIRST_PIPE_INSTANCE
    );
    LocalFree(descriptor);
    if (server == NULL) {
        ReleaseMutex(mutex);
        CloseHandle(mutex);
        return NULL;
    }
    server->mutex = mutex;
    server->instances_stop = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (server->instances_stop == NULL) {
        ob_windows_local_server_destroy(server);
        return NULL;
    }
    *status = OB_WINDOWS_IPC_OK;
    return server;
}

ObWindowsLocalServer *ob_windows_local_server_create_instance(
    const ObWindowsLocalServer *primary,
    int *status
) {
    *status = OB_WINDOWS_IPC_SYSTEM_ERROR;
    HANDLE process_token = NULL;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &process_token)) return NULL;
    BYTE *token_user = ob_token_user(process_token);
    CloseHandle(process_token);
    if (token_user == NULL) return NULL;
    PSECURITY_DESCRIPTOR descriptor =
        ob_current_user_descriptor(((TOKEN_USER *)token_user)->User.Sid);
    if (descriptor == NULL) {
        HeapFree(GetProcessHeap(), 0, token_user);
        return NULL;
    }
    SECURITY_ATTRIBUTES attributes = {
        .nLength = sizeof(SECURITY_ATTRIBUTES),
        .lpSecurityDescriptor = descriptor,
        .bInheritHandle = FALSE,
    };
    ObWindowsLocalServer *server = ob_windows_local_server_open_instance(
        primary->pipe_name,
        token_user,
        &attributes,
        0
    );
    LocalFree(descriptor);
    if (server == NULL) return NULL;
    server->accept_wake = primary->instances_stop;
    *status = OB_WINDOWS_IPC_OK;
    return server;
}

void ob_windows_local_server_destroy(ObWindowsLocalServer *server) {
    if (server == NULL) return;
    if (server->listening) {
        ob_cancel_overlapped(server->pipe, &server->listen);
        server->listening = FALSE;
    }
    ob_windows_local_server_end_connection(server);
    CloseHandle(server->listen.hEvent);
    CloseHandle(server->readiness_read.hEvent);
    CloseHandle(server->pipe);
    if (server->instances_stop != NULL) CloseHandle(server->instances_stop);
    if (server->mutex != NULL) {
        ReleaseMutex(server->mutex);
        CloseHandle(server->mutex);
    }
    HeapFree(GetProcessHeap(), 0, server->token_user);
    HeapFree(GetProcessHeap(), 0, server);
}

void ob_windows_local_server_stop_instances(ObWindowsLocalServer *primary) {
    (void)SetEvent(primary->instances_stop);
}

void ob_windows_local_server_resume_instances(ObWindowsLocalServer *primary) {
    (void)ResetEvent(primary->instances_stop);
}

/* Whether the connected client runs as this process's user. Impersonation needs a byte already
 * read from the pipe. A failed revert ends the process instead of continuing as the client. */
static BOOL ob_windows_local_server_peer_is_same_user(ObWindowsLocalServer *server) {
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
    return same_user;
}

/* Keeps the byte a completed readiness read delivered; a connection's first byte is kept only
 * from a peer that passes the identity check. */
static int ob_windows_local_server_take_readiness_byte(
    ObWindowsLocalServer *server,
    DWORD transferred,
    uint32_t *readable
) {
    server->read_state = OB_WINDOWS_READ_IDLE;
    if (transferred != 1) return OB_WINDOWS_IPC_CONNECTION_CLOSED;
    if (!server->peer_verified) {
        if (!ob_windows_local_server_peer_is_same_user(server)) {
            server->prefetched_byte = 0;
            return OB_WINDOWS_IPC_PEER_IDENTITY_MISMATCH;
        }
        server->peer_verified = TRUE;
    }
    server->read_state = OB_WINDOWS_READ_PREFETCHED;
    *readable = 1;
    return OB_WINDOWS_IPC_OK;
}

/* Arms the one-byte readiness read from `idle`; a byte already queued completes it at once. */
static int ob_windows_local_server_start_readiness_read(
    ObWindowsLocalServer *server,
    uint32_t *readable
) {
    HANDLE readiness_event = server->readiness_read.hEvent;
    ZeroMemory(&server->readiness_read, sizeof(server->readiness_read));
    server->readiness_read.hEvent = readiness_event;
    if (!ResetEvent(readiness_event)) return OB_WINDOWS_IPC_SYSTEM_ERROR;

    if (ReadFile(server->pipe, &server->prefetched_byte, 1, NULL, &server->readiness_read)) {
        DWORD transferred = 0;
        if (!GetOverlappedResult(
                server->pipe,
                &server->readiness_read,
                &transferred,
                FALSE
            )) {
            return ob_connection_status(GetLastError());
        }
        return ob_windows_local_server_take_readiness_byte(server, transferred, readable);
    }
    const DWORD start_error = GetLastError();
    if (start_error != ERROR_IO_PENDING) return ob_connection_status(start_error);
    server->read_state = OB_WINDOWS_READ_PENDING;
    return OB_WINDOWS_IPC_OK;
}

/* Arms the listen for the next client; `client_connected` reports a client that connected before
 * it, which is then already accepted. */
static int ob_windows_local_server_listen(
    ObWindowsLocalServer *server,
    BOOL *client_connected
) {
    *client_connected = FALSE;
    HANDLE listen_event = server->listen.hEvent;
    ZeroMemory(&server->listen, sizeof(server->listen));
    server->listen.hEvent = listen_event;
    if (!ResetEvent(listen_event)) return OB_WINDOWS_IPC_SYSTEM_ERROR;
    if (ConnectNamedPipe(server->pipe, &server->listen)) {
        *client_connected = TRUE;
        return OB_WINDOWS_IPC_OK;
    }
    const DWORD listen_error = GetLastError();
    switch (listen_error) {
        case ERROR_IO_PENDING:
            server->listening = TRUE;
            return OB_WINDOWS_IPC_OK;
        case ERROR_PIPE_CONNECTED:
            *client_connected = TRUE;
            return OB_WINDOWS_IPC_OK;
        case ERROR_NO_DATA:
            // A client connected and left before the listen; reset the instance for the next one.
            (void)DisconnectNamedPipe(server->pipe);
            return OB_WINDOWS_IPC_CONNECTION_CLOSED;
        default:
            return ob_connection_status(listen_error);
    }
}

/* Waits at most `timeout_ms` for a client, or until the accept wake is signalled; on either
 * timeout the listen stays armed. Accepting reads nothing: the readiness wait on the serving
 * thread awaits the first byte, and with it the peer check, under the caller's own deadline. */
static int ob_windows_local_server_accept_with_timeout(
    ObWindowsLocalServer *server,
    DWORD timeout_ms
) {
    ob_windows_local_server_end_connection(server);
    for (;;) {
        BOOL client_connected = FALSE;
        if (!server->listening) {
            const int listen_status = ob_windows_local_server_listen(server, &client_connected);
            if (listen_status != OB_WINDOWS_IPC_OK) return listen_status;
            if (client_connected) break;
        }
        HANDLE events[2] = {server->listen.hEvent, server->accept_wake};
        const DWORD event_count = server->accept_wake != NULL ? 2 : 1;
        // A connection and a wake together return the connection: its index is lower.
        const DWORD wait_result = WaitForMultipleObjects(event_count, events, FALSE, timeout_ms);
        if (wait_result == WAIT_TIMEOUT ||
            (event_count == 2 && wait_result == WAIT_OBJECT_0 + 1)) {
            return OB_WINDOWS_IPC_ACCEPT_TIMEOUT;
        }
        if (wait_result != WAIT_OBJECT_0) return OB_WINDOWS_IPC_SYSTEM_ERROR;
        server->listening = FALSE;
        DWORD ignored = 0;
        if (GetOverlappedResult(server->pipe, &server->listen, &ignored, FALSE)) break;
        const DWORD listen_error = GetLastError();
        // Windows cancels a thread's pending I/O when the thread exits, so a listen armed by an
        // earlier caller's thread is armed again here.
        if (listen_error == ERROR_OPERATION_ABORTED) continue;
        (void)DisconnectNamedPipe(server->pipe);
        return ob_connection_status(listen_error);
    }
    server->connected = TRUE;
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
    if (server->read_state == OB_WINDOWS_READ_PENDING || !server->peer_verified) {
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
    HANDLE wake_event,
    HANDLE wake_timer,
    uint32_t *readable
) {
    HANDLE events[3] = {server->readiness_read.hEvent, NULL, NULL};
    DWORD event_count = 1;
    if (wake_event != NULL) events[event_count++] = wake_event;
    if (wake_timer != NULL) events[event_count++] = wake_timer;
    const DWORD wait_result = WaitForMultipleObjects(
        event_count,
        events,
        FALSE,
        timeout_ms
    );
    if (wait_result == WAIT_TIMEOUT) return OB_WINDOWS_IPC_OK;
    if (wait_result > WAIT_OBJECT_0 && wait_result < WAIT_OBJECT_0 + event_count) {
        return OB_WINDOWS_IPC_OK;
    }
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
    return ob_windows_local_server_take_readiness_byte(server, transferred, readable);
}

int ob_windows_local_server_wait_readable(
    ObWindowsLocalServer *server,
    uint32_t timeout_ms,
    uint32_t *readable
) {
    return ob_windows_local_server_wait_readable_or_wake(
        server,
        timeout_ms,
        NULL,
        NULL,
        readable
    );
}

int ob_windows_local_server_wait_readable_or_wake(
    ObWindowsLocalServer *server,
    uint32_t timeout_ms,
    void *wake_event,
    void *wake_timer,
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
            wake_event,
            wake_timer,
            readable
        );
    }

    // Queued bytes stay available for one bulk read, but only after the peer is verified.
    if (server->peer_verified) {
        DWORD available_bytes = 0;
        if (!PeekNamedPipe(server->pipe, NULL, 0, NULL, &available_bytes, NULL)) {
            return ob_connection_status(GetLastError());
        }
        if (available_bytes > 0) {
            *readable = 1;
            return OB_WINDOWS_IPC_OK;
        }
    }
    const int start_status = ob_windows_local_server_start_readiness_read(server, readable);
    if (start_status != OB_WINDOWS_IPC_OK || server->read_state != OB_WINDOWS_READ_PENDING) {
        return start_status;
    }
    return ob_windows_local_server_finish_readiness_read(
        server,
        timeout_ms,
        wake_event,
        wake_timer,
        readable
    );
}

int ob_windows_local_server_read(
    ObWindowsLocalServer *server,
    uint8_t *buffer,
    uint32_t capacity,
    uint32_t *read_bytes
) {
    if (server->read_state == OB_WINDOWS_READ_PENDING || !server->peer_verified) {
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
    if (!server->peer_verified) return OB_WINDOWS_IPC_CONNECTION_CLOSED;
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
    if (!server->peer_verified) return OB_WINDOWS_IPC_CONNECTION_CLOSED;
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
    ob_windows_local_server_end_connection(server);
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
