#define _GNU_SOURCE

#include "private_ipc_security.h"

#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <sys/file.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

#if defined(__APPLE__)
#include <sys/acl.h>
#endif

#if defined(__linux__)
#include <sys/xattr.h>
#endif

_Static_assert(sizeof(uid_t) <= sizeof(uint32_t), "uid_t must fit in the security ABI");

static int acl_absent(void) {
#if defined(__APPLE__)
    return errno == ENOENT;
#elif defined(__linux__)
    return errno == ENODATA || errno == ENOTSUP || errno == EOPNOTSUPP;
#else
#error unsupported SSH Environment Runtime target
#endif
}

static int clear_fd_acl(int descriptor) {
#if defined(__APPLE__)
    acl_t current = acl_get_fd_np(descriptor, ACL_TYPE_EXTENDED);
    if (current == NULL) {
        return acl_absent() ? OB_SSH_RUNTIME_SECURITY_OK :
            OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    }
    acl_free(current);
    acl_t empty = acl_init(0);
    if (empty == NULL) return OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    const int result = acl_set_fd_np(descriptor, empty, ACL_TYPE_EXTENDED);
    acl_free(empty);
    return result == 0 ? OB_SSH_RUNTIME_SECURITY_OK :
        OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
#elif defined(__linux__)
    const char *const names[] = {
        "system.posix_acl_access",
        "system.posix_acl_default",
    };
    for (size_t index = 0; index < 2; index += 1) {
        errno = 0;
        if (fremovexattr(descriptor, names[index]) != 0 && !acl_absent()) {
            return OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
        }
    }
    return OB_SSH_RUNTIME_SECURITY_OK;
#endif
}

static int validate_fd_acl(int descriptor) {
#if defined(__APPLE__)
    acl_t current = acl_get_fd_np(descriptor, ACL_TYPE_EXTENDED);
    if (current == NULL) {
        return acl_absent() ? OB_SSH_RUNTIME_SECURITY_OK :
            OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    }
    acl_free(current);
    return OB_SSH_RUNTIME_SECURITY_INSECURE;
#elif defined(__linux__)
    const char *const names[] = {
        "system.posix_acl_access",
        "system.posix_acl_default",
    };
    for (size_t index = 0; index < 2; index += 1) {
        errno = 0;
        if (fgetxattr(descriptor, names[index], NULL, 0) >= 0) {
            return OB_SSH_RUNTIME_SECURITY_INSECURE;
        }
        if (!acl_absent()) return OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    }
    return OB_SSH_RUNTIME_SECURITY_OK;
#endif
}

static int clear_path_acl(const char *path) {
#if defined(__APPLE__)
    acl_t current = acl_get_link_np(path, ACL_TYPE_EXTENDED);
    if (current == NULL) {
        return acl_absent() ? OB_SSH_RUNTIME_SECURITY_OK :
            OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    }
    acl_free(current);
    acl_t empty = acl_init(0);
    if (empty == NULL) return OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    const int result = acl_set_link_np(path, ACL_TYPE_EXTENDED, empty);
    acl_free(empty);
    return result == 0 ? OB_SSH_RUNTIME_SECURITY_OK :
        OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
#elif defined(__linux__)
    const char *const names[] = {
        "system.posix_acl_access",
        "system.posix_acl_default",
    };
    for (size_t index = 0; index < 2; index += 1) {
        errno = 0;
        if (lremovexattr(path, names[index]) != 0 && !acl_absent()) {
            return OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
        }
    }
    return OB_SSH_RUNTIME_SECURITY_OK;
#endif
}

static int validate_path_acl(const char *path) {
#if defined(__APPLE__)
    acl_t current = acl_get_link_np(path, ACL_TYPE_EXTENDED);
    if (current == NULL) {
        return acl_absent() ? OB_SSH_RUNTIME_SECURITY_OK :
            OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    }
    acl_free(current);
    return OB_SSH_RUNTIME_SECURITY_INSECURE;
#elif defined(__linux__)
    const char *const names[] = {
        "system.posix_acl_access",
        "system.posix_acl_default",
    };
    for (size_t index = 0; index < 2; index += 1) {
        errno = 0;
        if (lgetxattr(path, names[index], NULL, 0) >= 0) {
            return OB_SSH_RUNTIME_SECURITY_INSECURE;
        }
        if (!acl_absent()) return OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    }
    return OB_SSH_RUNTIME_SECURITY_OK;
#endif
}

static int validate_directory_fd(int descriptor, uint32_t expected_uid) {
    struct stat status;
    if (fstat(descriptor, &status) != 0) return OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    if (!S_ISDIR(status.st_mode) || (uint32_t)status.st_uid != expected_uid ||
        (status.st_mode & (S_IRWXU | S_IRWXG | S_IRWXO)) != S_IRWXU) {
        return OB_SSH_RUNTIME_SECURITY_INSECURE;
    }
    return validate_fd_acl(descriptor);
}

static int open_directory(const char *path, uint32_t expected_uid, int secure) {
    const int descriptor = open(path, O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW);
    if (descriptor < 0) return OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    int result = OB_SSH_RUNTIME_SECURITY_OK;
    struct stat status;
    if (fstat(descriptor, &status) != 0) {
        result = OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    } else if (!S_ISDIR(status.st_mode) || (uint32_t)status.st_uid != expected_uid) {
        result = OB_SSH_RUNTIME_SECURITY_INSECURE;
    } else if (secure) {
        result = clear_fd_acl(descriptor);
        if (result == OB_SSH_RUNTIME_SECURITY_OK && fchmod(descriptor, S_IRWXU) != 0) {
            result = OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
        }
    }
    if (result == OB_SSH_RUNTIME_SECURITY_OK) {
        result = validate_directory_fd(descriptor, expected_uid);
    }
    close(descriptor);
    return result;
}

int ob_ssh_runtime_prepare_directory(const char *path, uint32_t expected_uid) {
    if (mkdir(path, S_IRWXU) != 0 && errno != EEXIST) {
        return OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    }
    return open_directory(path, expected_uid, 1);
}

int ob_ssh_runtime_validate_directory(const char *path, uint32_t expected_uid) {
    return open_directory(path, expected_uid, 0);
}

int ob_ssh_runtime_acquire_lock(
    const char *directory_path,
    const char *lock_name,
    uint32_t expected_uid,
    int *lock_fd) {
    if (lock_fd == NULL) return OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    *lock_fd = -1;
    const int directory = open(
        directory_path,
        O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW);
    if (directory < 0) return OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    int result = validate_directory_fd(directory, expected_uid);
    if (result != OB_SSH_RUNTIME_SECURITY_OK) {
        close(directory);
        return result;
    }
    const int descriptor = openat(
        directory,
        lock_name,
        O_RDWR | O_CREAT | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK,
        S_IRUSR | S_IWUSR);
    close(directory);
    if (descriptor < 0) return OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;

    struct stat status;
    if (fstat(descriptor, &status) != 0 || !S_ISREG(status.st_mode) ||
        (uint32_t)status.st_uid != expected_uid) {
        close(descriptor);
        return OB_SSH_RUNTIME_SECURITY_INSECURE;
    }
    result = clear_fd_acl(descriptor);
    if (result == OB_SSH_RUNTIME_SECURITY_OK &&
        fchmod(descriptor, S_IRUSR | S_IWUSR) != 0) {
        result = OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    }
    if (result == OB_SSH_RUNTIME_SECURITY_OK) result = validate_fd_acl(descriptor);
    if (result == OB_SSH_RUNTIME_SECURITY_OK && flock(descriptor, LOCK_EX | LOCK_NB) != 0) {
        result = (errno == EWOULDBLOCK) ? OB_SSH_RUNTIME_SECURITY_ALREADY_RUNNING :
            OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    }
    if (result != OB_SSH_RUNTIME_SECURITY_OK) {
        close(descriptor);
        return result;
    }
    *lock_fd = descriptor;
    return OB_SSH_RUNTIME_SECURITY_OK;
}

static int endpoint_status(const char *path, uint32_t expected_uid) {
    struct stat status;
    if (lstat(path, &status) != 0) {
        return errno == ENOENT ? OB_SSH_RUNTIME_SECURITY_ENDPOINT_UNAVAILABLE :
            OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    }
    if (!S_ISSOCK(status.st_mode) || (uint32_t)status.st_uid != expected_uid ||
        (status.st_mode & (S_IRWXU | S_IRWXG | S_IRWXO)) !=
            (S_IRUSR | S_IWUSR)) {
        return OB_SSH_RUNTIME_SECURITY_INSECURE;
    }
    return validate_path_acl(path);
}

int ob_ssh_runtime_secure_endpoint(const char *path, uint32_t expected_uid) {
    struct stat status;
    if (lstat(path, &status) != 0) return OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    if (!S_ISSOCK(status.st_mode) || (uint32_t)status.st_uid != expected_uid) {
        return OB_SSH_RUNTIME_SECURITY_INSECURE;
    }
    int result = clear_path_acl(path);
    if (result == OB_SSH_RUNTIME_SECURITY_OK && chmod(path, S_IRUSR | S_IWUSR) != 0) {
        result = OB_SSH_RUNTIME_SECURITY_SYSTEM_ERROR;
    }
    if (result != OB_SSH_RUNTIME_SECURITY_OK) return result;
    return endpoint_status(path, expected_uid);
}

int ob_ssh_runtime_validate_endpoint(const char *path, uint32_t expected_uid) {
    return endpoint_status(path, expected_uid);
}

int ob_ssh_runtime_require_peer_uid(int socket_fd, uint32_t expected_uid) {
#if defined(__APPLE__)
    uid_t effective_uid = 0;
    gid_t effective_gid = 0;
    if (getpeereid(socket_fd, &effective_uid, &effective_gid) != 0) {
        return OB_SSH_RUNTIME_SECURITY_PEER_UNAVAILABLE;
    }
    (void)effective_gid;
#elif defined(__linux__)
    struct ucred credentials;
    socklen_t length = sizeof(credentials);
    if (getsockopt(socket_fd, SOL_SOCKET, SO_PEERCRED, &credentials, &length) != 0 ||
        length != sizeof(credentials)) {
        return OB_SSH_RUNTIME_SECURITY_PEER_UNAVAILABLE;
    }
    const uid_t effective_uid = credentials.uid;
#endif
    return (uint32_t)effective_uid == expected_uid ? OB_SSH_RUNTIME_SECURITY_OK :
        OB_SSH_RUNTIME_SECURITY_PEER_MISMATCH;
}
