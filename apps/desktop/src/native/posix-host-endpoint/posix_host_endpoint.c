#define _GNU_SOURCE

#include "posix_host_endpoint.h"

#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/un.h>
#include <unistd.h>

#if defined(__APPLE__)
#include <sys/acl.h>
#elif defined(__linux__)
#include <sys/xattr.h>
#else
#error unsupported Desktop POSIX Host endpoint target
#endif

_Static_assert(sizeof(uid_t) <= sizeof(uint32_t), "uid_t must fit in the adapter ABI");

static int runtime_directory_stat_is_private(
    const struct stat *status,
    uint32_t expected_uid) {
    return S_ISDIR(status->st_mode) &&
        (uint32_t)status->st_uid == expected_uid &&
        (status->st_mode & (S_IRWXU | S_IRWXG | S_IRWXO)) == S_IRWXU;
}

static int endpoint_stat_is_private(
    const struct stat *status,
    uint32_t expected_uid) {
    return S_ISSOCK(status->st_mode) &&
        (uint32_t)status->st_uid == expected_uid &&
        (status->st_mode & (S_IRWXU | S_IRWXG | S_IRWXO)) ==
            (S_IRUSR | S_IWUSR);
}

#if defined(__APPLE__)
static int darwin_acl_missing(void) {
    return errno == ENOENT;
}

static int clear_descriptor_acl(int descriptor) {
    acl_t current = acl_get_fd_np(descriptor, ACL_TYPE_EXTENDED);
    if (current == NULL) {
        if (darwin_acl_missing()) return OB_DESKTOP_POSIX_ENDPOINT_OK;
        return OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
    }
    acl_free(current);

    acl_t empty = acl_init(0);
    if (empty == NULL) return OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
    const int set_result = acl_set_fd_np(descriptor, empty, ACL_TYPE_EXTENDED);
    acl_free(empty);
    return set_result == 0 ? OB_DESKTOP_POSIX_ENDPOINT_OK :
        OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
}

static int validate_descriptor_acl(int descriptor) {
    acl_t current = acl_get_fd_np(descriptor, ACL_TYPE_EXTENDED);
    if (current == NULL) {
        return darwin_acl_missing() ? OB_DESKTOP_POSIX_ENDPOINT_OK :
            OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
    }
    acl_free(current);
    return OB_DESKTOP_POSIX_ENDPOINT_INSECURE;
}

static int validate_endpoint_acl(const char *path) {
    acl_t current = acl_get_link_np(path, ACL_TYPE_EXTENDED);
    if (current == NULL) {
        return darwin_acl_missing() ? OB_DESKTOP_POSIX_ENDPOINT_OK :
            OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
    }
    acl_free(current);
    return OB_DESKTOP_POSIX_ENDPOINT_INSECURE;
}
#elif defined(__linux__)
static const char *const linux_access_acl = "system.posix_acl_access";
static const char *const linux_default_acl = "system.posix_acl_default";

static int linux_acl_absent(void) {
    return errno == ENODATA || errno == ENOTSUP || errno == EOPNOTSUPP;
}

static int remove_linux_acl_fd(int descriptor, const char *name) {
    if (fremovexattr(descriptor, name) == 0 || linux_acl_absent()) {
        return OB_DESKTOP_POSIX_ENDPOINT_OK;
    }
    return OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
}

static int validate_linux_acl_fd(int descriptor, const char *name) {
    errno = 0;
    if (fgetxattr(descriptor, name, NULL, 0) >= 0) {
        return OB_DESKTOP_POSIX_ENDPOINT_INSECURE;
    }
    return linux_acl_absent() ? OB_DESKTOP_POSIX_ENDPOINT_OK :
        OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
}

static int validate_linux_acl_link(const char *path, const char *name) {
    errno = 0;
    if (lgetxattr(path, name, NULL, 0) >= 0) {
        return OB_DESKTOP_POSIX_ENDPOINT_INSECURE;
    }
    return linux_acl_absent() ? OB_DESKTOP_POSIX_ENDPOINT_OK :
        OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
}

static int clear_descriptor_acl(int descriptor) {
    int result = remove_linux_acl_fd(descriptor, linux_access_acl);
    if (result != OB_DESKTOP_POSIX_ENDPOINT_OK) return result;
    return remove_linux_acl_fd(descriptor, linux_default_acl);
}

static int validate_descriptor_acl(int descriptor) {
    int result = validate_linux_acl_fd(descriptor, linux_access_acl);
    if (result != OB_DESKTOP_POSIX_ENDPOINT_OK) return result;
    return validate_linux_acl_fd(descriptor, linux_default_acl);
}

static int validate_endpoint_acl(const char *path) {
    int result = validate_linux_acl_link(path, linux_access_acl);
    if (result != OB_DESKTOP_POSIX_ENDPOINT_OK) return result;
    return validate_linux_acl_link(path, linux_default_acl);
}
#endif

static int validate_runtime_directory_descriptor(
    int directory_fd,
    uint32_t expected_uid) {
    struct stat status;
    if (fstat(directory_fd, &status) != 0) {
        return OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
    }
    if (!runtime_directory_stat_is_private(&status, expected_uid)) {
        return OB_DESKTOP_POSIX_ENDPOINT_INSECURE;
    }
    return validate_descriptor_acl(directory_fd);
}

static int open_runtime_directory(
    const char *runtime_directory,
    uint32_t expected_uid,
    int *directory_fd) {
    if (runtime_directory == NULL || runtime_directory[0] != '/' || directory_fd == NULL) {
        return OB_DESKTOP_POSIX_ENDPOINT_INVALID_ARGUMENT;
    }
    *directory_fd = -1;
    const int descriptor = open(
        runtime_directory,
        O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (descriptor < 0) {
        switch (errno) {
            case EACCES:
            case ELOOP:
            case ENOTDIR:
                return OB_DESKTOP_POSIX_ENDPOINT_INSECURE;
            case ENOENT:
                return OB_DESKTOP_POSIX_ENDPOINT_UNAVAILABLE;
            default:
                return OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
        }
    }
    const int result = validate_runtime_directory_descriptor(descriptor, expected_uid);
    if (result != OB_DESKTOP_POSIX_ENDPOINT_OK) {
        close(descriptor);
        return result;
    }
    *directory_fd = descriptor;
    return OB_DESKTOP_POSIX_ENDPOINT_OK;
}

int ob_desktop_prepare_runtime_directory(
    const char *runtime_directory,
    uint32_t expected_uid) {
    if (runtime_directory == NULL || runtime_directory[0] != '/') {
        return OB_DESKTOP_POSIX_ENDPOINT_INVALID_ARGUMENT;
    }
    if (mkdir(runtime_directory, S_IRWXU) != 0 && errno != EEXIST) {
        return OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
    }

    const int descriptor = open(
        runtime_directory,
        O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (descriptor < 0) {
        return errno == EACCES || errno == ELOOP || errno == ENOTDIR ?
            OB_DESKTOP_POSIX_ENDPOINT_INSECURE : OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
    }

    struct stat status;
    int result = OB_DESKTOP_POSIX_ENDPOINT_OK;
    if (fstat(descriptor, &status) != 0) {
        result = OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
    } else if (!S_ISDIR(status.st_mode) ||
               (uint32_t)status.st_uid != expected_uid ||
               (status.st_mode & (S_IRWXG | S_IRWXO)) != 0) {
        result = OB_DESKTOP_POSIX_ENDPOINT_INSECURE;
    }
    if (result == OB_DESKTOP_POSIX_ENDPOINT_OK) {
        result = clear_descriptor_acl(descriptor);
    }
    if (result == OB_DESKTOP_POSIX_ENDPOINT_OK && fchmod(descriptor, S_IRWXU) != 0) {
        result = OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
    }
    if (result == OB_DESKTOP_POSIX_ENDPOINT_OK) {
        result = validate_runtime_directory_descriptor(descriptor, expected_uid);
    }
    close(descriptor);
    return result;
}

static int endpoint_id_is_valid(const char *endpoint_id) {
    if (endpoint_id == NULL || strlen(endpoint_id) != 32) return 0;
    for (size_t index = 0; index < 32; index += 1) {
        const char byte = endpoint_id[index];
        if (!((byte >= '0' && byte <= '9') || (byte >= 'a' && byte <= 'f'))) return 0;
    }
    return 1;
}

static int build_endpoint_paths(
    const char *runtime_directory,
    const char *endpoint_id,
    char *endpoint_name,
    size_t endpoint_name_capacity,
    char *endpoint_path,
    size_t endpoint_path_capacity) {
    if (!endpoint_id_is_valid(endpoint_id)) {
        return OB_DESKTOP_POSIX_ENDPOINT_INVALID_ARGUMENT;
    }
    const int name_length = snprintf(
        endpoint_name,
        endpoint_name_capacity,
        "%s.s",
        endpoint_id);
    if (name_length < 0 || (size_t)name_length >= endpoint_name_capacity) {
        return OB_DESKTOP_POSIX_ENDPOINT_INVALID_ARGUMENT;
    }
    const size_t runtime_length = strlen(runtime_directory);
    const int path_length = snprintf(
        endpoint_path,
        endpoint_path_capacity,
        runtime_length > 0 && runtime_directory[runtime_length - 1] == '/' ? "%s%s" : "%s/%s",
        runtime_directory,
        endpoint_name);
    if (path_length < 0 || (size_t)path_length >= endpoint_path_capacity) {
        return OB_DESKTOP_POSIX_ENDPOINT_INVALID_ARGUMENT;
    }
    return OB_DESKTOP_POSIX_ENDPOINT_OK;
}

static int validate_endpoint(
    int directory_fd,
    const char *endpoint_name,
    const char *endpoint_path,
    uint32_t expected_uid) {
    struct stat status;
    if (fstatat(directory_fd, endpoint_name, &status, AT_SYMLINK_NOFOLLOW) != 0) {
        return errno == ENOENT ? OB_DESKTOP_POSIX_ENDPOINT_UNAVAILABLE :
            OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
    }
    if (!endpoint_stat_is_private(&status, expected_uid)) {
        return OB_DESKTOP_POSIX_ENDPOINT_INSECURE;
    }
    return validate_endpoint_acl(endpoint_path);
}

int ob_desktop_require_peer_effective_uid(int socket_fd, uint32_t expected_uid) {
    uid_t peer_uid;
#if defined(__linux__)
    struct ucred credentials;
    socklen_t credentials_length = sizeof(credentials);
    if (getsockopt(
            socket_fd,
            SOL_SOCKET,
            SO_PEERCRED,
            &credentials,
            &credentials_length) != 0 ||
        credentials_length != sizeof(credentials)) {
        return OB_DESKTOP_POSIX_ENDPOINT_PEER_IDENTITY_UNAVAILABLE;
    }
    peer_uid = credentials.uid;
#elif defined(__APPLE__)
    gid_t peer_gid;
    if (getpeereid(socket_fd, &peer_uid, &peer_gid) != 0) {
        return OB_DESKTOP_POSIX_ENDPOINT_PEER_IDENTITY_UNAVAILABLE;
    }
#endif
    return (uint32_t)peer_uid == expected_uid ? OB_DESKTOP_POSIX_ENDPOINT_OK :
        OB_DESKTOP_POSIX_ENDPOINT_PEER_IDENTITY_MISMATCH;
}

static int connection_error_status(void) {
    switch (errno) {
        case ECONNREFUSED:
        case ECONNRESET:
        case ENOENT:
            return OB_DESKTOP_POSIX_ENDPOINT_UNAVAILABLE;
        default:
            return OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
    }
}

int ob_desktop_connect_authenticated(
    const char *runtime_directory,
    const char *endpoint_id,
    uint32_t expected_uid,
    int *connected_fd) {
    if (connected_fd == NULL) return OB_DESKTOP_POSIX_ENDPOINT_INVALID_ARGUMENT;
    *connected_fd = -1;

    int directory_fd = -1;
    int result = open_runtime_directory(runtime_directory, expected_uid, &directory_fd);
    if (result != OB_DESKTOP_POSIX_ENDPOINT_OK) return result;

    char endpoint_name[35];
    struct sockaddr_un address;
    memset(&address, 0, sizeof(address));
    result = build_endpoint_paths(
        runtime_directory,
        endpoint_id,
        endpoint_name,
        sizeof(endpoint_name),
        address.sun_path,
        sizeof(address.sun_path));
    if (result == OB_DESKTOP_POSIX_ENDPOINT_OK) {
        result = validate_endpoint(
            directory_fd,
            endpoint_name,
            address.sun_path,
            expected_uid);
    }
    close(directory_fd);
    if (result != OB_DESKTOP_POSIX_ENDPOINT_OK) return result;

    const int socket_fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (socket_fd < 0) return OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
    const int descriptor_flags = fcntl(socket_fd, F_GETFD);
    if (descriptor_flags < 0 || fcntl(socket_fd, F_SETFD, descriptor_flags | FD_CLOEXEC) != 0) {
        close(socket_fd);
        return OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR;
    }

    address.sun_family = AF_UNIX;
#if defined(__APPLE__)
    address.sun_len = (uint8_t)sizeof(address);
#endif
    if (connect(socket_fd, (const struct sockaddr *)&address, sizeof(address)) != 0) {
        result = connection_error_status();
        close(socket_fd);
        return result;
    }
    result = ob_desktop_require_peer_effective_uid(socket_fd, expected_uid);
    if (result != OB_DESKTOP_POSIX_ENDPOINT_OK) {
        close(socket_fd);
        return result;
    }

    *connected_fd = socket_fd;
    return OB_DESKTOP_POSIX_ENDPOINT_OK;
}
