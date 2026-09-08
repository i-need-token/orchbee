#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stdio.h>

// The public write boundary runs against real named pipes. Events control the
// client read and the helper's pre-dispatch window; no response bytes are faked.
static HANDLE flush_entered, release_flush, disconnected;
static BOOL hold_flush, fail_thread, fail_disconnect;
static BOOL expire_after_write;
static ULONGLONG clock_start;
static unsigned clock_reads;
static DWORD writer_id;
static DWORD flush_wait_budget;

static BOOL observed_flush(HANDLE pipe) {
    SetEvent(flush_entered);
    if (hold_flush && WaitForSingleObject(release_flush, 5000) != WAIT_OBJECT_0) ExitProcess(90);
    return FlushFileBuffers(pipe);
}
static HANDLE observed_create_thread(LPSECURITY_ATTRIBUTES attributes, SIZE_T stack,
    LPTHREAD_START_ROUTINE function, LPVOID argument, DWORD flags, LPDWORD identifier) {
    if (fail_thread) { SetLastError(ERROR_NOT_ENOUGH_MEMORY); return NULL; }
    return CreateThread(attributes, stack, function, argument, flags, identifier);
}
static BOOL observed_disconnect(HANDLE pipe) {
    BOOL result;
    if (fail_disconnect) { SetLastError(ERROR_ACCESS_DENIED); result = FALSE; }
    else result = DisconnectNamedPipe(pipe);
    SetEvent(disconnected);
    return result;
}
static ULONGLONG observed_clock(void) {
    if (!expire_after_write) return GetTickCount64();
    return clock_reads++ < 2 ? clock_start : clock_start + 100;
}
static DWORD observed_wait(HANDLE handle, DWORD timeout) {
    if (GetCurrentThreadId() == writer_id && timeout != INFINITE) flush_wait_budget = timeout;
    return WaitForSingleObject(handle, timeout);
}
#define FlushFileBuffers observed_flush
#define CreateThread observed_create_thread
#define DisconnectNamedPipe observed_disconnect
#define GetTickCount64 observed_clock
#define WaitForSingleObject observed_wait
#include "../semantic_command_endpoint_windows.c"
#undef FlushFileBuffers
#undef CreateThread
#undef DisconnectNamedPipe
#undef GetTickCount64
#undef WaitForSingleObject

static unsigned failures;
static const uint8_t response[] = {'r', 0, 's', 255, 'p', 'o', 'n', 's', 'e'};
static void require(BOOL valid, const char *claim) {
    printf("%s: %s\n", valid ? "PASS" : "FAIL", claim);
    if (!valid) failures += 1;
}
static void await_event(HANDLE event) {
    if (WaitForSingleObject(event, 5000) != WAIT_OBJECT_0) ExitProcess(91);
}
typedef struct {
    ObSemanticCommandServer *server;
    uint32_t timeout, written;
    int result;
} Write;
static DWORD WINAPI write_thread(LPVOID argument) {
    Write *write = argument;
    writer_id = GetCurrentThreadId();
    write->result = ob_semantic_endpoint_server_write_before(write->server, response,
        sizeof(response), write->timeout, &write->written);
    return 0;
}
typedef struct { ObSemanticCommandServer *server; int result; } Accept;
static DWORD WINAPI accept_thread(LPVOID argument) {
    Accept *accept = argument;
    accept->result = ob_semantic_endpoint_server_accept_before(accept->server, 1000);
    return 0;
}
static HANDLE authenticated_client(ObSemanticCommandServer *server, const wchar_t *name, uint8_t marker) {
    Accept accept = {.server = server, .result = -1};
    HANDLE accepting = CreateThread(NULL, 0, accept_thread, &accept, 0, NULL);
    if (accepting == NULL || !WaitNamedPipeW(name, 1000)) ExitProcess(95);
    HANDLE client = CreateFileW(name, GENERIC_READ | GENERIC_WRITE, 0, NULL, OPEN_EXISTING,
        FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION, NULL);
    if (client == INVALID_HANDLE_VALUE) ExitProcess(92);
    uint32_t count = 0;
    if (write_before(client, &marker, 1, 1000, &count) != OB_SEMANTIC_ENDPOINT_OK) ExitProcess(93);
    await_event(accepting); CloseHandle(accepting);
    require(accept.result == OB_SEMANTIC_ENDPOINT_OK,
        "same listener authenticates the current client");
    uint8_t received = 0;
    count = 0;
    require(ob_semantic_endpoint_server_read_before(server, &received, 1, 1000, &count) ==
        OB_SEMANTIC_ENDPOINT_OK && count == 1 && received == marker,
        "current authenticated prefix is returned exactly once");
    ResetEvent(flush_entered); ResetEvent(release_flush); ResetEvent(disconnected);
    return client;
}
static void read_response(HANDLE client, HANDLE writer) {
    uint8_t bytes[sizeof(response)] = {0};
    uint32_t received = 0;
    while (received < sizeof(bytes)) {
        uint32_t count = 0;
        int status = read_before(client, bytes + received, 3, 1000, &count);
        if (status != OB_SEMANTIC_ENDPOINT_OK) { require(FALSE, "response read succeeds"); return; }
        received += count;
        if (received < sizeof(bytes)) require(WaitForSingleObject(writer, 0) == WAIT_TIMEOUT,
            "partial client reads cannot publish write success");
    }
    require(memcmp(bytes, response, sizeof(bytes)) == 0, "delayed client reads the exact binary response");
}
static HANDLE start_write(Write *write, ObSemanticCommandServer *server, uint32_t timeout) {
    *write = (Write){.server = server, .timeout = timeout, .written = 77, .result = -1};
    HANDLE thread = CreateThread(NULL, 0, write_thread, write, 0, NULL);
    if (thread == NULL) ExitProcess(94);
    return thread;
}
static void finish_write(HANDLE thread) { await_event(thread); CloseHandle(thread); }

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    flush_entered = CreateEventW(NULL, TRUE, FALSE, NULL);
    release_flush = CreateEventW(NULL, TRUE, FALSE, NULL);
    disconnected = CreateEventW(NULL, TRUE, FALSE, NULL);
    const char *endpoint = "dededededededededededededededede";
    wchar_t name[OB_PIPE_NAME_CAPACITY];
    if (!build_pipe_name(endpoint, name, OB_PIPE_NAME_CAPACITY)) return 3;
    int status = ob_semantic_endpoint_prepare_runtime_directory(argv[1]);
    if (status != OB_SEMANTIC_ENDPOINT_OK) return 4;
    ObSemanticCommandServer *server = ob_semantic_endpoint_server_create(argv[1], endpoint, &status);
    if (server == NULL) return 5;
    Write write;

    HANDLE client = authenticated_client(server, name, 'A');
    HANDLE thread = start_write(&write, server, 1000);
    HANDLE first_events[] = {flush_entered, thread};
    if (WaitForMultipleObjects(2, first_events, FALSE, 5000) != WAIT_OBJECT_0) {
        require(FALSE, "write cannot return before the client reads its response");
        finish_write(thread); ob_semantic_endpoint_server_disconnect(server); CloseHandle(client);
        ob_semantic_endpoint_server_destroy(server);
        CloseHandle(flush_entered); CloseHandle(release_flush); CloseHandle(disconnected);
        return 1;
    }
    require(WaitForSingleObject(thread, 0) == WAIT_TIMEOUT,
        "write success waits for actual peer consumption");
    read_response(client, thread);
    finish_write(thread);
    require(write.result == OB_SEMANTIC_ENDPOINT_OK && write.written == sizeof(response),
        "consumed response returns its exact successful byte count");
    ob_semantic_endpoint_server_disconnect(server);
    uint8_t byte = 0; uint32_t count = 0;
    require(read_before(client, &byte, 1, 1000, &count) == OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED,
        "disconnect follows consumption and exposes EOF");
    CloseHandle(client);

    for (unsigned before_dispatch = 0; before_dispatch < 2; before_dispatch += 1) {
        client = authenticated_client(server, name, (uint8_t)('B' + before_dispatch));
        hold_flush = before_dispatch != 0;
        thread = start_write(&write, server, 100);
        await_event(flush_entered);
        await_event(disconnected);
        if (hold_flush) {
            require(WaitForSingleObject(thread, 0) == WAIT_TIMEOUT,
                "deadline retains helper context until pre-dispatch helper retires");
            SetEvent(release_flush);
        }
        finish_write(thread);
        require(write.result == OB_SEMANTIC_ENDPOINT_TIMEOUT && write.written == 77,
            "timeout cannot publish a partial or late successful write");
        require(flush_wait_budget <= 100,
            "flush wait uses the remaining original write budget");
        count = 0;
        require(read_before(client, &byte, 1, 100, &count) == OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED,
            "deadline disconnect releases the stalled peer connection");
        CloseHandle(client); hold_flush = FALSE;
    }

    client = authenticated_client(server, name, 'D');
    thread = start_write(&write, server, 1000);
    await_event(flush_entered);
    CloseHandle(client);
    finish_write(thread);
    require(write.result == OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED && write.written == 77,
        "peer close during flush fails without claiming delivery");

    client = authenticated_client(server, name, 'E');
    fail_thread = TRUE;
    thread = start_write(&write, server, 1000);
    finish_write(thread); fail_thread = FALSE;
    require(write.result == OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR && write.written == 77,
        "helper creation failure exposes no successful write count");
    CloseHandle(client);

    client = authenticated_client(server, name, 'F');
    expire_after_write = TRUE; clock_reads = 0; clock_start = GetTickCount64();
    thread = start_write(&write, server, 100);
    finish_write(thread); expire_after_write = FALSE;
    require(write.result == OB_SEMANTIC_ENDPOINT_TIMEOUT && write.written == 77 &&
        WaitForSingleObject(flush_entered, 0) == WAIT_TIMEOUT,
        "write exhaustion starts no helper and does not renew the deadline");
    CloseHandle(client);

    client = authenticated_client(server, name, 'G');
    hold_flush = TRUE; fail_disconnect = TRUE;
    thread = start_write(&write, server, 100);
    await_event(flush_entered); await_event(disconnected);
    require(WaitForSingleObject(thread, 0) == WAIT_TIMEOUT,
        "unknown OS disconnect failure retains helper ownership");
    CloseHandle(client); SetEvent(release_flush);
    finish_write(thread); fail_disconnect = FALSE; hold_flush = FALSE;
    require(write.result == OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR && write.written == 77 &&
        server->pipe == INVALID_HANDLE_VALUE,
        "unknown disconnect failure retires the listener after helper quiescence");
    require(ob_semantic_endpoint_server_accept_before(server, 100) == OB_SEMANTIC_ENDPOINT_UNAVAILABLE,
        "retired listener cannot authenticate or reuse the old pipe");
    ob_semantic_endpoint_server_destroy(server);
    CloseHandle(flush_entered); CloseHandle(release_flush); CloseHandle(disconnected);
    printf("Windows semantic endpoint response: %u failure(s).\n", failures);
    return failures == 0 ? 0 : 1;
}
