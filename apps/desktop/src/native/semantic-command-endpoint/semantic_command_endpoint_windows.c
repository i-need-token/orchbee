#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <aclapi.h>
#include <sddl.h>

#include "semantic_command_endpoint.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

#ifndef PIPE_REJECT_REMOTE_CLIENTS
#define PIPE_REJECT_REMOTE_CLIENTS 0x00000008
#endif

#define OB_WIDE_PATH_CAPACITY 32768
#define OB_ENDPOINT_ID_LENGTH 32
#define OB_PIPE_NAME_CAPACITY 128

typedef enum { ACCEPT_IDLE, ACCEPT_CONNECTING, ACCEPT_IDENTITY, ACCEPT_AUTHENTICATED, ACCEPT_CLOSED } AcceptPhase;

struct ObSemanticCommandServer {
    HANDLE pipe;
    BYTE *token_user;
    AcceptPhase phase;
    OVERLAPPED accept_operation;
    BOOL accept_pending;
    ULONGLONG connection_started;
    uint32_t connection_budget;
    uint8_t authenticated_byte;
    BOOL has_authenticated_byte;
};

static int endpoint_id_is_valid(const char *endpoint_id) {
    if (endpoint_id == NULL || strlen(endpoint_id) != OB_ENDPOINT_ID_LENGTH) return 0;
    for (size_t index = 0; index < OB_ENDPOINT_ID_LENGTH; index += 1) {
        const char byte = endpoint_id[index];
        if (!((byte >= '0' && byte <= '9') || (byte >= 'a' && byte <= 'f'))) return 0;
    }
    return 1;
}

static int utf8_to_wide(const char *input, wchar_t *output, size_t capacity) {
    if (input == NULL || input[0] == '\0' || output == NULL || capacity == 0 ||
        capacity > INT_MAX) {
        return 0;
    }
    const int count = MultiByteToWideChar(
        CP_UTF8,
        MB_ERR_INVALID_CHARS,
        input,
        -1,
        output,
        (int)capacity);
    return count > 0 && (size_t)count <= capacity;
}

static int build_pipe_name(
    const char *endpoint_id,
    wchar_t *pipe_name,
    size_t capacity) {
    if (!endpoint_id_is_valid(endpoint_id) || pipe_name == NULL || capacity == 0) return 0;
    wchar_t wide_endpoint[OB_ENDPOINT_ID_LENGTH + 1];
    if (!utf8_to_wide(endpoint_id, wide_endpoint, sizeof(wide_endpoint) / sizeof(wide_endpoint[0]))) {
        return 0;
    }
    const int length = _snwprintf_s(
        pipe_name,
        capacity,
        _TRUNCATE,
        L"\\\\.\\pipe\\orchbee-semantic-command-%ls",
        wide_endpoint);
    return length > 0 && (size_t)length < capacity;
}

static int build_locator_paths(
    const char *runtime_directory,
    const char *endpoint_id,
    wchar_t *locator_path,
    size_t locator_capacity,
    wchar_t *temporary_path,
    size_t temporary_capacity) {
    if (!endpoint_id_is_valid(endpoint_id)) return 0;
    wchar_t runtime_path[OB_WIDE_PATH_CAPACITY];
    wchar_t wide_endpoint[OB_ENDPOINT_ID_LENGTH + 1];
    if (!utf8_to_wide(
            runtime_directory,
            runtime_path,
            sizeof(runtime_path) / sizeof(runtime_path[0])) ||
        !utf8_to_wide(
            endpoint_id,
            wide_endpoint,
            sizeof(wide_endpoint) / sizeof(wide_endpoint[0]))) {
        return 0;
    }
    const wchar_t separator = runtime_path[wcslen(runtime_path) - 1] == L'\\' ? L'\0' : L'\\';
    const int locator_length = separator == L'\0'
        ? _snwprintf_s(
              locator_path,
              locator_capacity,
              _TRUNCATE,
              L"%ls%ls.c.json",
              runtime_path,
              wide_endpoint)
        : _snwprintf_s(
              locator_path,
              locator_capacity,
              _TRUNCATE,
              L"%ls\\%ls.c.json",
              runtime_path,
              wide_endpoint);
    const int temporary_length = separator == L'\0'
        ? _snwprintf_s(
              temporary_path,
              temporary_capacity,
              _TRUNCATE,
              L"%ls%ls.c.json.tmp",
              runtime_path,
              wide_endpoint)
        : _snwprintf_s(
              temporary_path,
              temporary_capacity,
              _TRUNCATE,
              L"%ls\\%ls.c.json.tmp",
              runtime_path,
              wide_endpoint);
    return locator_length > 0 && temporary_length > 0 &&
        (size_t)locator_length < locator_capacity &&
        (size_t)temporary_length < temporary_capacity;
}

static BYTE *token_user(HANDLE token) {
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

static BYTE *current_token_user(void) {
    HANDLE token = NULL;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token)) return NULL;
    BYTE *user = token_user(token);
    CloseHandle(token);
    return user;
}

static PSECURITY_DESCRIPTOR current_user_descriptor(PSID user) {
    EXPLICIT_ACCESSW access[3];
    ZeroMemory(access, sizeof(access));
    BYTE system_buffer[SECURITY_MAX_SID_SIZE];
    BYTE administrators_buffer[SECURITY_MAX_SID_SIZE];
    DWORD system_length = sizeof(system_buffer);
    DWORD administrators_length = sizeof(administrators_buffer);
    if (!CreateWellKnownSid(WinLocalSystemSid, NULL, system_buffer, &system_length) ||
        !CreateWellKnownSid(
            WinBuiltinAdministratorsSid,
            NULL,
            administrators_buffer,
            &administrators_length)) {
        return NULL;
    }
    PSID identities[] = {user, system_buffer, administrators_buffer};
    for (size_t index = 0; index < 3; index += 1) {
        access[index].grfAccessPermissions = GENERIC_ALL;
        access[index].grfAccessMode = SET_ACCESS;
        access[index].grfInheritance = index == 0
            ? SUB_CONTAINERS_AND_OBJECTS_INHERIT
            : NO_INHERITANCE;
        access[index].Trustee.TrusteeForm = TRUSTEE_IS_SID;
        access[index].Trustee.TrusteeType = TRUSTEE_IS_USER;
        access[index].Trustee.ptstrName = identities[index];
    }
    PACL acl = NULL;
    if (SetEntriesInAclW(3, access, NULL, &acl) != ERROR_SUCCESS) return NULL;
    PSECURITY_DESCRIPTOR descriptor = LocalAlloc(LPTR, SECURITY_DESCRIPTOR_MIN_LENGTH);
    if (descriptor == NULL || !InitializeSecurityDescriptor(
                                  descriptor,
                                  SECURITY_DESCRIPTOR_REVISION) ||
        !SetSecurityDescriptorOwner(descriptor, user, FALSE) ||
        !SetSecurityDescriptorDacl(descriptor, TRUE, acl, FALSE) ||
        !SetSecurityDescriptorControl(descriptor, SE_DACL_PROTECTED, SE_DACL_PROTECTED)) {
        if (descriptor != NULL) LocalFree(descriptor);
        LocalFree(acl);
        return NULL;
    }
    return descriptor;
}

static void free_current_user_descriptor(PSECURITY_DESCRIPTOR descriptor) {
    if (descriptor == NULL) return;
    BOOL present = FALSE;
    BOOL defaulted = FALSE;
    PACL acl = NULL;
    if (GetSecurityDescriptorDacl(descriptor, &present, &acl, &defaulted) && acl != NULL) {
        LocalFree(acl);
    }
    LocalFree(descriptor);
}

static BOOL trusted_acl_sid(PSID candidate, PSID user) {
    BYTE system_buffer[SECURITY_MAX_SID_SIZE];
    BYTE administrators_buffer[SECURITY_MAX_SID_SIZE];
    DWORD system_length = sizeof(system_buffer);
    DWORD administrators_length = sizeof(administrators_buffer);
    if (!CreateWellKnownSid(WinLocalSystemSid, NULL, system_buffer, &system_length) ||
        !CreateWellKnownSid(
            WinBuiltinAdministratorsSid,
            NULL,
            administrators_buffer,
            &administrators_length)) {
        return FALSE;
    }
    return EqualSid(candidate, user) || EqualSid(candidate, system_buffer) ||
        EqualSid(candidate, administrators_buffer);
}

static BOOL owned_handle(
    HANDLE handle,
    PSID user,
    BOOL require_directory,
    PSECURITY_DESCRIPTOR *descriptor,
    PACL *dacl) {
    FILE_ATTRIBUTE_TAG_INFO tag = {0};
    if (!GetFileInformationByHandleEx(handle, FileAttributeTagInfo, &tag, sizeof(tag)) ||
        (tag.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) ||
        (!!(tag.FileAttributes & FILE_ATTRIBUTE_DIRECTORY) != !!require_directory)) {
        return FALSE;
    }
    PSID owner = NULL;
    const DWORD security_result = GetSecurityInfo(
        handle,
        SE_FILE_OBJECT,
        OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
        &owner,
        NULL,
        dacl,
        NULL,
        descriptor);
    if (security_result != ERROR_SUCCESS || *descriptor == NULL || owner == NULL ||
        !EqualSid(owner, user)) {
        if (*descriptor != NULL) LocalFree(*descriptor);
        *descriptor = NULL;
        *dacl = NULL;
        return FALSE;
    }
    return TRUE;
}

static BOOL private_handle(HANDLE handle, PSID user, BOOL require_directory) {
    PACL dacl = NULL;
    PSECURITY_DESCRIPTOR descriptor = NULL;
    if (!owned_handle(handle, user, require_directory, &descriptor, &dacl)) return FALSE;
    if (dacl == NULL) {
        LocalFree(descriptor);
        return FALSE;
    }
    BOOL private_acl = TRUE;
    for (DWORD index = 0; index < dacl->AceCount; index += 1) {
        ACE_HEADER *header = NULL;
        if (!GetAce(dacl, index, (void **)&header)) {
            private_acl = FALSE;
            break;
        }
        if ((header->AceFlags & INHERIT_ONLY_ACE) != 0) continue;
        if (header->AceType == ACCESS_ALLOWED_ACE_TYPE) {
            ACCESS_ALLOWED_ACE *ace = (ACCESS_ALLOWED_ACE *)header;
            if (!trusted_acl_sid((PSID)&ace->SidStart, user)) {
                private_acl = FALSE;
                break;
            }
        }
    }
    LocalFree(descriptor);
    return private_acl;
}

static int open_private_directory(
    const char *runtime_directory,
    BYTE **user,
    HANDLE *directory) {
    wchar_t path[OB_WIDE_PATH_CAPACITY];
    if (!utf8_to_wide(runtime_directory, path, sizeof(path) / sizeof(path[0])) ||
        user == NULL || directory == NULL) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    *user = current_token_user();
    if (*user == NULL) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    *directory = CreateFileW(
        path,
        READ_CONTROL,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        NULL,
        OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
        NULL);
    if (*directory == INVALID_HANDLE_VALUE) {
        HeapFree(GetProcessHeap(), 0, *user);
        *user = NULL;
        return GetLastError() == ERROR_FILE_NOT_FOUND || GetLastError() == ERROR_PATH_NOT_FOUND
            ? OB_SEMANTIC_ENDPOINT_UNAVAILABLE
            : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    if (!private_handle(*directory, ((TOKEN_USER *)*user)->User.Sid, TRUE)) {
        CloseHandle(*directory);
        HeapFree(GetProcessHeap(), 0, *user);
        *directory = INVALID_HANDLE_VALUE;
        *user = NULL;
        return OB_SEMANTIC_ENDPOINT_INSECURE;
    }
    return OB_SEMANTIC_ENDPOINT_OK;
}

int ob_semantic_endpoint_prepare_runtime_directory(const char *runtime_directory) {
    wchar_t path[OB_WIDE_PATH_CAPACITY];
    if (!utf8_to_wide(runtime_directory, path, sizeof(path) / sizeof(path[0]))) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    BYTE *user = current_token_user();
    if (user == NULL) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    PSECURITY_DESCRIPTOR descriptor = current_user_descriptor(((TOKEN_USER *)user)->User.Sid);
    if (descriptor == NULL) {
        HeapFree(GetProcessHeap(), 0, user);
        return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    SECURITY_ATTRIBUTES security = {
        .nLength = sizeof(security),
        .lpSecurityDescriptor = descriptor,
        .bInheritHandle = FALSE,
    };
    const BOOL created = CreateDirectoryW(path, &security);
    int status = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    HANDLE directory = INVALID_HANDLE_VALUE;
    if (!created && GetLastError() != ERROR_ALREADY_EXISTS) goto finished;
    directory = CreateFileW(
        path,
        READ_CONTROL | (created ? 0 : WRITE_DAC),
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        NULL,
        OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
        NULL);
    if (directory == INVALID_HANDLE_VALUE) goto finished;
    if (!created) {
        PSECURITY_DESCRIPTOR existing = NULL;
        PACL existing_dacl = NULL;
        if (!owned_handle(directory, ((TOKEN_USER *)user)->User.Sid, TRUE, &existing, &existing_dacl)) {
            status = OB_SEMANTIC_ENDPOINT_INSECURE;
            goto finished;
        }
        LocalFree(existing);
        BOOL present = FALSE;
        BOOL defaulted = FALSE;
        PACL dacl = NULL;
        if (!GetSecurityDescriptorDacl(descriptor, &present, &dacl, &defaulted) ||
            !present || dacl == NULL ||
            SetSecurityInfo(
                directory,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                NULL,
                NULL,
                dacl,
                NULL) != ERROR_SUCCESS) {
            goto finished;
        }
    }
    status = private_handle(directory, ((TOKEN_USER *)user)->User.Sid, TRUE)
        ? OB_SEMANTIC_ENDPOINT_OK
        : OB_SEMANTIC_ENDPOINT_INSECURE;
finished:
    if (directory != INVALID_HANDLE_VALUE) CloseHandle(directory);
    free_current_user_descriptor(descriptor);
    HeapFree(GetProcessHeap(), 0, user);
    return status;
}

static int wait_overlapped(
    HANDLE handle,
    OVERLAPPED *overlapped,
    uint32_t timeout_ms,
    DWORD *transferred) {
    const DWORD wait_result = WaitForSingleObject(overlapped->hEvent, timeout_ms);
    if (wait_result == WAIT_TIMEOUT) {
        (void)CancelIoEx(handle, overlapped);
        DWORD ignored = 0;
        (void)GetOverlappedResult(handle, overlapped, &ignored, TRUE);
        return OB_SEMANTIC_ENDPOINT_TIMEOUT;
    }
    if (wait_result != WAIT_OBJECT_0 ||
        !GetOverlappedResult(handle, overlapped, transferred, FALSE)) {
        const DWORD error = GetLastError();
        return error == ERROR_BROKEN_PIPE || error == ERROR_NO_DATA ||
                error == ERROR_PIPE_NOT_CONNECTED
            ? OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED
            : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    return OB_SEMANTIC_ENDPOINT_OK;
}

static int transfer_result(
    HANDLE handle,
    OVERLAPPED *overlapped,
    BOOL started,
    uint32_t timeout_ms,
    DWORD *transferred) {
    if (started) {
        if (GetOverlappedResult(handle, overlapped, transferred, FALSE)) {
            return OB_SEMANTIC_ENDPOINT_OK;
        }
        const DWORD error = GetLastError();
        return error == ERROR_BROKEN_PIPE || error == ERROR_NO_DATA ||
                error == ERROR_PIPE_NOT_CONNECTED
            ? OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED
            : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    const DWORD error = GetLastError();
    if (error != ERROR_IO_PENDING) {
        return error == ERROR_BROKEN_PIPE || error == ERROR_NO_DATA ||
                error == ERROR_PIPE_NOT_CONNECTED
            ? OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED
            : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    return wait_overlapped(handle, overlapped, timeout_ms, transferred);
}

ObSemanticCommandServer *ob_semantic_endpoint_server_create(
    const char *runtime_directory,
    const char *endpoint_id,
    int *status) {
    if (status == NULL || !endpoint_id_is_valid(endpoint_id)) return NULL;
    *status = ob_semantic_endpoint_prepare_runtime_directory(runtime_directory);
    if (*status != OB_SEMANTIC_ENDPOINT_OK) return NULL;
    wchar_t pipe_name[OB_PIPE_NAME_CAPACITY];
    if (!build_pipe_name(endpoint_id, pipe_name, sizeof(pipe_name) / sizeof(pipe_name[0]))) {
        *status = OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
        return NULL;
    }
    BYTE *user = current_token_user();
    if (user == NULL) {
        *status = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        return NULL;
    }
    PSECURITY_DESCRIPTOR descriptor = current_user_descriptor(((TOKEN_USER *)user)->User.Sid);
    if (descriptor == NULL) {
        HeapFree(GetProcessHeap(), 0, user);
        *status = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        return NULL;
    }
    SECURITY_ATTRIBUTES security = {
        .nLength = sizeof(security),
        .lpSecurityDescriptor = descriptor,
        .bInheritHandle = FALSE,
    };
    HANDLE pipe = CreateNamedPipeW(
        pipe_name,
        PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
        1,
        128 * 1024,
        128 * 1024,
        0,
        &security);
    free_current_user_descriptor(descriptor);
    if (pipe == INVALID_HANDLE_VALUE) {
        HeapFree(GetProcessHeap(), 0, user);
        *status = GetLastError() == ERROR_ACCESS_DENIED || GetLastError() == ERROR_PIPE_BUSY
            ? OB_SEMANTIC_ENDPOINT_UNAVAILABLE
            : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        return NULL;
    }
    ObSemanticCommandServer *server = calloc(1, sizeof(*server));
    if (server == NULL) {
        CloseHandle(pipe);
        HeapFree(GetProcessHeap(), 0, user);
        *status = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        return NULL;
    }
    server->pipe = pipe;
    server->token_user = user;
    *status = OB_SEMANTIC_ENDPOINT_OK;
    return server;
}

// All server calls are serialized by the Worker. Completion must be observed even
// when CancelIoEx loses the race, before operation storage or the pipe is reused.
static void quiesce_accept(ObSemanticCommandServer *server) {
    if (server->accept_pending) {
        (void)CancelIoEx(server->pipe, &server->accept_operation);
        DWORD ignored = 0;
        (void)GetOverlappedResult(server->pipe, &server->accept_operation, &ignored, TRUE);
        server->accept_pending = FALSE;
    }
    if (server->accept_operation.hEvent != NULL) CloseHandle(server->accept_operation.hEvent);
    memset(&server->accept_operation, 0, sizeof(server->accept_operation));
}

void ob_semantic_endpoint_server_disconnect(ObSemanticCommandServer *server) {
    if (server == NULL) return;
    quiesce_accept(server);
    server->has_authenticated_byte = FALSE;
    server->authenticated_byte = 0;
    server->phase = ACCEPT_IDLE;
    server->connection_started = 0;
    server->connection_budget = 0;
    if (server->pipe != INVALID_HANDLE_VALUE &&
        !DisconnectNamedPipe(server->pipe) && GetLastError() != ERROR_PIPE_NOT_CONNECTED) {
        // Unknown cleanup failure must never grant the old peer a fresh budget.
        // The exact pending operation is already quiescent before handle retirement.
        CloseHandle(server->pipe);
        server->pipe = INVALID_HANDLE_VALUE;
    }
}

static int close_connection(ObSemanticCommandServer *server, int status) {
    ob_semantic_endpoint_server_disconnect(server);
    server->phase = ACCEPT_CLOSED;
    return status;
}

void ob_semantic_endpoint_server_destroy(ObSemanticCommandServer *server) {
    if (server == NULL) return;
    ob_semantic_endpoint_server_disconnect(server);
    if (server->pipe != INVALID_HANDLE_VALUE) CloseHandle(server->pipe);
    HeapFree(GetProcessHeap(), 0, server->token_user);
    free(server);
}

static int read_before(
    HANDLE handle, uint8_t *buffer, uint32_t capacity, uint32_t timeout_ms, uint32_t *read_bytes);

static uint32_t remaining_time(ULONGLONG started, uint32_t timeout_ms) {
    if (timeout_ms == INFINITE) return INFINITE;
    const ULONGLONG elapsed = GetTickCount64() - started;
    return elapsed >= timeout_ms ? 0 : timeout_ms - (uint32_t)elapsed;
}

static uint32_t connection_remaining(const ObSemanticCommandServer *server) {
    return remaining_time(server->connection_started, server->connection_budget);
}

static int pipe_error(DWORD error) {
    return error == ERROR_BROKEN_PIPE || error == ERROR_NO_DATA || error == ERROR_PIPE_NOT_CONNECTED
        ? OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
}

static int start_accept_operation(ObSemanticCommandServer *server) {
    server->accept_operation.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (server->accept_operation.hEvent == NULL) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    const BOOL started = server->phase == ACCEPT_CONNECTING
        ? ConnectNamedPipe(server->pipe, &server->accept_operation)
        : ReadFile(server->pipe, &server->authenticated_byte, 1, NULL, &server->accept_operation);
    const DWORD error = started ? ERROR_SUCCESS : GetLastError();
    if (error == ERROR_IO_PENDING) {
        server->accept_pending = TRUE;
        return OB_SEMANTIC_ENDPOINT_OK;
    }
    if (error == ERROR_PIPE_CONNECTED && server->phase == ACCEPT_CONNECTING) return OB_SEMANTIC_ENDPOINT_OK;
    if (!started) return pipe_error(error);
    DWORD transferred = 0;
    if (!GetOverlappedResult(server->pipe, &server->accept_operation, &transferred, FALSE)) return pipe_error(GetLastError());
    return server->phase == ACCEPT_IDENTITY && transferred != 1
        ? OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED : OB_SEMANTIC_ENDPOINT_OK;
}

static int poll_accept_operation(ObSemanticCommandServer *server, uint32_t timeout_ms) {
    if (!server->accept_pending) return OB_SEMANTIC_ENDPOINT_OK;
    const DWORD waited = WaitForSingleObject(server->accept_operation.hEvent, timeout_ms);
    // Poll expiry leaves the exact operation and its storage owned by this server.
    if (waited == WAIT_TIMEOUT) return OB_SEMANTIC_ENDPOINT_TIMEOUT;
    if (waited != WAIT_OBJECT_0) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    DWORD transferred = 0;
    const BOOL completed = GetOverlappedResult(server->pipe, &server->accept_operation, &transferred, FALSE);
    const DWORD error = completed ? ERROR_SUCCESS : GetLastError();
    if (error != ERROR_IO_INCOMPLETE) server->accept_pending = FALSE;
    if (!completed) return pipe_error(error);
    return server->phase == ACCEPT_IDENTITY && transferred != 1
        ? OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED : OB_SEMANTIC_ENDPOINT_OK;
}

int ob_semantic_endpoint_server_accept_before(
    ObSemanticCommandServer *server,
    uint32_t timeout_ms,
    uint32_t connection_budget_ms,
    uint32_t *remaining_request_ms) {
    if (server == NULL || remaining_request_ms == NULL || connection_budget_ms == 0) return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    if (server->pipe == INVALID_HANDLE_VALUE) return OB_SEMANTIC_ENDPOINT_UNAVAILABLE;
    if (server->phase == ACCEPT_CLOSED) return OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED;
    if (server->phase == ACCEPT_AUTHENTICATED) return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    const ULONGLONG started = GetTickCount64();
    if (server->phase == ACCEPT_IDLE) {
        server->phase = ACCEPT_CONNECTING;
        const int result = start_accept_operation(server);
        if (result != OB_SEMANTIC_ENDPOINT_OK) return close_connection(server, result);
    }
    if (server->phase == ACCEPT_CONNECTING) {
        const int result = poll_accept_operation(server, remaining_time(started, timeout_ms));
        if (result == OB_SEMANTIC_ENDPOINT_TIMEOUT) return result;
        if (result != OB_SEMANTIC_ENDPOINT_OK) return close_connection(server, result);
        quiesce_accept(server);
        server->connection_started = GetTickCount64();
        server->connection_budget = connection_budget_ms;
        server->phase = ACCEPT_IDENTITY;
    }
    uint32_t remaining = connection_remaining(server);
    if (remaining == 0) return close_connection(server, OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED);
    const uint32_t poll_remaining = remaining_time(started, timeout_ms);
    if (poll_remaining == 0) return OB_SEMANTIC_ENDPOINT_TIMEOUT;
    if (server->accept_operation.hEvent == NULL) {
        const int result = start_accept_operation(server);
        if (result != OB_SEMANTIC_ENDPOINT_OK) return close_connection(server, result);
    }
    remaining = connection_remaining(server);
    if (remaining == 0) return close_connection(server, OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED);
    const uint32_t wait_remaining = remaining_time(started, timeout_ms);
    const int result = poll_accept_operation(server, remaining < wait_remaining ? remaining : wait_remaining);
    if (connection_remaining(server) == 0) return close_connection(server, OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED);
    if (result == OB_SEMANTIC_ENDPOINT_TIMEOUT) return result;
    if (result != OB_SEMANTIC_ENDPOINT_OK) return close_connection(server, result);
    quiesce_accept(server);
    const BOOL impersonating = ImpersonateNamedPipeClient(server->pipe);
    HANDLE client_token = NULL;
    BYTE *client_user = NULL;
    BOOL same_user = FALSE;
    if (impersonating &&
        OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, TRUE, &client_token)) {
        client_user = token_user(client_token);
        if (client_user != NULL) {
            same_user = EqualSid(
                ((TOKEN_USER *)server->token_user)->User.Sid,
                ((TOKEN_USER *)client_user)->User.Sid);
        }
    }
    if (client_user != NULL) HeapFree(GetProcessHeap(), 0, client_user);
    if (client_token != NULL) CloseHandle(client_token);
    if (impersonating && !RevertToSelf()) {
        DWORD exit_code = GetLastError();
        if (exit_code == ERROR_SUCCESS) exit_code = ERROR_CANNOT_IMPERSONATE;
        TerminateProcess(GetCurrentProcess(), exit_code);
        ExitProcess(exit_code);
    }
    if (!same_user) {
        return close_connection(server, impersonating ? OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_MISMATCH
                             : OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_UNAVAILABLE);
    }
    remaining = connection_remaining(server);
    if (remaining == 0) return close_connection(server, OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED);
    server->phase = ACCEPT_AUTHENTICATED;
    server->has_authenticated_byte = TRUE;
    *remaining_request_ms = remaining;
    return OB_SEMANTIC_ENDPOINT_OK;
}

static int read_before(
    HANDLE handle,
    uint8_t *buffer,
    uint32_t capacity,
    uint32_t timeout_ms,
    uint32_t *read_bytes) {
    OVERLAPPED overlapped = {0};
    overlapped.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (overlapped.hEvent == NULL) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    const BOOL started = ReadFile(handle, buffer, capacity, NULL, &overlapped);
    DWORD transferred = 0;
    const int result = transfer_result(
        handle,
        &overlapped,
        started,
        timeout_ms,
        &transferred);
    CloseHandle(overlapped.hEvent);
    if (result != OB_SEMANTIC_ENDPOINT_OK) return result;
    if (transferred == 0) return OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED;
    *read_bytes = (uint32_t)transferred;
    return OB_SEMANTIC_ENDPOINT_OK;
}

static int write_before(
    HANDLE handle,
    const uint8_t *buffer,
    uint32_t length,
    uint32_t timeout_ms,
    uint32_t *written_bytes) {
    OVERLAPPED overlapped = {0};
    overlapped.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (overlapped.hEvent == NULL) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    const BOOL started = WriteFile(handle, buffer, length, NULL, &overlapped);
    DWORD transferred = 0;
    const int result = transfer_result(
        handle,
        &overlapped,
        started,
        timeout_ms,
        &transferred);
    CloseHandle(overlapped.hEvent);
    if (result != OB_SEMANTIC_ENDPOINT_OK) return result;
    if (transferred == 0) return OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED;
    *written_bytes = (uint32_t)transferred;
    return OB_SEMANTIC_ENDPOINT_OK;
}

int ob_semantic_endpoint_server_read_before(
    ObSemanticCommandServer *server,
    uint8_t *buffer,
    uint32_t capacity,
    uint32_t timeout_ms,
    uint32_t *read_bytes) {
    if (server == NULL || buffer == NULL || capacity == 0 || read_bytes == NULL) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    if (server->pipe == INVALID_HANDLE_VALUE) return OB_SEMANTIC_ENDPOINT_UNAVAILABLE;
    if (server->phase != ACCEPT_AUTHENTICATED) return OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED;
    const uint32_t remaining = connection_remaining(server);
    if (remaining == 0) return close_connection(server, OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED);
    if (server->has_authenticated_byte) {
        buffer[0] = server->authenticated_byte;
        *read_bytes = 1;
        server->has_authenticated_byte = FALSE;
        server->authenticated_byte = 0;
        return OB_SEMANTIC_ENDPOINT_OK;
    }
    uint32_t transferred = 0;
    const int result = read_before(server->pipe, buffer, capacity,
        remaining < timeout_ms ? remaining : timeout_ms, &transferred);
    if (connection_remaining(server) == 0) return close_connection(server, OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED);
    if (result != OB_SEMANTIC_ENDPOINT_OK && result != OB_SEMANTIC_ENDPOINT_TIMEOUT) {
        return close_connection(server, result);
    }
    if (result == OB_SEMANTIC_ENDPOINT_OK) *read_bytes = transferred;
    return result;
}

typedef struct {
    HANDLE pipe;
    int status;
} FlushOperation;

static DWORD WINAPI flush_pipe(LPVOID argument) {
    FlushOperation *operation = argument;
    if (FlushFileBuffers(operation->pipe)) {
        operation->status = OB_SEMANTIC_ENDPOINT_OK;
    } else {
        const DWORD error = GetLastError();
        operation->status = error == ERROR_BROKEN_PIPE || error == ERROR_NO_DATA ||
                error == ERROR_PIPE_NOT_CONNECTED
            ? OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED
            : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    return 0;
}

static int flush_before(
    ObSemanticCommandServer *server,
    ULONGLONG started,
    uint32_t timeout_ms) {
    if (remaining_time(started, timeout_ms) == 0) return OB_SEMANTIC_ENDPOINT_TIMEOUT;
    FlushOperation operation = {.pipe = server->pipe, .status = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR};
    HANDLE thread = CreateThread(NULL, 0, flush_pipe, &operation, 0, NULL);
    if (thread == NULL) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    const DWORD waited = WaitForSingleObject(thread, remaining_time(started, timeout_ms));
    int result = waited == WAIT_OBJECT_0 ? operation.status
        : waited == WAIT_TIMEOUT ? OB_SEMANTIC_ENDPOINT_TIMEOUT
                                 : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    if (result == OB_SEMANTIC_ENDPOINT_OK && remaining_time(started, timeout_ms) == 0) {
        result = OB_SEMANTIC_ENDPOINT_TIMEOUT;
    }
    BOOL retire_pipe = FALSE;
    if (result != OB_SEMANTIC_ENDPOINT_OK) {
        server->has_authenticated_byte = FALSE;
        server->authenticated_byte = 0;
        // Disconnect also covers a helper that has not entered FlushFileBuffers
        // yet. Cancellation alone could miss that dispatch window.
        if (!DisconnectNamedPipe(server->pipe) && GetLastError() != ERROR_PIPE_NOT_CONNECTED) {
            result = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
            retire_pipe = TRUE;
        }
    }
    // The peer deadline ends above. Keep the stack context and same pipe owned
    // until OS I/O retirement; neither listener reuse nor destruction may race it.
    // An OS cleanup failure has no promised completion time.
    while (WaitForSingleObject(thread, INFINITE) != WAIT_OBJECT_0) {
        result = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        retire_pipe = TRUE;
        Sleep(1);
    }
    CloseHandle(thread);
    if (retire_pipe) {
        CloseHandle(server->pipe);
        server->pipe = INVALID_HANDLE_VALUE;
    }
    return result;
}

int ob_semantic_endpoint_server_write_before(
    ObSemanticCommandServer *server,
    const uint8_t *buffer,
    uint32_t length,
    uint32_t timeout_ms,
    uint32_t *written_bytes) {
    if (server == NULL || buffer == NULL || length == 0 || written_bytes == NULL) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    if (server->pipe == INVALID_HANDLE_VALUE) return OB_SEMANTIC_ENDPOINT_UNAVAILABLE;
    const ULONGLONG started = GetTickCount64();
    uint32_t transferred = 0;
    const uint32_t remaining = remaining_time(started, timeout_ms);
    int result = remaining == 0 ? OB_SEMANTIC_ENDPOINT_TIMEOUT
        : write_before(server->pipe, buffer, length, remaining, &transferred);
    if (result == OB_SEMANTIC_ENDPOINT_OK) result = flush_before(server, started, timeout_ms);
    if (result != OB_SEMANTIC_ENDPOINT_OK) return close_connection(server, result);
    *written_bytes = transferred;
    return result;
}

static int validate_locator_handle(HANDLE file, PSID user, uint32_t capacity, DWORD *size) {
    if (!private_handle(file, user, FALSE)) return OB_SEMANTIC_ENDPOINT_INSECURE;
    LARGE_INTEGER length;
    if (!GetFileSizeEx(file, &length) || length.QuadPart < 0) {
        return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    if ((uint64_t)length.QuadPart > capacity) return OB_SEMANTIC_ENDPOINT_CAPACITY_EXCEEDED;
    *size = (DWORD)length.QuadPart;
    return OB_SEMANTIC_ENDPOINT_OK;
}

int ob_semantic_endpoint_publish_locator(
    const char *runtime_directory,
    const char *endpoint_id,
    const uint8_t *content,
    uint32_t content_length) {
    if (content == NULL || content_length == 0 || content_length > 4096) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    int status = ob_semantic_endpoint_prepare_runtime_directory(runtime_directory);
    if (status != OB_SEMANTIC_ENDPOINT_OK) return status;
    wchar_t locator_path[OB_WIDE_PATH_CAPACITY];
    wchar_t temporary_path[OB_WIDE_PATH_CAPACITY];
    if (!build_locator_paths(
            runtime_directory,
            endpoint_id,
            locator_path,
            sizeof(locator_path) / sizeof(locator_path[0]),
            temporary_path,
            sizeof(temporary_path) / sizeof(temporary_path[0]))) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    if (GetFileAttributesW(locator_path) != INVALID_FILE_ATTRIBUTES) {
        return OB_SEMANTIC_ENDPOINT_UNAVAILABLE;
    }
    BYTE *user = current_token_user();
    if (user == NULL) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    PSECURITY_DESCRIPTOR descriptor = current_user_descriptor(((TOKEN_USER *)user)->User.Sid);
    if (descriptor == NULL) {
        HeapFree(GetProcessHeap(), 0, user);
        return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    SECURITY_ATTRIBUTES security = {
        .nLength = sizeof(security),
        .lpSecurityDescriptor = descriptor,
        .bInheritHandle = FALSE,
    };
    HANDLE file = CreateFileW(
        temporary_path,
        GENERIC_WRITE,
        0,
        &security,
        CREATE_NEW,
        FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
        NULL);
    free_current_user_descriptor(descriptor);
    if (file == INVALID_HANDLE_VALUE) {
        HeapFree(GetProcessHeap(), 0, user);
        return GetLastError() == ERROR_FILE_EXISTS || GetLastError() == ERROR_ALREADY_EXISTS
            ? OB_SEMANTIC_ENDPOINT_UNAVAILABLE
            : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    DWORD written = 0;
    status = WriteFile(file, content, content_length, &written, NULL) &&
            written == content_length && FlushFileBuffers(file)
        ? OB_SEMANTIC_ENDPOINT_OK
        : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    CloseHandle(file);
    if (status == OB_SEMANTIC_ENDPOINT_OK &&
        !MoveFileExW(temporary_path, locator_path, MOVEFILE_WRITE_THROUGH)) {
        status = GetLastError() == ERROR_ALREADY_EXISTS || GetLastError() == ERROR_FILE_EXISTS
            ? OB_SEMANTIC_ENDPOINT_UNAVAILABLE
            : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    if (status != OB_SEMANTIC_ENDPOINT_OK) (void)DeleteFileW(temporary_path);
    if (status == OB_SEMANTIC_ENDPOINT_OK) {
        HANDLE validation = CreateFileW(
            locator_path,
            READ_CONTROL,
            FILE_SHARE_READ,
            NULL,
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            NULL);
        if (validation == INVALID_HANDLE_VALUE ||
            !private_handle(validation, ((TOKEN_USER *)user)->User.Sid, FALSE)) {
            status = OB_SEMANTIC_ENDPOINT_INSECURE;
        }
        if (validation != INVALID_HANDLE_VALUE) CloseHandle(validation);
    }
    HeapFree(GetProcessHeap(), 0, user);
    return status;
}

int ob_semantic_endpoint_read_locator(
    const char *runtime_directory,
    const char *endpoint_id,
    uint8_t *buffer,
    uint32_t capacity,
    uint32_t *read_bytes) {
    if (buffer == NULL || capacity == 0 || read_bytes == NULL) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    BYTE *directory_user = NULL;
    HANDLE directory = INVALID_HANDLE_VALUE;
    int status = open_private_directory(runtime_directory, &directory_user, &directory);
    if (status != OB_SEMANTIC_ENDPOINT_OK) return status;
    CloseHandle(directory);
    wchar_t locator_path[OB_WIDE_PATH_CAPACITY];
    wchar_t temporary_path[OB_WIDE_PATH_CAPACITY];
    if (!build_locator_paths(
            runtime_directory,
            endpoint_id,
            locator_path,
            sizeof(locator_path) / sizeof(locator_path[0]),
            temporary_path,
            sizeof(temporary_path) / sizeof(temporary_path[0]))) {
        HeapFree(GetProcessHeap(), 0, directory_user);
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    HANDLE file = CreateFileW(
        locator_path,
        GENERIC_READ | READ_CONTROL,
        FILE_SHARE_READ,
        NULL,
        OPEN_EXISTING,
        FILE_FLAG_OPEN_REPARSE_POINT,
        NULL);
    if (file == INVALID_HANDLE_VALUE) {
        HeapFree(GetProcessHeap(), 0, directory_user);
        return GetLastError() == ERROR_FILE_NOT_FOUND ? OB_SEMANTIC_ENDPOINT_UNAVAILABLE
                                                      : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    DWORD size = 0;
    status = validate_locator_handle(
        file,
        ((TOKEN_USER *)directory_user)->User.Sid,
        capacity,
        &size);
    DWORD received = 0;
    if (status == OB_SEMANTIC_ENDPOINT_OK &&
        (!ReadFile(file, buffer, size, &received, NULL) || received != size)) {
        status = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    CloseHandle(file);
    HeapFree(GetProcessHeap(), 0, directory_user);
    if (status == OB_SEMANTIC_ENDPOINT_OK) *read_bytes = received;
    return status;
}

int ob_semantic_endpoint_remove_locator(
    const char *runtime_directory,
    const char *endpoint_id) {
    uint8_t buffer[4096];
    uint32_t ignored = 0;
    int status = ob_semantic_endpoint_read_locator(
        runtime_directory,
        endpoint_id,
        buffer,
        sizeof(buffer),
        &ignored);
    if (status == OB_SEMANTIC_ENDPOINT_UNAVAILABLE) return OB_SEMANTIC_ENDPOINT_OK;
    if (status != OB_SEMANTIC_ENDPOINT_OK) return status;
    wchar_t locator_path[OB_WIDE_PATH_CAPACITY];
    wchar_t temporary_path[OB_WIDE_PATH_CAPACITY];
    if (!build_locator_paths(
            runtime_directory,
            endpoint_id,
            locator_path,
            sizeof(locator_path) / sizeof(locator_path[0]),
            temporary_path,
            sizeof(temporary_path) / sizeof(temporary_path[0]))) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    return DeleteFileW(locator_path) ? OB_SEMANTIC_ENDPOINT_OK
                                     : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
}

static int validate_server_process(HANDLE pipe, PSID current_user) {
    ULONG server_process_id = 0;
    if (!GetNamedPipeServerProcessId(pipe, &server_process_id) || server_process_id == 0) {
        return OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_UNAVAILABLE;
    }
    HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, server_process_id);
    if (process == NULL) return OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_UNAVAILABLE;
    HANDLE token = NULL;
    BYTE *server_user = NULL;
    int result = OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_UNAVAILABLE;
    if (OpenProcessToken(process, TOKEN_QUERY, &token)) {
        server_user = token_user(token);
        if (server_user != NULL) {
            result = EqualSid(
                         current_user,
                         ((TOKEN_USER *)server_user)->User.Sid)
                ? OB_SEMANTIC_ENDPOINT_OK
                : OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_MISMATCH;
        }
    }
    if (server_user != NULL) HeapFree(GetProcessHeap(), 0, server_user);
    if (token != NULL) CloseHandle(token);
    CloseHandle(process);
    return result;
}

int ob_semantic_endpoint_client_exchange(
    const char *runtime_directory,
    const char *endpoint_id,
    const uint8_t *request,
    uint32_t request_length,
    uint8_t *response,
    uint32_t response_capacity,
    uint32_t timeout_ms,
    uint32_t *response_length) {
    if (request == NULL || request_length == 0 || response == NULL || response_capacity == 0 ||
        response_length == NULL) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    BYTE *directory_user = NULL;
    HANDLE directory = INVALID_HANDLE_VALUE;
    int status = open_private_directory(runtime_directory, &directory_user, &directory);
    if (status != OB_SEMANTIC_ENDPOINT_OK) return status;
    CloseHandle(directory);
    wchar_t pipe_name[OB_PIPE_NAME_CAPACITY];
    if (!build_pipe_name(endpoint_id, pipe_name, sizeof(pipe_name) / sizeof(pipe_name[0]))) {
        HeapFree(GetProcessHeap(), 0, directory_user);
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    if (!WaitNamedPipeW(pipe_name, timeout_ms)) {
        const DWORD error = GetLastError();
        HeapFree(GetProcessHeap(), 0, directory_user);
        return error == ERROR_SEM_TIMEOUT ? OB_SEMANTIC_ENDPOINT_TIMEOUT
                                          : OB_SEMANTIC_ENDPOINT_UNAVAILABLE;
    }
    HANDLE pipe = CreateFileW(
        pipe_name,
        GENERIC_READ | GENERIC_WRITE,
        0,
        NULL,
        OPEN_EXISTING,
        FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
        NULL);
    if (pipe == INVALID_HANDLE_VALUE) {
        HeapFree(GetProcessHeap(), 0, directory_user);
        return OB_SEMANTIC_ENDPOINT_UNAVAILABLE;
    }
    status = validate_server_process(pipe, ((TOKEN_USER *)directory_user)->User.Sid);
    HeapFree(GetProcessHeap(), 0, directory_user);
    uint32_t offset = 0;
    while (status == OB_SEMANTIC_ENDPOINT_OK && offset < request_length) {
        uint32_t written = 0;
        status = write_before(
            pipe,
            request + offset,
            request_length - offset,
            timeout_ms,
            &written);
        offset += written;
    }
    uint32_t received = 0;
    while (status == OB_SEMANTIC_ENDPOINT_OK) {
        if (received == response_capacity) {
            status = OB_SEMANTIC_ENDPOINT_CAPACITY_EXCEEDED;
            break;
        }
        uint32_t count = 0;
        status = read_before(
            pipe,
            response + received,
            response_capacity - received,
            timeout_ms,
            &count);
        if (status == OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED && received > 0) {
            status = OB_SEMANTIC_ENDPOINT_OK;
            break;
        }
        received += count;
    }
    CloseHandle(pipe);
    if (status == OB_SEMANTIC_ENDPOINT_OK) *response_length = received;
    return status;
}
