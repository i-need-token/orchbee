#ifndef ORCHBEE_SSH_RUNTIME_PRIVATE_IPC_SECURITY_H
#define ORCHBEE_SSH_RUNTIME_PRIVATE_IPC_SECURITY_H

#include <stdint.h>

enum ob_ssh_runtime_security_status {
    OB_SSH_RUNTIME_SECURITY_OK = 0,
    OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR = 1,
    OB_SSH_RUNTIME_SECURITY_INSECURE = 2,
    OB_SSH_RUNTIME_SECURITY_ALREADY_RUNNING = 3,
    OB_SSH_RUNTIME_SECURITY_PEER_MISMATCH = 4,
    OB_SSH_RUNTIME_SECURITY_PEER_UNAVAILABLE = 5,
    OB_SSH_RUNTIME_SECURITY_ENDPOINT_UNAVAILABLE = 6,
};

int ob_ssh_runtime_prepare_directory(const char *path, uint32_t expected_uid);
int ob_ssh_runtime_validate_directory(const char *path, uint32_t expected_uid);
int ob_ssh_runtime_acquire_lock(
    const char *directory_path,
    const char *lock_name,
    uint32_t expected_uid,
    int *lock_fd);
int ob_ssh_runtime_secure_endpoint(const char *path, uint32_t expected_uid);
int ob_ssh_runtime_validate_endpoint(const char *path, uint32_t expected_uid);
int ob_ssh_runtime_require_peer_uid(int socket_fd, uint32_t expected_uid);

#endif
