#ifndef ORCHBEE_POSIX_ENDPOINT_SECURITY_H
#define ORCHBEE_POSIX_ENDPOINT_SECURITY_H

#include <stdint.h>

enum ob_posix_security_status {
    OB_POSIX_SECURITY_OK = 0,
    OB_POSIX_SECURITY_SYSTEM_ERROR = 1,
    OB_POSIX_SECURITY_INSECURE = 2,
    OB_POSIX_SECURITY_PEER_IDENTITY_MISMATCH = 3,
    OB_POSIX_SECURITY_PEER_IDENTITY_UNAVAILABLE = 4,
    OB_POSIX_SECURITY_ENDPOINT_UNAVAILABLE = 5,
};

int ob_posix_secure_runtime_directory(int directory_fd, uint32_t expected_uid);
int ob_posix_validate_runtime_directory(int directory_fd, uint32_t expected_uid);
int ob_posix_open_lock_at(
    int directory_fd,
    const char *lock_name,
    uint32_t expected_uid,
    int *lock_fd);
int ob_posix_secure_endpoint_at(
    int directory_fd,
    const char *endpoint_name,
    uint32_t expected_uid);
int ob_posix_validate_endpoint_at(
    int directory_fd,
    const char *endpoint_name,
    uint32_t expected_uid);
int ob_posix_require_peer_effective_uid(int socket_fd, uint32_t expected_uid);

#endif
