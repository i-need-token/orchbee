#define _GNU_SOURCE

#include "posix_endpoint_security.h"

#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

#if defined(__APPLE__) || defined(__FreeBSD__)
#include <sys/acl.h>
#endif

#if defined(__linux__)
#include <sys/xattr.h>
#endif

_Static_assert(sizeof(uid_t) <= sizeof(uint32_t), "uid_t must fit in the security ABI");

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

static int lock_stat_is_private(
    const struct stat *status,
    uint32_t expected_uid) {
    return S_ISREG(status->st_mode) &&
        (uint32_t)status->st_uid == expected_uid &&
        (status->st_mode & (S_IRWXU | S_IRWXG | S_IRWXO)) ==
            (S_IRUSR | S_IWUSR);
}

#if defined(__APPLE__)
static int darwin_acl_missing(void) {
    return errno == ENOENT;
}

static int clear_darwin_acl_fd(int descriptor) {
    acl_t current = acl_get_fd_np(descriptor, ACL_TYPE_EXTENDED);
    if (current == NULL) {
        return darwin_acl_missing() ? OB_POSIX_SECURITY_OK :
            OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    acl_free(current);

    acl_t empty = acl_init(0);
    if (empty == NULL) return OB_POSIX_SECURITY_SYSTEM_ERROR;
    const int set_result = acl_set_fd_np(descriptor, empty, ACL_TYPE_EXTENDED);
    acl_free(empty);
    if (set_result != 0) return OB_POSIX_SECURITY_SYSTEM_ERROR;
    return OB_POSIX_SECURITY_OK;
}

static int clear_darwin_acl_link(const char *path) {
    acl_t current = acl_get_link_np(path, ACL_TYPE_EXTENDED);
    if (current == NULL) {
        return darwin_acl_missing() ? OB_POSIX_SECURITY_OK :
            OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    acl_free(current);

    acl_t empty = acl_init(0);
    if (empty == NULL) return OB_POSIX_SECURITY_SYSTEM_ERROR;
    const int set_result = acl_set_link_np(path, ACL_TYPE_EXTENDED, empty);
    acl_free(empty);
    if (set_result != 0) return OB_POSIX_SECURITY_SYSTEM_ERROR;
    return OB_POSIX_SECURITY_OK;
}

static int validate_darwin_acl_fd(int descriptor) {
    acl_t current = acl_get_fd_np(descriptor, ACL_TYPE_EXTENDED);
    if (current == NULL) {
        return darwin_acl_missing() ? OB_POSIX_SECURITY_OK :
            OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    acl_free(current);
    return OB_POSIX_SECURITY_INSECURE;
}

static int validate_darwin_acl_link(const char *path) {
    acl_t current = acl_get_link_np(path, ACL_TYPE_EXTENDED);
    if (current == NULL) {
        return darwin_acl_missing() ? OB_POSIX_SECURITY_OK :
            OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    acl_free(current);
    return OB_POSIX_SECURITY_INSECURE;
}
#endif

#if defined(__linux__)
static const char *const linux_access_acl = "system.posix_acl_access";
static const char *const linux_default_acl = "system.posix_acl_default";

static int linux_acl_absent(void) {
    return errno == ENODATA || errno == ENOTSUP || errno == EOPNOTSUPP;
}

static int remove_linux_acl_fd(int descriptor, const char *name) {
    if (fremovexattr(descriptor, name) == 0 || linux_acl_absent()) {
        return OB_POSIX_SECURITY_OK;
    }
    return OB_POSIX_SECURITY_SYSTEM_ERROR;
}

static int remove_linux_acl_link(const char *path, const char *name) {
    if (lremovexattr(path, name) == 0 || linux_acl_absent()) {
        return OB_POSIX_SECURITY_OK;
    }
    return OB_POSIX_SECURITY_SYSTEM_ERROR;
}

static int validate_linux_acl_fd(int descriptor, const char *name) {
    errno = 0;
    if (fgetxattr(descriptor, name, NULL, 0) >= 0) {
        return OB_POSIX_SECURITY_INSECURE;
    }
    return linux_acl_absent() ? OB_POSIX_SECURITY_OK :
        OB_POSIX_SECURITY_SYSTEM_ERROR;
}

static int validate_linux_acl_link(const char *path, const char *name) {
    errno = 0;
    if (lgetxattr(path, name, NULL, 0) >= 0) {
        return OB_POSIX_SECURITY_INSECURE;
    }
    return linux_acl_absent() ? OB_POSIX_SECURITY_OK :
        OB_POSIX_SECURITY_SYSTEM_ERROR;
}
#endif

#if defined(__FreeBSD__)
static int freebsd_acl_type_unsupported(void) {
    return errno == EINVAL || errno == ENOENT || errno == ENOTSUP ||
        errno == EOPNOTSUPP;
}

static int strip_freebsd_acl_fd(int descriptor, acl_type_t type) {
    errno = 0;
    acl_t current = acl_get_fd_np(descriptor, type);
    if (current == NULL) {
        return freebsd_acl_type_unsupported() ? OB_POSIX_SECURITY_OK :
            OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    int trivial = 0;
    if (acl_is_trivial_np(current, &trivial) != 0) {
        acl_free(current);
        return OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    if (trivial != 0) {
        acl_free(current);
        return OB_POSIX_SECURITY_OK;
    }
    acl_t stripped = acl_strip_np(current, 1);
    acl_free(current);
    if (stripped == NULL) return OB_POSIX_SECURITY_SYSTEM_ERROR;
    const int set_result = acl_set_fd_np(descriptor, stripped, type);
    acl_free(stripped);
    return set_result == 0 ? OB_POSIX_SECURITY_OK : OB_POSIX_SECURITY_SYSTEM_ERROR;
}

static int strip_freebsd_acl_link(const char *path, acl_type_t type) {
    errno = 0;
    acl_t current = acl_get_link_np(path, type);
    if (current == NULL) {
        return freebsd_acl_type_unsupported() ? OB_POSIX_SECURITY_OK :
            OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    int trivial = 0;
    if (acl_is_trivial_np(current, &trivial) != 0) {
        acl_free(current);
        return OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    if (trivial != 0) {
        acl_free(current);
        return OB_POSIX_SECURITY_OK;
    }
    acl_t stripped = acl_strip_np(current, 1);
    acl_free(current);
    if (stripped == NULL) return OB_POSIX_SECURITY_SYSTEM_ERROR;
    const int set_result = acl_set_link_np(path, type, stripped);
    acl_free(stripped);
    return set_result == 0 ? OB_POSIX_SECURITY_OK : OB_POSIX_SECURITY_SYSTEM_ERROR;
}

static int validate_freebsd_acl_fd(int descriptor, acl_type_t type) {
    errno = 0;
    acl_t current = acl_get_fd_np(descriptor, type);
    if (current == NULL) {
        return freebsd_acl_type_unsupported() ? OB_POSIX_SECURITY_OK :
            OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    int trivial = 0;
    const int result = acl_is_trivial_np(current, &trivial);
    acl_free(current);
    if (result != 0) return OB_POSIX_SECURITY_SYSTEM_ERROR;
    return trivial != 0 ? OB_POSIX_SECURITY_OK : OB_POSIX_SECURITY_INSECURE;
}

static int validate_freebsd_acl_link(const char *path, acl_type_t type) {
    errno = 0;
    acl_t current = acl_get_link_np(path, type);
    if (current == NULL) {
        return freebsd_acl_type_unsupported() ? OB_POSIX_SECURITY_OK :
            OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    int trivial = 0;
    const int result = acl_is_trivial_np(current, &trivial);
    acl_free(current);
    if (result != 0) return OB_POSIX_SECURITY_SYSTEM_ERROR;
    return trivial != 0 ? OB_POSIX_SECURITY_OK : OB_POSIX_SECURITY_INSECURE;
}
#endif

static int clear_descriptor_acl(int descriptor) {
#if defined(__APPLE__)
    return clear_darwin_acl_fd(descriptor);
#elif defined(__linux__)
    int result = remove_linux_acl_fd(descriptor, linux_access_acl);
    if (result != OB_POSIX_SECURITY_OK) return result;
    return remove_linux_acl_fd(descriptor, linux_default_acl);
#elif defined(__FreeBSD__)
    if (acl_delete_fd_np(descriptor, ACL_TYPE_DEFAULT) != 0 &&
        !freebsd_acl_type_unsupported()) {
        return OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    int result = strip_freebsd_acl_fd(descriptor, ACL_TYPE_ACCESS);
    if (result != OB_POSIX_SECURITY_OK) return result;
    return strip_freebsd_acl_fd(descriptor, ACL_TYPE_NFS4);
#else
#error unsupported POSIX endpoint-security target
#endif
}

static int validate_descriptor_acl(int descriptor) {
#if defined(__APPLE__)
    return validate_darwin_acl_fd(descriptor);
#elif defined(__linux__)
    int result = validate_linux_acl_fd(descriptor, linux_access_acl);
    if (result != OB_POSIX_SECURITY_OK) return result;
    return validate_linux_acl_fd(descriptor, linux_default_acl);
#elif defined(__FreeBSD__)
    int result = validate_freebsd_acl_fd(descriptor, ACL_TYPE_ACCESS);
    if (result != OB_POSIX_SECURITY_OK) return result;
    result = validate_freebsd_acl_fd(descriptor, ACL_TYPE_NFS4);
    if (result != OB_POSIX_SECURITY_OK) return result;
    return validate_freebsd_acl_fd(descriptor, ACL_TYPE_DEFAULT);
#else
#error unsupported POSIX endpoint-security target
#endif
}

static int clear_endpoint_acl(const char *path) {
#if defined(__APPLE__)
    return clear_darwin_acl_link(path);
#elif defined(__linux__)
    int result = remove_linux_acl_link(path, linux_access_acl);
    if (result != OB_POSIX_SECURITY_OK) return result;
    return remove_linux_acl_link(path, linux_default_acl);
#elif defined(__FreeBSD__)
    if (acl_delete_link_np(path, ACL_TYPE_DEFAULT) != 0 &&
        !freebsd_acl_type_unsupported()) {
        return OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    int result = strip_freebsd_acl_link(path, ACL_TYPE_ACCESS);
    if (result != OB_POSIX_SECURITY_OK) return result;
    return strip_freebsd_acl_link(path, ACL_TYPE_NFS4);
#else
#error unsupported POSIX endpoint-security target
#endif
}

static int validate_endpoint_acl(const char *path) {
#if defined(__APPLE__)
    return validate_darwin_acl_link(path);
#elif defined(__linux__)
    int result = validate_linux_acl_link(path, linux_access_acl);
    if (result != OB_POSIX_SECURITY_OK) return result;
    return validate_linux_acl_link(path, linux_default_acl);
#elif defined(__FreeBSD__)
    int result = validate_freebsd_acl_link(path, ACL_TYPE_ACCESS);
    if (result != OB_POSIX_SECURITY_OK) return result;
    return validate_freebsd_acl_link(path, ACL_TYPE_NFS4);
#else
#error unsupported POSIX endpoint-security target
#endif
}

int ob_posix_validate_runtime_directory(int directory_fd, uint32_t expected_uid) {
    struct stat status;
    if (fstat(directory_fd, &status) != 0) return OB_POSIX_SECURITY_SYSTEM_ERROR;
    if (!runtime_directory_stat_is_private(&status, expected_uid)) {
        return OB_POSIX_SECURITY_INSECURE;
    }
    return validate_descriptor_acl(directory_fd);
}

int ob_posix_secure_runtime_directory(int directory_fd, uint32_t expected_uid) {
    struct stat status;
    if (fstat(directory_fd, &status) != 0) return OB_POSIX_SECURITY_SYSTEM_ERROR;
    if (!S_ISDIR(status.st_mode) || (uint32_t)status.st_uid != expected_uid) {
        return OB_POSIX_SECURITY_INSECURE;
    }
    int result = clear_descriptor_acl(directory_fd);
    if (result != OB_POSIX_SECURITY_OK) return result;
    if (fchmod(directory_fd, S_IRWXU) != 0) return OB_POSIX_SECURITY_SYSTEM_ERROR;
    return ob_posix_validate_runtime_directory(directory_fd, expected_uid);
}

static int validate_lock_descriptor(int lock_fd, uint32_t expected_uid) {
    struct stat status;
    if (fstat(lock_fd, &status) != 0) return OB_POSIX_SECURITY_SYSTEM_ERROR;
    if (!lock_stat_is_private(&status, expected_uid)) {
        return OB_POSIX_SECURITY_INSECURE;
    }
    return validate_descriptor_acl(lock_fd);
}

static int lock_open_error_status(void) {
    switch (errno) {
        case EACCES:
        case EISDIR:
        case ELOOP:
        case ENODEV:
        case ENXIO:
            return OB_POSIX_SECURITY_INSECURE;
        default:
            return OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
}

int ob_posix_open_lock_at(
    int directory_fd,
    const char *lock_name,
    uint32_t expected_uid,
    int *lock_fd) {
    if (lock_fd == NULL) return OB_POSIX_SECURITY_SYSTEM_ERROR;
    *lock_fd = -1;

    /* Read-write, because Linux NFS refuses an exclusive lock on a read-only descriptor. */
    const int base_flags = O_RDWR | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK;
    int created = 1;
    int descriptor = openat(
        directory_fd,
        lock_name,
        base_flags | O_CREAT | O_EXCL,
        S_IRUSR | S_IWUSR);
    if (descriptor < 0) {
        if (errno != EEXIST) return lock_open_error_status();
        created = 0;
        descriptor = openat(directory_fd, lock_name, base_flags);
        if (descriptor < 0) return lock_open_error_status();
    }

    int result = OB_POSIX_SECURITY_OK;
    if (created != 0) {
        struct stat status;
        if (fstat(descriptor, &status) != 0) {
            result = OB_POSIX_SECURITY_SYSTEM_ERROR;
        } else if (!S_ISREG(status.st_mode) ||
                   (uint32_t)status.st_uid != expected_uid) {
            result = OB_POSIX_SECURITY_INSECURE;
        } else {
            result = clear_descriptor_acl(descriptor);
            if (result == OB_POSIX_SECURITY_OK &&
                fchmod(descriptor, S_IRUSR | S_IWUSR) != 0) {
                result = OB_POSIX_SECURITY_SYSTEM_ERROR;
            }
        }
    }
    if (result == OB_POSIX_SECURITY_OK) {
        result = validate_lock_descriptor(descriptor, expected_uid);
    }
    if (result != OB_POSIX_SECURITY_OK) {
        close(descriptor);
        return result;
    }

    *lock_fd = descriptor;
    return OB_POSIX_SECURITY_OK;
}

int ob_posix_validate_endpoint_at(
    int directory_fd,
    const char *endpoint_name,
    uint32_t expected_uid) {
    struct stat status;
    if (fstatat(directory_fd, endpoint_name, &status, AT_SYMLINK_NOFOLLOW) != 0) {
        return errno == ENOENT ? OB_POSIX_SECURITY_ENDPOINT_UNAVAILABLE :
            OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    if (!endpoint_stat_is_private(&status, expected_uid)) {
        return OB_POSIX_SECURITY_INSECURE;
    }
    return validate_endpoint_acl(endpoint_name);
}

int ob_posix_secure_endpoint_at(
    int directory_fd,
    const char *endpoint_name,
    uint32_t expected_uid) {
    struct stat status;
    if (fstatat(directory_fd, endpoint_name, &status, AT_SYMLINK_NOFOLLOW) != 0) {
        return errno == ENOENT ? OB_POSIX_SECURITY_ENDPOINT_UNAVAILABLE :
            OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    if (!S_ISSOCK(status.st_mode) || (uint32_t)status.st_uid != expected_uid) {
        return OB_POSIX_SECURITY_INSECURE;
    }
    int result = clear_endpoint_acl(endpoint_name);
    if (result != OB_POSIX_SECURITY_OK) return result;
    if (fchmodat(directory_fd, endpoint_name, S_IRUSR | S_IWUSR, 0) != 0) {
        return OB_POSIX_SECURITY_SYSTEM_ERROR;
    }
    return ob_posix_validate_endpoint_at(directory_fd, endpoint_name, expected_uid);
}

int ob_posix_require_peer_effective_uid(int socket_fd, uint32_t expected_uid) {
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
        return OB_POSIX_SECURITY_PEER_IDENTITY_UNAVAILABLE;
    }
    peer_uid = credentials.uid;
#elif defined(__APPLE__) || defined(__FreeBSD__)
    gid_t peer_gid;
    if (getpeereid(socket_fd, &peer_uid, &peer_gid) != 0) {
        return OB_POSIX_SECURITY_PEER_IDENTITY_UNAVAILABLE;
    }
#else
#error unsupported POSIX peer-identity target
#endif
    return (uint32_t)peer_uid == expected_uid ? OB_POSIX_SECURITY_OK :
        OB_POSIX_SECURITY_PEER_IDENTITY_MISMATCH;
}
