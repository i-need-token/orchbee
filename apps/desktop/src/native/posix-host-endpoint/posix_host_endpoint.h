#ifndef ORCHBEE_DESKTOP_POSIX_HOST_ENDPOINT_H
#define ORCHBEE_DESKTOP_POSIX_HOST_ENDPOINT_H

#include <stdint.h>

enum ob_desktop_posix_endpoint_status {
    OB_DESKTOP_POSIX_ENDPOINT_OK = 0,
    OB_DESKTOP_POSIX_ENDPOINT_INVALID_ARGUMENT = 1,
    OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR = 2,
    OB_DESKTOP_POSIX_ENDPOINT_INSECURE = 3,
    OB_DESKTOP_POSIX_ENDPOINT_UNAVAILABLE = 4,
    OB_DESKTOP_POSIX_ENDPOINT_PEER_IDENTITY_MISMATCH = 5,
    OB_DESKTOP_POSIX_ENDPOINT_PEER_IDENTITY_UNAVAILABLE = 6,
};

int ob_desktop_prepare_runtime_directory(
    const char *runtime_directory,
    uint32_t expected_uid);
int ob_desktop_connect_authenticated(
    const char *runtime_directory,
    const char *endpoint_id,
    uint32_t expected_uid,
    int *connected_fd);
int ob_desktop_require_peer_effective_uid(int socket_fd, uint32_t expected_uid);

#endif
