#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>
#include <stdint.h>

// Real named-pipe I/O; events fix the first-write ordering. Clock and identity
// failure seams cover budget/refusal cuts without changing accounts or tokens.
static HANDLE read_started;
static BOOL reject_identity;
static BOOL omit_identity;
static BOOL fail_impersonation;
static BOOL close_before_write;
static BOOL expire_after_read;
static LPOVERLAPPED preread_operation;
static BOOL preread_completed;
static BOOL scripted_clock;
static ULONGLONG clock_after_start;
static unsigned clock_reads;
static DWORD last_wait;
static unsigned pipe_reads;
static DWORD client_flags;
static uint32_t accept_timeout = 1000;
static uint32_t connection_budget = 1000;

static BOOL observed_read(HANDLE file, LPVOID buffer, DWORD capacity, LPDWORD read, LPOVERLAPPED overlapped) {
    pipe_reads += 1;
    preread_operation = overlapped;
    SetEvent(read_started);
    return ReadFile(file, buffer, capacity, read, overlapped);
}
static BOOL observed_completion(HANDLE file, LPOVERLAPPED overlapped, LPDWORD transferred, BOOL wait) {
    const BOOL completed = GetOverlappedResult(file, overlapped, transferred, wait);
    if (completed && overlapped == preread_operation && *transferred == 1) preread_completed = TRUE;
    return completed;
}
static BOOL observed_equal_sid(PSID left, PSID right) {
    return reject_identity ? FALSE : EqualSid(left, right);
}
static BOOL observed_thread_token(HANDLE thread, DWORD access, BOOL self, PHANDLE token) {
    if (omit_identity) { SetLastError(ERROR_NO_TOKEN); return FALSE; }
    return OpenThreadToken(thread, access, self, token);
}
static BOOL observed_impersonation(HANDLE pipe) {
    if (fail_impersonation) { SetLastError(ERROR_CANNOT_IMPERSONATE); return FALSE; }
    return ImpersonateNamedPipeClient(pipe);
}
ULONGLONG observed_clock(void) {
    if (!scripted_clock) return GetTickCount64();
    if (expire_after_read && preread_completed) return 100;
    return clock_reads++ == 0 ? 0 : clock_after_start;
}
static DWORD observed_wait(HANDLE event, DWORD timeout) {
    last_wait = timeout;
    return WaitForSingleObject(event, timeout);
}
#define ReadFile observed_read
#define GetOverlappedResult observed_completion
#define EqualSid observed_equal_sid
#define OpenThreadToken observed_thread_token
#define ImpersonateNamedPipeClient observed_impersonation
#define GetTickCount64 observed_clock
#define WaitForSingleObject observed_wait
#include "../semantic_command_endpoint_windows.c"
#undef ReadFile
#undef GetOverlappedResult
#undef EqualSid
#undef OpenThreadToken
#undef ImpersonateNamedPipeClient
#undef GetTickCount64
#undef WaitForSingleObject

static unsigned failures;
static void require(BOOL valid, const char *claim) {
    printf("%s: %s\n", valid ? "PASS" : "FAIL", claim);
    if (!valid) failures += 1;
}
typedef struct {
    ObSemanticCommandServer *server;
    HANDLE done;
    uint32_t timeout;
    int result;
    BOOL reverted;
} Accept;
static DWORD WINAPI accept_thread(LPVOID argument) {
    Accept *accept = argument;
    uint32_t remaining = 0;
    accept->result = ob_semantic_endpoint_server_accept_before(accept->server, accept->timeout, connection_budget, &remaining);
    HANDLE token = NULL;
    accept->reverted = !OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, TRUE, &token) && GetLastError() == ERROR_NO_TOKEN;
    if (token != NULL) CloseHandle(token);
    SetEvent(accept->done);
    return 0;
}
static HANDLE connect_client(const wchar_t *name) {
    return CreateFileW(name, GENERIC_READ | GENERIC_WRITE, 0, NULL, OPEN_EXISTING,
        client_flags, NULL);
}
static int accept_with_payload(ObSemanticCommandServer *server, HANDLE *client, const wchar_t *name,
    const uint8_t *bytes, DWORD length, BOOL expect_preread) {
    ResetEvent(read_started);
    preread_operation = NULL;
    preread_completed = FALSE;
    Accept accept = {.server = server, .done = CreateEventW(NULL, TRUE, FALSE, NULL), .timeout = accept_timeout};
    HANDLE thread = CreateThread(NULL, 0, accept_thread, &accept, 0, NULL);
    if (*client == INVALID_HANDLE_VALUE) {
        if (!WaitNamedPipeW(name, 1000)) ExitProcess(2);
        *client = connect_client(name);
        if (*client == INVALID_HANDLE_VALUE) ExitProcess(2);
    }
    HANDLE events[] = {read_started, accept.done};
    DWORD event = WaitForMultipleObjects(2, events, FALSE, 2000);
    if (expect_preread && event != WAIT_OBJECT_0 && event != WAIT_OBJECT_0 + 1) ExitProcess(2);
    if (bytes != NULL) {
        DWORD written = 0;
        const BOOL sent = WriteFile(*client, bytes, length, &written, NULL);
        if (!sent) printf("Observed first-write Win32 error: %lu\n", GetLastError());
        require(sent && written == length,
            "first client write survives controlled connect-before-write ordering");
    }
    if (close_before_write) {
        CloseHandle(*client);
        *client = INVALID_HANDLE_VALUE;
    }
    if (WaitForSingleObject(thread, 3000) != WAIT_OBJECT_0) ExitProcess(2);
    require(accept.reverted, "accept thread returns without impersonation");
    CloseHandle(thread);
    CloseHandle(accept.done);
    return accept.result;
}
static void disconnect(ObSemanticCommandServer *server, HANDLE client) {
    ob_semantic_endpoint_server_disconnect(server);
    if (client != INVALID_HANDLE_VALUE) CloseHandle(client);
}

int wmain(int argc, wchar_t **argv) {
    if (argc != 2) return 2;
    char runtime[4096];
    if (!WideCharToMultiByte(CP_UTF8, 0, argv[1], -1, runtime, sizeof(runtime), NULL, NULL)) return 2;
    read_started = CreateEventW(NULL, TRUE, FALSE, NULL);
    char endpoint[33];
    snprintf(endpoint, sizeof(endpoint), "%032lx", (unsigned long)GetCurrentProcessId());
    wchar_t pipe_name[OB_PIPE_NAME_CAPACITY];
    if (!build_pipe_name(endpoint, pipe_name, OB_PIPE_NAME_CAPACITY)) return 2;
    int status = 0;
    ObSemanticCommandServer *server = ob_semantic_endpoint_server_create(runtime, endpoint, &status);
    if (server == NULL) return 2;
    const uint8_t payload[] = {0, 0xff, 'x'};
    HANDLE client = connect_client(pipe_name);
    require(client != INVALID_HANDLE_VALUE, "real same-user client connects before any write");
    status = accept_with_payload(server, &client, pipe_name, payload, sizeof(payload), TRUE);
    require(status == OB_SEMANTIC_ENDPOINT_OK, "controlled same-user accept succeeds");
    printf("Observed actual Desktop accept status with default client flags: %d\n", status);
    if (status == OB_SEMANTIC_ENDPOINT_OK) {
        uint8_t invalid_byte = 77;
        uint32_t invalid_count = 91;
        require(ob_semantic_endpoint_server_read_before(server, &invalid_byte, 0, 100, &invalid_count) == OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT &&
            invalid_byte == 77 && invalid_count == 91,
            "zero-capacity read rejects without changing caller outputs or consuming buffered data");
        require(ob_semantic_endpoint_server_read_before(server, &invalid_byte, 1, 100, NULL) == OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT &&
            ob_semantic_endpoint_server_read_before(server, NULL, 1, 100, &invalid_count) == OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT &&
            invalid_byte == 77 && invalid_count == 91,
            "null read outputs reject without consuming buffered data");
        uint8_t received[sizeof(payload)] = {0};
        uint32_t total = 0;
        while (total < sizeof(received)) {
            uint32_t read = 0;
            status = ob_semantic_endpoint_server_read_before(server, received + total, sizeof(received) - total, 100, &read);
            if (status != OB_SEMANTIC_ENDPOINT_OK || read == 0) break;
            total += read;
        }
        require(total == sizeof(payload) && memcmp(received, payload, sizeof(payload)) == 0,
            "authenticated binary preread and stream tail return exactly once in order");
        uint8_t delayed[4] = {0};
        uint32_t delayed_read = 0;
        require(ob_semantic_endpoint_server_read_before(server, delayed, sizeof(delayed), 250, &delayed_read) == OB_SEMANTIC_ENDPOINT_TIMEOUT,
            "authenticated idle read reports the worker polling timeout");
        const uint8_t tail[] = {'t', 'a', 'i', 'l'};
        DWORD sent = 0;
        require(WriteFile(client, tail, sizeof(tail), &sent, NULL) && sent == sizeof(tail),
            "same authenticated peer can write its delayed fragment after read polling timeout");
        require(ob_semantic_endpoint_server_read_before(server, delayed, sizeof(delayed), 250, &delayed_read) == OB_SEMANTIC_ENDPOINT_OK &&
            delayed_read == sizeof(tail) && memcmp(delayed, tail, sizeof(tail)) == 0,
            "delayed fragment arrives exactly once without replaying the consumed first byte");
    }
    disconnect(server, client);

    uint32_t remaining = 0;
    require(ob_semantic_endpoint_server_accept_before(server, 10, 1000, &remaining) == OB_SEMANTIC_ENDPOINT_TIMEOUT,
        "absent client poll preserves pending connect for the next call");
    client = INVALID_HANDLE_VALUE;
    pipe_reads = 0;
    accept_timeout = 100;
    status = accept_with_payload(server, &client, pipe_name, NULL, 0, FALSE);
    require(status == OB_SEMANTIC_ENDPOINT_TIMEOUT && pipe_reads == 1,
        "connected peer without first byte returns poll timeout and retains pending read");
    disconnect(server, client);
    client = INVALID_HANDLE_VALUE;
    close_before_write = TRUE;
    status = accept_with_payload(server, &client, pipe_name, NULL, 0, TRUE);
    close_before_write = FALSE;
    require(status == OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED,
        "peer EOF before first byte rejects admission and releases pending read");
    disconnect(server, client);

    scripted_clock = TRUE;
    clock_after_start = 90;
    clock_reads = 0;
    last_wait = INFINITE;
    ob_semantic_endpoint_server_destroy(server);
    server = ob_semantic_endpoint_server_create(runtime, endpoint, &status);
    if (server == NULL) return 2;
    client = connect_client(pipe_name);
    status = accept_with_payload(server, &client, pipe_name, NULL, 0, FALSE);
    require(status == OB_SEMANTIC_ENDPOINT_TIMEOUT && last_wait == 10,
        "preread receives only ten milliseconds remaining from original accept budget");
    disconnect(server, client);
    clock_after_start = 100;
    clock_reads = 0;
    pipe_reads = 0;
    ob_semantic_endpoint_server_destroy(server);
    server = ob_semantic_endpoint_server_create(runtime, endpoint, &status);
    if (server == NULL) return 2;
    client = connect_client(pipe_name);
    status = accept_with_payload(server, &client, pipe_name, NULL, 0, FALSE);
    require(status == OB_SEMANTIC_ENDPOINT_TIMEOUT && pipe_reads == 0,
        "expired original accept budget starts no preread");
    disconnect(server, client);
    connection_budget = 100;
    expire_after_read = TRUE;
    clock_after_start = 0;
    clock_reads = 0;
    pipe_reads = 0;
    client = INVALID_HANDLE_VALUE;
    status = accept_with_payload(server, &client, pipe_name, payload, 1, TRUE);
    require(status == OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED && preread_completed,
        "deadline reached after first byte cannot publish authenticated admission");
    uint8_t expired_byte = 77;
    uint32_t expired_count = 0;
    require(ob_semantic_endpoint_server_read_before(server, &expired_byte, 1, 10, &expired_count) != OB_SEMANTIC_ENDPOINT_OK && expired_byte == 77,
        "expired admission retains no readable byte");
    disconnect(server, client);
    expire_after_read = FALSE;
    scripted_clock = FALSE;
    accept_timeout = 1000;
    connection_budget = 1000;

    for (unsigned failure = 0; failure < 3; failure += 1) {
        client = INVALID_HANDLE_VALUE;
        reject_identity = failure == 0;
        omit_identity = failure == 1;
        fail_impersonation = failure == 2;
        status = accept_with_payload(server, &client, pipe_name, payload, sizeof(payload), TRUE);
        reject_identity = FALSE;
        omit_identity = FALSE;
        fail_impersonation = FALSE;
        require(status == (failure == 2 ? OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_UNAVAILABLE : OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_MISMATCH),
            "mismatched token, missing token, or unavailable impersonation fails closed");
        uint8_t byte = 77;
        uint32_t read = 0;
        require(ob_semantic_endpoint_server_read_before(server, &byte, 1, 10, &read) != OB_SEMANTIC_ENDPOINT_OK && byte == 77,
            "identity refusal exposes no buffered byte");
        disconnect(server, client);
    }

    client = INVALID_HANDLE_VALUE;
    status = accept_with_payload(server, &client, pipe_name, payload, 1, TRUE);
    require(status == OB_SEMANTIC_ENDPOINT_OK, "authenticated unread byte exists before peer write failure");
    CloseHandle(client);
    client = INVALID_HANDLE_VALUE;
    uint32_t written = 0;
    require(ob_semantic_endpoint_server_write_before(server, payload, 1, 100, &written) == OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED,
        "write observes peer EOF while an authenticated byte is buffered");
    uint8_t refused_byte = 77;
    uint32_t refused_count = 0;
    require(ob_semantic_endpoint_server_read_before(server, &refused_byte, 1, 10, &refused_count) != OB_SEMANTIC_ENDPOINT_OK && refused_byte == 77,
        "connection write failure clears the unread authenticated byte");
    disconnect(server, client);

    // Leave an authenticated byte unread, disconnect, then prove the next peer owns its byte.
    client = INVALID_HANDLE_VALUE;
    status = accept_with_payload(server, &client, pipe_name, payload, 1, TRUE);
    require(status == OB_SEMANTIC_ENDPOINT_OK, "unread authenticated buffer is established");
    disconnect(server, client);
    client_flags = SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION;
    accept_timeout = INFINITE;
    connection_budget = INFINITE;
    client = INVALID_HANDLE_VALUE;
    const uint8_t next = 'N';
    status = accept_with_payload(server, &client, pipe_name, &next, 1, TRUE);
    require(status == OB_SEMANTIC_ENDPOINT_OK, "same listener accepts bundled-SQOS successor with infinite budget after timeout/refusal/disconnect");
    if (status == OB_SEMANTIC_ENDPOINT_OK) {
        CloseHandle(client);
        client = INVALID_HANDLE_VALUE;
        uint8_t byte = 0;
        uint32_t read = 0;
        require(ob_semantic_endpoint_server_read_before(server, &byte, 1, 100, &read) == OB_SEMANTIC_ENDPOINT_OK && read == 1 && byte == next,
            "peer EOF preserves its authenticated byte and never replays the previous peer");
        require(ob_semantic_endpoint_server_read_before(server, &byte, 1, 100, &read) == OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED,
            "drained preread exposes underlying peer EOF");
    }
    if (client != INVALID_HANDLE_VALUE) CloseHandle(client);
    ob_semantic_endpoint_server_destroy(server);
    CloseHandle(read_started);
    printf("Windows semantic endpoint accept: %u failure(s).\n", failures);
    return failures == 0 ? 0 : 1;
}
