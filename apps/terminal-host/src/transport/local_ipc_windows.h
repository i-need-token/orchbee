#ifndef ORCHBEE_LOCAL_IPC_WINDOWS_H
#define ORCHBEE_LOCAL_IPC_WINDOWS_H

#include <stddef.h>
#include <stdint.h>
#include <wchar.h>

enum {
    OB_WINDOWS_IPC_OK = 0,
    OB_WINDOWS_IPC_ALREADY_RUNNING = 1,
    OB_WINDOWS_IPC_INVALID_RUNTIME_DIRECTORY = 2,
    OB_WINDOWS_IPC_PEER_IDENTITY_MISMATCH = 3,
    OB_WINDOWS_IPC_CONNECTION_CLOSED = 4,
    OB_WINDOWS_IPC_SYSTEM_ERROR = 5,
    OB_WINDOWS_IPC_WRITE_TIMEOUT = 6,
    OB_WINDOWS_IPC_ACCEPT_TIMEOUT = 7,
};

typedef struct ObWindowsLocalServer ObWindowsLocalServer;

ObWindowsLocalServer *ob_windows_local_server_create(
    const wchar_t *runtime_directory,
    const wchar_t *endpoint_id,
    int *status
);
/* Another instance of `primary`'s pipe for a control connection; it holds no lifetime mutex and
 * may be created and destroyed on any thread. */
ObWindowsLocalServer *ob_windows_local_server_create_instance(
    const ObWindowsLocalServer *primary,
    int *status
);
void ob_windows_local_server_destroy(ObWindowsLocalServer *server);
/* The Win32 code of the last accept or listen failure on `server`, 0 when none was recorded. */
uint32_t ob_windows_local_server_last_error(const ObWindowsLocalServer *server);
/* Replaces `primary`'s pipe instance with a new instance of the same pipe after an accept failure,
 * ending any connection and listen on the old one. The lifetime mutex stays held. Call it only
 * while no control-lane instance exists. */
int ob_windows_local_server_renew(ObWindowsLocalServer *primary);
/* Ends every instance's accept wait, now and until resumed, as a timeout. Each instance waits on
 * an event `primary` owns, so every instance is destroyed before `primary`. */
void ob_windows_local_server_stop_instances(ObWindowsLocalServer *primary);
void ob_windows_local_server_resume_instances(ObWindowsLocalServer *primary);
/* Waits for a client to connect, at most `timeout_ms` for the bounded form; the listen stays armed
 * across a timeout. The connection's first byte is not awaited here: the readiness wait delivers
 * it, after checking the peer's identity, within the caller's own deadline. */
int ob_windows_local_server_accept(ObWindowsLocalServer *server);
int ob_windows_local_server_accept_before(
    ObWindowsLocalServer *server,
    uint32_t timeout_ms
);
int ob_windows_local_server_peek(ObWindowsLocalServer *server, uint32_t *available);
int ob_windows_local_server_wait_readable(
    ObWindowsLocalServer *server,
    uint32_t timeout_ms,
    uint32_t *readable
);
/* Also returns, with `readable` unset, once an optional self-resetting wake object is signalled. */
int ob_windows_local_server_wait_readable_or_wake(
    ObWindowsLocalServer *server,
    uint32_t timeout_ms,
    void *wake_event,
    void *wake_timer,
    uint32_t *readable
);
int ob_windows_local_server_read(
    ObWindowsLocalServer *server,
    uint8_t *buffer,
    uint32_t capacity,
    uint32_t *read_bytes
);
int ob_windows_local_server_write(
    ObWindowsLocalServer *server,
    const uint8_t *buffer,
    uint32_t length,
    uint32_t *written_bytes
);
int ob_windows_local_server_write_before(
    ObWindowsLocalServer *server,
    const uint8_t *buffer,
    uint32_t length,
    uint32_t timeout_ms,
    uint32_t *written_bytes
);
/* Starts a write of `buffer`, or continues the one already pending, and waits at most
 * `timeout_ms` for it. A write still pending returns OK with `written_bytes` 0; the caller then
 * passes the same buffer again, or cancels, before releasing it. */
int ob_windows_local_server_write_wait(
    ObWindowsLocalServer *server,
    const uint8_t *buffer,
    uint32_t length,
    uint32_t timeout_ms,
    uint32_t *written_bytes
);
/* Cancels a pending write and waits until the kernel has released its buffer. */
void ob_windows_local_server_cancel_write(ObWindowsLocalServer *server);
void ob_windows_local_server_disconnect(ObWindowsLocalServer *server);

void *ob_windows_local_test_client_connect(const wchar_t *endpoint_id, int *status);
int ob_windows_local_test_client_read(
    void *client,
    uint8_t *buffer,
    uint32_t capacity,
    uint32_t *read_bytes
);
int ob_windows_local_test_client_peek(void *client, uint32_t *available);
int ob_windows_local_test_client_write(
    void *client,
    const uint8_t *buffer,
    uint32_t length,
    uint32_t *written_bytes
);
void ob_windows_local_test_client_close(void *client);

int ob_windows_local_test_make_directory_private(const wchar_t *path);
/* 1 when the private rule admits the descriptor's owner and DACL for `user_sid`, 0 when it
 * refuses them, -1 when either string does not parse. */
int ob_windows_local_test_private_descriptor(
    const wchar_t *descriptor_sddl,
    const wchar_t *user_sid
);
/* 1 when the descriptor a Host at integrity `level` gives its pipe and mutex carries the High
 * no-read-up, no-write-up label, 0 when it carries no label, -1 when it cannot be built. */
int ob_windows_local_test_endpoint_label(uint32_t level);

#endif
