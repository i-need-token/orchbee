#define _GNU_SOURCE

#include "semantic_command_endpoint.h"

#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
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
#error unsupported semantic command POSIX endpoint target
#endif

#define OB_ENDPOINT_ID_LENGTH 32
#define OB_ENDPOINT_NAME_CAPACITY 35
#define OB_LOCATOR_NAME_CAPACITY 40
#define OB_POLL_SLICE_MILLISECONDS 100

struct ObSemanticCommandServer {
    int client_fd;
    int listener_fd;
    dev_t endpoint_device;
    ino_t endpoint_inode;
    char endpoint_path[sizeof(((struct sockaddr_un *)0)->sun_path)];
};

static int current_effective_uid(uint32_t *uid) {
    if (uid == NULL) return 0;
    const uid_t current = geteuid();
    if ((uintmax_t)current > UINT32_MAX) return 0;
    *uid = (uint32_t)current;
    return 1;
}

static int endpoint_id_is_valid(const char *endpoint_id) {
    if (endpoint_id == NULL || strlen(endpoint_id) != OB_ENDPOINT_ID_LENGTH) return 0;
    for (size_t index = 0; index < OB_ENDPOINT_ID_LENGTH; index += 1) {
        const char byte = endpoint_id[index];
        if (!((byte >= '0' && byte <= '9') || (byte >= 'a' && byte <= 'f'))) return 0;
    }
    return 1;
}

static int build_names(
    const char *runtime_directory,
    const char *endpoint_id,
    char *endpoint_name,
    size_t endpoint_name_capacity,
    char *endpoint_path,
    size_t endpoint_path_capacity,
    char *locator_name,
    size_t locator_name_capacity) {
    if (runtime_directory == NULL || runtime_directory[0] != '/' ||
        !endpoint_id_is_valid(endpoint_id)) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    const int endpoint_name_length = snprintf(
        endpoint_name,
        endpoint_name_capacity,
        "%s.s",
        endpoint_id);
    const int locator_name_length = snprintf(
        locator_name,
        locator_name_capacity,
        "%s.c.json",
        endpoint_id);
    const size_t runtime_length = strlen(runtime_directory);
    const int endpoint_path_length = snprintf(
        endpoint_path,
        endpoint_path_capacity,
        runtime_length > 0 && runtime_directory[runtime_length - 1] == '/' ? "%s%s" : "%s/%s",
        runtime_directory,
        endpoint_name);
    return endpoint_name_length > 0 &&
            (size_t)endpoint_name_length < endpoint_name_capacity &&
            locator_name_length > 0 &&
            (size_t)locator_name_length < locator_name_capacity &&
            endpoint_path_length > 0 &&
            (size_t)endpoint_path_length < endpoint_path_capacity
        ? OB_SEMANTIC_ENDPOINT_OK
        : OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
}

#if defined(__APPLE__)
static int missing_acl(void) {
    return errno == ENOENT;
}

static int clear_directory_acl(int descriptor) {
    acl_t current = acl_get_fd_np(descriptor, ACL_TYPE_EXTENDED);
    if (current == NULL) {
        if (missing_acl()) return OB_SEMANTIC_ENDPOINT_OK;
        return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    acl_free(current);
    acl_t empty = acl_init(0);
    if (empty == NULL) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    const int result = acl_set_fd_np(descriptor, empty, ACL_TYPE_EXTENDED);
    acl_free(empty);
    return result == 0 ? OB_SEMANTIC_ENDPOINT_OK : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
}

static int validate_descriptor_acl(int descriptor) {
    acl_t current = acl_get_fd_np(descriptor, ACL_TYPE_EXTENDED);
    if (current == NULL) {
        return missing_acl() ? OB_SEMANTIC_ENDPOINT_OK : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    acl_free(current);
    return OB_SEMANTIC_ENDPOINT_INSECURE;
}

static int validate_path_acl(const char *path) {
    acl_t current = acl_get_link_np(path, ACL_TYPE_EXTENDED);
    if (current == NULL) {
        return missing_acl() ? OB_SEMANTIC_ENDPOINT_OK : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    acl_free(current);
    return OB_SEMANTIC_ENDPOINT_INSECURE;
}
#elif defined(__linux__)
static int missing_acl(void) {
    return errno == ENODATA || errno == ENOTSUP || errno == EOPNOTSUPP;
}

static int remove_acl(int descriptor, const char *name) {
    if (fremovexattr(descriptor, name) == 0 || missing_acl()) return OB_SEMANTIC_ENDPOINT_OK;
    return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
}

static int clear_directory_acl(int descriptor) {
    int result = remove_acl(descriptor, "system.posix_acl_access");
    if (result != OB_SEMANTIC_ENDPOINT_OK) return result;
    return remove_acl(descriptor, "system.posix_acl_default");
}

static int validate_descriptor_acl(int descriptor) {
    errno = 0;
    if (fgetxattr(descriptor, "system.posix_acl_access", NULL, 0) >= 0) {
        return OB_SEMANTIC_ENDPOINT_INSECURE;
    }
    return missing_acl() ? OB_SEMANTIC_ENDPOINT_OK : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
}

static int validate_path_acl(const char *path) {
    errno = 0;
    if (lgetxattr(path, "system.posix_acl_access", NULL, 0) >= 0) {
        return OB_SEMANTIC_ENDPOINT_INSECURE;
    }
    return missing_acl() ? OB_SEMANTIC_ENDPOINT_OK : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
}
#endif

static int validate_runtime_directory_descriptor(int descriptor, uint32_t uid) {
    struct stat status;
    if (fstat(descriptor, &status) != 0) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    if (!S_ISDIR(status.st_mode) || (uint32_t)status.st_uid != uid ||
        (status.st_mode & (S_IRWXU | S_IRWXG | S_IRWXO)) != S_IRWXU) {
        return OB_SEMANTIC_ENDPOINT_INSECURE;
    }
    return validate_descriptor_acl(descriptor);
}

static int open_runtime_directory(const char *path, uint32_t uid, int *directory_fd) {
    if (path == NULL || path[0] != '/' || directory_fd == NULL) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    *directory_fd = -1;
    const int descriptor = open(path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (descriptor < 0) {
        if (errno == ENOENT) return OB_SEMANTIC_ENDPOINT_UNAVAILABLE;
        if (errno == EACCES || errno == ELOOP || errno == ENOTDIR) {
            return OB_SEMANTIC_ENDPOINT_INSECURE;
        }
        return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    const int result = validate_runtime_directory_descriptor(descriptor, uid);
    if (result != OB_SEMANTIC_ENDPOINT_OK) {
        close(descriptor);
        return result;
    }
    *directory_fd = descriptor;
    return OB_SEMANTIC_ENDPOINT_OK;
}

int ob_semantic_endpoint_prepare_runtime_directory(const char *runtime_directory) {
    uint32_t uid;
    if (runtime_directory == NULL || runtime_directory[0] != '/' ||
        !current_effective_uid(&uid)) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    if (mkdir(runtime_directory, S_IRWXU) != 0 && errno != EEXIST) {
        return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    const int descriptor = open(
        runtime_directory,
        O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (descriptor < 0) {
        return errno == EACCES || errno == ELOOP || errno == ENOTDIR
            ? OB_SEMANTIC_ENDPOINT_INSECURE
            : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    struct stat status;
    int result = OB_SEMANTIC_ENDPOINT_OK;
    if (fstat(descriptor, &status) != 0) {
        result = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    } else if (!S_ISDIR(status.st_mode) || (uint32_t)status.st_uid != uid ||
               (status.st_mode & (S_IRWXG | S_IRWXO)) != 0) {
        result = OB_SEMANTIC_ENDPOINT_INSECURE;
    }
    if (result == OB_SEMANTIC_ENDPOINT_OK) result = clear_directory_acl(descriptor);
    if (result == OB_SEMANTIC_ENDPOINT_OK && fchmod(descriptor, S_IRWXU) != 0) {
        result = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    if (result == OB_SEMANTIC_ENDPOINT_OK) {
        result = validate_runtime_directory_descriptor(descriptor, uid);
    }
    close(descriptor);
    return result;
}

static int require_peer_uid(int socket_fd, uint32_t expected_uid) {
    uid_t peer_uid;
#if defined(__linux__)
    struct ucred credentials;
    socklen_t length = sizeof(credentials);
    if (getsockopt(socket_fd, SOL_SOCKET, SO_PEERCRED, &credentials, &length) != 0 ||
        length != sizeof(credentials)) {
        return OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_UNAVAILABLE;
    }
    peer_uid = credentials.uid;
#elif defined(__APPLE__)
    gid_t peer_gid;
    if (getpeereid(socket_fd, &peer_uid, &peer_gid) != 0) {
        return OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_UNAVAILABLE;
    }
#endif
    return (uint32_t)peer_uid == expected_uid ? OB_SEMANTIC_ENDPOINT_OK
                                               : OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_MISMATCH;
}

static int set_close_on_exec(int descriptor) {
    const int flags = fcntl(descriptor, F_GETFD);
    return flags >= 0 && fcntl(descriptor, F_SETFD, flags | FD_CLOEXEC) == 0;
}

ObSemanticCommandServer *ob_semantic_endpoint_server_create(
    const char *runtime_directory,
    const char *endpoint_id,
    int *status) {
    if (status == NULL) return NULL;
    *status = ob_semantic_endpoint_prepare_runtime_directory(runtime_directory);
    if (*status != OB_SEMANTIC_ENDPOINT_OK) return NULL;
    uint32_t uid;
    if (!current_effective_uid(&uid)) {
        *status = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        return NULL;
    }
    int directory_fd = -1;
    *status = open_runtime_directory(runtime_directory, uid, &directory_fd);
    if (*status != OB_SEMANTIC_ENDPOINT_OK) return NULL;
    char endpoint_name[OB_ENDPOINT_NAME_CAPACITY];
    char locator_name[OB_LOCATOR_NAME_CAPACITY];
    char endpoint_path[sizeof(((struct sockaddr_un *)0)->sun_path)];
    *status = build_names(
        runtime_directory,
        endpoint_id,
        endpoint_name,
        sizeof(endpoint_name),
        endpoint_path,
        sizeof(endpoint_path),
        locator_name,
        sizeof(locator_name));
    if (*status != OB_SEMANTIC_ENDPOINT_OK) {
        close(directory_fd);
        return NULL;
    }
    struct stat existing;
    if (fstatat(directory_fd, endpoint_name, &existing, AT_SYMLINK_NOFOLLOW) == 0) {
        close(directory_fd);
        *status = OB_SEMANTIC_ENDPOINT_UNAVAILABLE;
        return NULL;
    }
    if (errno != ENOENT) {
        close(directory_fd);
        *status = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        return NULL;
    }
    const int listener = socket(AF_UNIX, SOCK_STREAM, 0);
    if (listener < 0 || !set_close_on_exec(listener)) {
        if (listener >= 0) close(listener);
        close(directory_fd);
        *status = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        return NULL;
    }
    struct sockaddr_un address;
    memset(&address, 0, sizeof(address));
    address.sun_family = AF_UNIX;
#if defined(__APPLE__)
    address.sun_len = (uint8_t)sizeof(address);
#endif
    memcpy(address.sun_path, endpoint_path, strlen(endpoint_path) + 1);
    mode_t previous_mask = umask(S_IRWXG | S_IRWXO);
    const int bind_result = bind(listener, (const struct sockaddr *)&address, sizeof(address));
    umask(previous_mask);
    if (bind_result != 0 || fchmodat(directory_fd, endpoint_name, S_IRUSR | S_IWUSR, 0) != 0) {
        close(listener);
        (void)unlinkat(directory_fd, endpoint_name, 0);
        close(directory_fd);
        *status = bind_result != 0 && errno == EADDRINUSE
            ? OB_SEMANTIC_ENDPOINT_UNAVAILABLE
            : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        return NULL;
    }
    struct stat endpoint_status;
    if (fstatat(directory_fd, endpoint_name, &endpoint_status, AT_SYMLINK_NOFOLLOW) != 0 ||
        !S_ISSOCK(endpoint_status.st_mode) || (uint32_t)endpoint_status.st_uid != uid ||
        (endpoint_status.st_mode & (S_IRWXU | S_IRWXG | S_IRWXO)) !=
            (S_IRUSR | S_IWUSR)) {
        close(listener);
        (void)unlinkat(directory_fd, endpoint_name, 0);
        close(directory_fd);
        *status = OB_SEMANTIC_ENDPOINT_INSECURE;
        return NULL;
    }
    *status = validate_path_acl(endpoint_path);
    if (*status != OB_SEMANTIC_ENDPOINT_OK || listen(listener, 32) != 0) {
        close(listener);
        (void)unlinkat(directory_fd, endpoint_name, 0);
        close(directory_fd);
        if (*status == OB_SEMANTIC_ENDPOINT_OK) *status = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        return NULL;
    }
    ObSemanticCommandServer *server = calloc(1, sizeof(*server));
    if (server == NULL) {
        close(listener);
        (void)unlinkat(directory_fd, endpoint_name, 0);
        close(directory_fd);
        *status = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        return NULL;
    }
    server->client_fd = -1;
    server->listener_fd = listener;
    server->endpoint_device = endpoint_status.st_dev;
    server->endpoint_inode = endpoint_status.st_ino;
    memcpy(server->endpoint_path, endpoint_path, strlen(endpoint_path) + 1);
    close(directory_fd);
    *status = OB_SEMANTIC_ENDPOINT_OK;
    return server;
}

void ob_semantic_endpoint_server_disconnect(ObSemanticCommandServer *server) {
    if (server == NULL || server->client_fd < 0) return;
    (void)shutdown(server->client_fd, SHUT_RDWR);
    close(server->client_fd);
    server->client_fd = -1;
}

void ob_semantic_endpoint_server_destroy(ObSemanticCommandServer *server) {
    if (server == NULL) return;
    ob_semantic_endpoint_server_disconnect(server);
    if (server->listener_fd >= 0) close(server->listener_fd);
    struct stat status;
    if (lstat(server->endpoint_path, &status) == 0 && S_ISSOCK(status.st_mode) &&
        status.st_dev == server->endpoint_device && status.st_ino == server->endpoint_inode) {
        (void)unlink(server->endpoint_path);
    }
    free(server);
}

static int poll_descriptor(int descriptor, short events, uint32_t timeout_ms) {
    struct pollfd descriptor_poll = {.fd = descriptor, .events = events, .revents = 0};
    int result;
    do {
        result = poll(&descriptor_poll, 1, (int)timeout_ms);
    } while (result < 0 && errno == EINTR);
    if (result == 0) return OB_SEMANTIC_ENDPOINT_TIMEOUT;
    if (result < 0) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    if ((descriptor_poll.revents & events) != 0) return OB_SEMANTIC_ENDPOINT_OK;
    return (descriptor_poll.revents & (POLLHUP | POLLERR | POLLNVAL)) != 0
        ? OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED
        : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
}

int ob_semantic_endpoint_server_accept_before(
    ObSemanticCommandServer *server,
    uint32_t timeout_ms,
    uint32_t connection_budget_ms,
    uint32_t *remaining_request_ms) {
    if (server == NULL || remaining_request_ms == NULL || connection_budget_ms == 0 ||
        server->listener_fd < 0 || server->client_fd >= 0) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    int result = poll_descriptor(server->listener_fd, POLLIN, timeout_ms);
    if (result != OB_SEMANTIC_ENDPOINT_OK) return result;
    int client;
    do {
        client = accept(server->listener_fd, NULL, NULL);
    } while (client < 0 && errno == EINTR);
    if (client < 0 || !set_close_on_exec(client)) {
        if (client >= 0) close(client);
        return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    uint32_t uid;
    result = current_effective_uid(&uid)
        ? require_peer_uid(client, uid)
        : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    if (result != OB_SEMANTIC_ENDPOINT_OK) {
        close(client);
        return result;
    }
    server->client_fd = client;
    *remaining_request_ms = connection_budget_ms;
    return OB_SEMANTIC_ENDPOINT_OK;
}

int ob_semantic_endpoint_server_read_before(
    ObSemanticCommandServer *server,
    uint8_t *buffer,
    uint32_t capacity,
    uint32_t timeout_ms,
    uint32_t *read_bytes) {
    if (server == NULL || server->client_fd < 0 || buffer == NULL || capacity == 0 ||
        read_bytes == NULL) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    int result = poll_descriptor(server->client_fd, POLLIN, timeout_ms);
    if (result != OB_SEMANTIC_ENDPOINT_OK) return result;
    ssize_t count;
    do {
        count = read(server->client_fd, buffer, capacity);
    } while (count < 0 && errno == EINTR);
    if (count < 0) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    if (count == 0) return OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED;
    *read_bytes = (uint32_t)count;
    return OB_SEMANTIC_ENDPOINT_OK;
}

int ob_semantic_endpoint_server_write_before(
    ObSemanticCommandServer *server,
    const uint8_t *buffer,
    uint32_t length,
    uint32_t timeout_ms,
    uint32_t *written_bytes) {
    if (server == NULL || server->client_fd < 0 || buffer == NULL || length == 0 ||
        written_bytes == NULL) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    int result = poll_descriptor(server->client_fd, POLLOUT, timeout_ms);
    if (result != OB_SEMANTIC_ENDPOINT_OK) return result;
    ssize_t count;
    do {
        count = write(server->client_fd, buffer, length);
    } while (count < 0 && errno == EINTR);
    if (count < 0) return errno == EPIPE ? OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED
                                         : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    if (count == 0) return OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED;
    *written_bytes = (uint32_t)count;
    return OB_SEMANTIC_ENDPOINT_OK;
}

static int validate_private_file(
    int directory_fd,
    const char *name,
    const char *path,
    uint32_t uid,
    struct stat *status) {
    if (fstatat(directory_fd, name, status, AT_SYMLINK_NOFOLLOW) != 0) {
        return errno == ENOENT ? OB_SEMANTIC_ENDPOINT_UNAVAILABLE
                              : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    if (!S_ISREG(status->st_mode) || (uint32_t)status->st_uid != uid ||
        (status->st_mode & (S_IRWXU | S_IRWXG | S_IRWXO)) != (S_IRUSR | S_IWUSR) ||
        status->st_nlink != 1) {
        return OB_SEMANTIC_ENDPOINT_INSECURE;
    }
    return validate_path_acl(path);
}

static int locator_paths(
    const char *runtime_directory,
    const char *endpoint_id,
    char *locator_name,
    size_t locator_name_capacity,
    char *locator_path,
    size_t locator_path_capacity) {
    char endpoint_name[OB_ENDPOINT_NAME_CAPACITY];
    char endpoint_path[sizeof(((struct sockaddr_un *)0)->sun_path)];
    int result = build_names(
        runtime_directory,
        endpoint_id,
        endpoint_name,
        sizeof(endpoint_name),
        endpoint_path,
        sizeof(endpoint_path),
        locator_name,
        locator_name_capacity);
    if (result != OB_SEMANTIC_ENDPOINT_OK) return result;
    const size_t runtime_length = strlen(runtime_directory);
    const int path_length = snprintf(
        locator_path,
        locator_path_capacity,
        runtime_length > 0 && runtime_directory[runtime_length - 1] == '/' ? "%s%s" : "%s/%s",
        runtime_directory,
        locator_name);
    return path_length > 0 && (size_t)path_length < locator_path_capacity
        ? OB_SEMANTIC_ENDPOINT_OK
        : OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
}

int ob_semantic_endpoint_publish_locator(
    const char *runtime_directory,
    const char *endpoint_id,
    const uint8_t *content,
    uint32_t content_length) {
    if (content == NULL || content_length == 0 || content_length > 4096) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    int result = ob_semantic_endpoint_prepare_runtime_directory(runtime_directory);
    if (result != OB_SEMANTIC_ENDPOINT_OK) return result;
    uint32_t uid;
    int directory_fd = -1;
    if (!current_effective_uid(&uid)) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    result = open_runtime_directory(runtime_directory, uid, &directory_fd);
    if (result != OB_SEMANTIC_ENDPOINT_OK) return result;
    char locator_name[OB_LOCATOR_NAME_CAPACITY];
    char locator_path[4096];
    result = locator_paths(
        runtime_directory,
        endpoint_id,
        locator_name,
        sizeof(locator_name),
        locator_path,
        sizeof(locator_path));
    char temporary_name[OB_LOCATOR_NAME_CAPACITY + 8];
    if (result == OB_SEMANTIC_ENDPOINT_OK) {
        const int length = snprintf(
            temporary_name,
            sizeof(temporary_name),
            "%s.tmp",
            locator_name);
        if (length <= 0 || (size_t)length >= sizeof(temporary_name)) {
            result = OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
        }
    }
    int file = -1;
    struct stat existing;
    if (result == OB_SEMANTIC_ENDPOINT_OK &&
        fstatat(directory_fd, locator_name, &existing, AT_SYMLINK_NOFOLLOW) == 0) {
        result = OB_SEMANTIC_ENDPOINT_UNAVAILABLE;
    } else if (result == OB_SEMANTIC_ENDPOINT_OK && errno != ENOENT) {
        result = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    if (result == OB_SEMANTIC_ENDPOINT_OK) {
        file = openat(
            directory_fd,
            temporary_name,
            O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC,
            S_IRUSR | S_IWUSR);
        if (file < 0) result = errno == EEXIST ? OB_SEMANTIC_ENDPOINT_UNAVAILABLE
                                               : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    uint32_t offset = 0;
    while (result == OB_SEMANTIC_ENDPOINT_OK && offset < content_length) {
        const ssize_t count = write(file, content + offset, content_length - offset);
        if (count <= 0) {
            result = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        } else {
            offset += (uint32_t)count;
        }
    }
    if (result == OB_SEMANTIC_ENDPOINT_OK && (fsync(file) != 0 || fchmod(file, 0600) != 0)) {
        result = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    if (file >= 0 && close(file) != 0 && result == OB_SEMANTIC_ENDPOINT_OK) {
        result = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    if (result == OB_SEMANTIC_ENDPOINT_OK &&
        linkat(directory_fd, temporary_name, directory_fd, locator_name, 0) != 0) {
        result = errno == EEXIST ? OB_SEMANTIC_ENDPOINT_UNAVAILABLE
                                 : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    (void)unlinkat(directory_fd, temporary_name, 0);
    if (result == OB_SEMANTIC_ENDPOINT_OK) {
        struct stat status;
        result = validate_private_file(directory_fd, locator_name, locator_path, uid, &status);
    }
    close(directory_fd);
    return result;
}

int ob_semantic_endpoint_read_locator(
    const char *runtime_directory,
    const char *endpoint_id,
    uint8_t *buffer,
    uint32_t capacity,
    uint32_t *read_bytes) {
    if (buffer == NULL || capacity == 0 || read_bytes == NULL) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    uint32_t uid;
    int directory_fd = -1;
    if (!current_effective_uid(&uid)) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    int result = open_runtime_directory(runtime_directory, uid, &directory_fd);
    if (result != OB_SEMANTIC_ENDPOINT_OK) return result;
    char locator_name[OB_LOCATOR_NAME_CAPACITY];
    char locator_path[4096];
    result = locator_paths(
        runtime_directory,
        endpoint_id,
        locator_name,
        sizeof(locator_name),
        locator_path,
        sizeof(locator_path));
    struct stat status;
    if (result == OB_SEMANTIC_ENDPOINT_OK) {
        result = validate_private_file(directory_fd, locator_name, locator_path, uid, &status);
    }
    if (result == OB_SEMANTIC_ENDPOINT_OK && (uintmax_t)status.st_size > capacity) {
        result = OB_SEMANTIC_ENDPOINT_CAPACITY_EXCEEDED;
    }
    int file = -1;
    if (result == OB_SEMANTIC_ENDPOINT_OK) {
        file = openat(directory_fd, locator_name, O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
        if (file < 0) result = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    uint32_t offset = 0;
    while (result == OB_SEMANTIC_ENDPOINT_OK && offset < (uint32_t)status.st_size) {
        const ssize_t count = read(file, buffer + offset, (uint32_t)status.st_size - offset);
        if (count <= 0) {
            result = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        } else {
            offset += (uint32_t)count;
        }
    }
    if (file >= 0) close(file);
    close(directory_fd);
    if (result == OB_SEMANTIC_ENDPOINT_OK) *read_bytes = offset;
    return result;
}

int ob_semantic_endpoint_remove_locator(
    const char *runtime_directory,
    const char *endpoint_id) {
    uint32_t uid;
    int directory_fd = -1;
    if (!current_effective_uid(&uid)) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    int result = open_runtime_directory(runtime_directory, uid, &directory_fd);
    if (result != OB_SEMANTIC_ENDPOINT_OK) return result;
    char locator_name[OB_LOCATOR_NAME_CAPACITY];
    char locator_path[4096];
    result = locator_paths(
        runtime_directory,
        endpoint_id,
        locator_name,
        sizeof(locator_name),
        locator_path,
        sizeof(locator_path));
    struct stat status;
    if (result == OB_SEMANTIC_ENDPOINT_OK) {
        result = validate_private_file(directory_fd, locator_name, locator_path, uid, &status);
    }
    if (result == OB_SEMANTIC_ENDPOINT_OK && unlinkat(directory_fd, locator_name, 0) != 0) {
        result = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    close(directory_fd);
    return result == OB_SEMANTIC_ENDPOINT_UNAVAILABLE ? OB_SEMANTIC_ENDPOINT_OK : result;
}

static int connect_authenticated(
    const char *runtime_directory,
    const char *endpoint_id,
    int *connected_fd) {
    *connected_fd = -1;
    uint32_t uid;
    int directory_fd = -1;
    if (!current_effective_uid(&uid)) return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    int result = open_runtime_directory(runtime_directory, uid, &directory_fd);
    if (result != OB_SEMANTIC_ENDPOINT_OK) return result;
    char endpoint_name[OB_ENDPOINT_NAME_CAPACITY];
    char locator_name[OB_LOCATOR_NAME_CAPACITY];
    char endpoint_path[sizeof(((struct sockaddr_un *)0)->sun_path)];
    result = build_names(
        runtime_directory,
        endpoint_id,
        endpoint_name,
        sizeof(endpoint_name),
        endpoint_path,
        sizeof(endpoint_path),
        locator_name,
        sizeof(locator_name));
    struct stat endpoint_status;
    if (result == OB_SEMANTIC_ENDPOINT_OK &&
        fstatat(directory_fd, endpoint_name, &endpoint_status, AT_SYMLINK_NOFOLLOW) != 0) {
        result = errno == ENOENT ? OB_SEMANTIC_ENDPOINT_UNAVAILABLE
                                : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    if (result == OB_SEMANTIC_ENDPOINT_OK &&
        (!S_ISSOCK(endpoint_status.st_mode) || (uint32_t)endpoint_status.st_uid != uid ||
         (endpoint_status.st_mode & (S_IRWXU | S_IRWXG | S_IRWXO)) !=
             (S_IRUSR | S_IWUSR))) {
        result = OB_SEMANTIC_ENDPOINT_INSECURE;
    }
    if (result == OB_SEMANTIC_ENDPOINT_OK) result = validate_path_acl(endpoint_path);
    close(directory_fd);
    if (result != OB_SEMANTIC_ENDPOINT_OK) return result;
    const int descriptor = socket(AF_UNIX, SOCK_STREAM, 0);
    if (descriptor < 0 || !set_close_on_exec(descriptor)) {
        if (descriptor >= 0) close(descriptor);
        return OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    struct sockaddr_un address;
    memset(&address, 0, sizeof(address));
    address.sun_family = AF_UNIX;
#if defined(__APPLE__)
    address.sun_len = (uint8_t)sizeof(address);
#endif
    memcpy(address.sun_path, endpoint_path, strlen(endpoint_path) + 1);
    if (connect(descriptor, (const struct sockaddr *)&address, sizeof(address)) != 0) {
        close(descriptor);
        return errno == ENOENT || errno == ECONNREFUSED
            ? OB_SEMANTIC_ENDPOINT_UNAVAILABLE
            : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    }
    result = require_peer_uid(descriptor, uid);
    if (result != OB_SEMANTIC_ENDPOINT_OK) {
        close(descriptor);
        return result;
    }
    *connected_fd = descriptor;
    return OB_SEMANTIC_ENDPOINT_OK;
}

int ob_semantic_endpoint_client_exchange(
    const char *runtime_directory,
    const char *endpoint_id,
    const uint8_t *request,
    uint32_t request_length,
    uint8_t *response,
    uint32_t response_capacity,
    uint32_t timeout_ms,
    uint32_t *response_length) {
    if (request == NULL || request_length == 0 || response == NULL || response_capacity == 0 ||
        response_length == NULL) {
        return OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT;
    }
    int descriptor = -1;
    int result = connect_authenticated(runtime_directory, endpoint_id, &descriptor);
    if (result != OB_SEMANTIC_ENDPOINT_OK) return result;
    uint32_t request_offset = 0;
    while (result == OB_SEMANTIC_ENDPOINT_OK && request_offset < request_length) {
        result = poll_descriptor(descriptor, POLLOUT, timeout_ms);
        if (result != OB_SEMANTIC_ENDPOINT_OK) break;
        const ssize_t count = write(descriptor, request + request_offset, request_length - request_offset);
        if (count <= 0) {
            result = errno == EPIPE ? OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED
                                    : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        } else {
            request_offset += (uint32_t)count;
        }
    }
    uint32_t response_offset = 0;
    while (result == OB_SEMANTIC_ENDPOINT_OK) {
        result = poll_descriptor(descriptor, POLLIN, timeout_ms);
        if (result == OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED && response_offset > 0) {
            result = OB_SEMANTIC_ENDPOINT_OK;
            break;
        }
        if (result != OB_SEMANTIC_ENDPOINT_OK) break;
        if (response_offset == response_capacity) {
            result = OB_SEMANTIC_ENDPOINT_CAPACITY_EXCEEDED;
            break;
        }
        const ssize_t count = read(
            descriptor,
            response + response_offset,
            response_capacity - response_offset);
        if (count < 0) {
            result = errno == EINTR ? OB_SEMANTIC_ENDPOINT_OK : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        } else if (count == 0) {
            break;
        } else {
            response_offset += (uint32_t)count;
        }
    }
    close(descriptor);
    if (result == OB_SEMANTIC_ENDPOINT_OK) *response_length = response_offset;
    return result;
}
