#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stdio.h>

static ULONGLONG clock_value;
static BOOL advance_after_body_read;
static DWORD cancellation_error;
static unsigned quiescence_observations;
static unsigned read_calls;
static BOOL fail_disconnect;
static HANDLE observed_pipe = INVALID_HANDLE_VALUE;
static unsigned pipe_closes;
static ULONGLONG test_clock(void) { return clock_value; }
static BOOL test_read(HANDLE pipe, LPVOID bytes, DWORD length, LPDWORD count, LPOVERLAPPED op) {
    read_calls += 1;
    const BOOL result = ReadFile(pipe, bytes, length, count, op);
    if (advance_after_body_read) clock_value = 100;
    return result;
}
static BOOL test_cancel(HANDLE pipe, LPOVERLAPPED op) {
    const BOOL result = CancelIoEx(pipe, op);
    cancellation_error = result ? ERROR_SUCCESS : GetLastError();
    SetLastError(cancellation_error);
    return result;
}
static BOOL test_result(HANDLE pipe, LPOVERLAPPED op, LPDWORD count, BOOL wait) {
    if (wait) quiescence_observations += 1;
    return GetOverlappedResult(pipe, op, count, wait);
}
static BOOL test_disconnect(HANDLE pipe) {
    if (fail_disconnect) { SetLastError(ERROR_INVALID_FUNCTION); return FALSE; }
    return DisconnectNamedPipe(pipe);
}
static BOOL test_close(HANDLE handle) {
    if (handle == observed_pipe) pipe_closes += 1;
    return CloseHandle(handle);
}
#define DisconnectNamedPipe test_disconnect
#define CloseHandle test_close
#define CancelIoEx test_cancel
#define GetOverlappedResult test_result
#define GetTickCount64 test_clock
#define ReadFile test_read
#include "../semantic_command_endpoint_windows.c"
#undef DisconnectNamedPipe
#undef CloseHandle
#undef CancelIoEx
#undef GetOverlappedResult
#undef GetTickCount64
#undef ReadFile

static unsigned failures;
static void require(BOOL value, const char *claim) {
    printf("%s: %s\n", value ? "PASS" : "FAIL", claim);
    if (!value) failures += 1;
}
static HANDLE connect_peer(const wchar_t *name) {
    return CreateFileW(name, GENERIC_READ | GENERIC_WRITE, 0, NULL, OPEN_EXISTING,
        SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION, NULL);
}
static void send_byte(HANDLE peer, uint8_t byte) {
    DWORD sent = 0;
    require(WriteFile(peer, &byte, 1, &sent, NULL) && sent == 1, "first write survives accept poll boundary");
}
static uint32_t receipt;
static int poll_accept(ObSemanticCommandServer *server) {
    return ob_semantic_endpoint_server_accept_before(server, 10, 100, &receipt);
}
int main(int argc, char **argv) {
    if (argc != 2) return 2;
    char endpoint[33];
    snprintf(endpoint, sizeof(endpoint), "%032lx", (unsigned long)GetCurrentProcessId());
    wchar_t name[OB_PIPE_NAME_CAPACITY];
    if (!build_pipe_name(endpoint, name, OB_PIPE_NAME_CAPACITY)) return 2;
    int status = 0;
    ObSemanticCommandServer *server = ob_semantic_endpoint_server_create(argv[1], endpoint, &status);
    if (server == NULL) return 2;
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_TIMEOUT, "idle poll returns without a client");
    require(WaitNamedPipeW(name, 100) != 0, "idle poll retains a listening pipe discoverable by the client");
    clock_value = 1000;
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_TIMEOUT, "idle listener survives beyond connection budget");
    HANDLE peer = connect_peer(name);
    require(peer != INVALID_HANDLE_VALUE, "client open after availability and expired poll reaches same listener");
    if (peer == INVALID_HANDLE_VALUE) { ob_semantic_endpoint_server_destroy(server); return 1; }
    clock_value = 0;
    receipt = 77;
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_TIMEOUT && receipt == 77,
        "connected preread poll retains ownership without publishing a budget receipt");
    clock_value = 60;
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_TIMEOUT,
        "second preread poll preserves original connection deadline");
    send_byte(peer, 'A');
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_OK && receipt == 40,
        "authentication returns only original remaining connection budget");
    uint8_t byte = 77;
    uint32_t count = 91;
    clock_value = 100;
    require(ob_semantic_endpoint_server_read_before(server, &byte, 1, 10, &count) == OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED && byte == 77 && count == 91,
        "scheduling beyond receipt deadline cannot publish held authenticated prefix");
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED,
        "expired connection stays closed until explicit owner disconnect");
    ob_semantic_endpoint_server_disconnect(server);
    CloseHandle(peer);

    clock_value = 0;
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_TIMEOUT, "explicit disconnect starts a fresh listener");
    peer = connect_peer(name);
    if (peer == INVALID_HANDLE_VALUE) return 2;
    send_byte(peer, 'B');
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_OK, "successor authenticates on reused listener");
    require(ob_semantic_endpoint_server_read_before(server, &byte, 1, 10, &count) == OB_SEMANTIC_ENDPOINT_OK && byte == 'B' && count == 1,
        "successor receives its own prefix exactly once");
    send_byte(peer, 'C');
    byte = 77; count = 91;
    advance_after_body_read = TRUE;
    require(ob_semantic_endpoint_server_read_before(server, &byte, 1, 10, &count) == OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED && count == 91,
        "body completion at original deadline publishes no successful byte count");
    advance_after_body_read = FALSE;
    ob_semantic_endpoint_server_disconnect(server);
    CloseHandle(peer);

    clock_value = 0;
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_TIMEOUT, "listener reenters after body expiry");
    peer = connect_peer(name);
    if (peer == INVALID_HANDLE_VALUE) return 2;
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_TIMEOUT, "pending identity read starts");
    clock_value = 100;
    const unsigned prior_reads = read_calls;
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED && read_calls == prior_reads,
        "identity deadline cancels exact pending read without restarting its budget");
    ob_semantic_endpoint_server_disconnect(server);
    CloseHandle(peer);

    clock_value = 0;
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_TIMEOUT, "listener starts before stop during connect");
    ob_semantic_endpoint_server_destroy(server);
    server = ob_semantic_endpoint_server_create(argv[1], endpoint, &status);
    require(server != NULL, "close quiesces pending connect before same endpoint recreation");
    if (server == NULL) return 2;
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_TIMEOUT, "recreated listener starts");
    peer = connect_peer(name);
    if (peer == INVALID_HANDLE_VALUE) return 2;
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_TIMEOUT, "pending identity read exists before stop");
    ob_semantic_endpoint_server_destroy(server);
    CloseHandle(peer);
    server = ob_semantic_endpoint_server_create(argv[1], endpoint, &status);
    require(server != NULL, "close quiesces pending preread before same endpoint recreation");
    if (server == NULL) return 2;
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_TIMEOUT, "listener starts before completed-read cancellation race");
    peer = connect_peer(name);
    if (peer == INVALID_HANDLE_VALUE) return 2;
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_TIMEOUT, "one preread remains pending before completion wins cancellation");
    send_byte(peer, 'D');
    require(WaitForSingleObject(server->accept_operation.hEvent, 1000) == WAIT_OBJECT_0,
        "real first-byte completion wins before cancellation is requested");
    const unsigned before_quiescence = quiescence_observations;
    ob_semantic_endpoint_server_disconnect(server);
    require(cancellation_error == ERROR_NOT_FOUND && quiescence_observations == before_quiescence + 1,
        "CancelIoEx NOT_FOUND still observes exact operation completion before releasing storage");
    CloseHandle(peer);
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_TIMEOUT, "completion-before-cancel leaves a reusable listener");
    peer = connect_peer(name);
    if (peer == INVALID_HANDLE_VALUE) return 2;
    send_byte(peer, 'E');
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_OK, "post-cancellation successor authenticates");
    require(ob_semantic_endpoint_server_read_before(server, &byte, 1, 10, &count) == OB_SEMANTIC_ENDPOINT_OK && byte == 'E',
        "completed but cancelled old prefix never reaches the successor");
    observed_pipe = server->pipe;
    fail_disconnect = TRUE;
    ob_semantic_endpoint_server_disconnect(server);
    fail_disconnect = FALSE;
    require(server->pipe == INVALID_HANDLE_VALUE && pipe_closes == 1,
        "unknown disconnect failure retires the original pipe while the old peer still owns its handle");
    receipt = 77;
    require(poll_accept(server) == OB_SEMANTIC_ENDPOINT_UNAVAILABLE && receipt == 77,
        "retired listener cannot authenticate the old peer or publish a renewed budget");
    require(ob_semantic_endpoint_server_read_before(server, &byte, 1, 10, &count) == OB_SEMANTIC_ENDPOINT_UNAVAILABLE,
        "retired listener rejects reads before the cached prefix path");
    require(ob_semantic_endpoint_server_write_before(server, &byte, 1, 10, &count) == OB_SEMANTIC_ENDPOINT_UNAVAILABLE,
        "retired listener rejects writes");
    ob_semantic_endpoint_server_destroy(server);
    require(pipe_closes == 1, "destroy never closes a retired pipe twice");
    CloseHandle(peer);
    printf("Windows persistent accept: %u failure(s).\n", failures);
    return failures == 0 ? 0 : 1;
}
