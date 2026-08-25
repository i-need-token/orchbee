#define _GNU_SOURCE

#include "posix_host_endpoint.h"

#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
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
#include <uuid/uuid.h>
#elif defined(__linux__)
#include <sys/xattr.h>
#endif

#define CHECK(condition, message) do { \
    if (!(condition)) { \
        fprintf(stderr, "POSIX Host endpoint test failed: %s\n", message); \
        return 1; \
    } \
} while (0)

enum acl_test_result {
    ACL_TEST_ADDED = 0,
    ACL_TEST_UNSUPPORTED = 1,
    ACL_TEST_FAILED = 2,
};

static uint32_t different_uid(uint32_t current_uid) {
    return current_uid == UINT32_MAX ? current_uid - 1 : current_uid + 1;
}

#if defined(__APPLE__)
static acl_t everyone_acl(void) {
    acl_t acl = acl_init(1);
    acl_entry_t entry;
    acl_permset_t permissions;
    acl_flagset_t flags;
    uuid_t everyone;
    if (acl == NULL ||
        uuid_parse("ABCDEFAB-CDEF-ABCD-EFAB-CDEF0000000C", everyone) != 0 ||
        acl_create_entry(&acl, &entry) != 0 ||
        acl_set_tag_type(entry, ACL_EXTENDED_ALLOW) != 0 ||
        acl_set_qualifier(entry, everyone) != 0 ||
        acl_get_permset(entry, &permissions) != 0 ||
        acl_clear_perms(permissions) != 0 ||
        acl_add_perm(permissions, ACL_READ_DATA) != 0 ||
        acl_set_permset(entry, permissions) != 0 ||
        acl_get_flagset_np(entry, &flags) != 0 ||
        acl_clear_flags_np(flags) != 0 ||
        acl_set_flagset_np(entry, flags) != 0) {
        if (acl != NULL) acl_free(acl);
        return NULL;
    }
    return acl;
}

static enum acl_test_result add_directory_acl(const char *path) {
    const int descriptor = open(path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW);
    if (descriptor < 0) return ACL_TEST_FAILED;
    acl_t acl = everyone_acl();
    if (acl == NULL) {
        close(descriptor);
        return ACL_TEST_FAILED;
    }
    const int result = acl_set_fd_np(descriptor, acl, ACL_TYPE_EXTENDED);
    acl_free(acl);
    close(descriptor);
    return result == 0 ? ACL_TEST_ADDED : ACL_TEST_FAILED;
}

static int directory_acl_is_absent(const char *path) {
    const int descriptor = open(path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW);
    if (descriptor < 0) return 0;
    errno = 0;
    acl_t acl = acl_get_fd_np(descriptor, ACL_TYPE_EXTENDED);
    close(descriptor);
    if (acl == NULL) return errno == ENOENT;
    acl_free(acl);
    return 0;
}

static enum acl_test_result add_endpoint_acl(const char *path) {
    acl_t acl = everyone_acl();
    if (acl == NULL) return ACL_TEST_FAILED;
    const int result = acl_set_link_np(path, ACL_TYPE_EXTENDED, acl);
    acl_free(acl);
    return result == 0 ? ACL_TEST_ADDED : ACL_TEST_FAILED;
}
#elif defined(__linux__)
struct linux_acl_header {
    uint32_t version;
};

struct linux_acl_entry {
    uint16_t tag;
    uint16_t permissions;
    uint32_t identifier;
};

struct linux_test_acl {
    struct linux_acl_header header;
    struct linux_acl_entry entries[5];
};

static struct linux_test_acl test_acl(uint32_t named_uid) {
    return (struct linux_test_acl){
        .header = { .version = 2 },
        .entries = {
            { .tag = 0x01, .permissions = 7, .identifier = UINT32_MAX },
            { .tag = 0x02, .permissions = 0, .identifier = named_uid },
            { .tag = 0x04, .permissions = 0, .identifier = UINT32_MAX },
            { .tag = 0x10, .permissions = 0, .identifier = UINT32_MAX },
            { .tag = 0x20, .permissions = 0, .identifier = UINT32_MAX },
        },
    };
}

static enum acl_test_result add_acl(const char *path) {
    const struct linux_test_acl acl = test_acl(different_uid((uint32_t)geteuid()));
    if (lsetxattr(path, "system.posix_acl_access", &acl, sizeof(acl), 0) == 0) {
        return ACL_TEST_ADDED;
    }
    return errno == ENOTSUP || errno == EOPNOTSUPP ? ACL_TEST_UNSUPPORTED :
        ACL_TEST_FAILED;
}

static enum acl_test_result add_directory_acl(const char *path) {
    return add_acl(path);
}

static int directory_acl_is_absent(const char *path) {
    errno = 0;
    return lgetxattr(path, "system.posix_acl_access", NULL, 0) < 0 &&
        (errno == ENODATA || errno == ENOTSUP || errno == EOPNOTSUPP);
}

static enum acl_test_result add_endpoint_acl(const char *path) {
    return add_acl(path);
}
#endif

static int create_listener(const char *endpoint_path) {
    const int descriptor = socket(AF_UNIX, SOCK_STREAM, 0);
    if (descriptor < 0) return -1;
    struct sockaddr_un address;
    memset(&address, 0, sizeof(address));
    address.sun_family = AF_UNIX;
    if (strlen(endpoint_path) >= sizeof(address.sun_path)) {
        close(descriptor);
        return -1;
    }
    memcpy(address.sun_path, endpoint_path, strlen(endpoint_path) + 1);
#if defined(__APPLE__)
    address.sun_len = (uint8_t)sizeof(address);
#endif
    if (bind(descriptor, (const struct sockaddr *)&address, sizeof(address)) != 0 ||
        chmod(endpoint_path, S_IRUSR | S_IWUSR) != 0 ||
        listen(descriptor, 1) != 0) {
        close(descriptor);
        return -1;
    }
    return descriptor;
}

int main(void) {
    char test_root[] = "/tmp/ob-desktop-native-XXXXXX";
    CHECK(mkdtemp(test_root) != NULL, "could not create test root");
    char runtime_directory[128];
    char endpoint_path[192];
    char linked_directory[128];
    char real_directory[128];
    const char *const endpoint_id = "0123456789abcdef0123456789abcdef";
    CHECK(
        snprintf(runtime_directory, sizeof(runtime_directory), "%s/r", test_root) > 0,
        "could not format runtime path");
    CHECK(
        snprintf(endpoint_path, sizeof(endpoint_path), "%s/%s.s", runtime_directory, endpoint_id) > 0,
        "could not format endpoint path");
    CHECK(
        snprintf(real_directory, sizeof(real_directory), "%s/real", test_root) > 0,
        "could not format real path");
    CHECK(
        snprintf(linked_directory, sizeof(linked_directory), "%s/link", test_root) > 0,
        "could not format link path");

    const uint32_t current_uid = (uint32_t)geteuid();
    CHECK(
        ob_desktop_prepare_runtime_directory(runtime_directory, current_uid) ==
            OB_DESKTOP_POSIX_ENDPOINT_OK,
        "private runtime preparation failed");
    struct stat runtime_status;
    CHECK(stat(runtime_directory, &runtime_status) == 0, "runtime stat failed");
    CHECK((runtime_status.st_mode & 0777) == 0700, "runtime mode is not private");
    CHECK(
        ob_desktop_prepare_runtime_directory(runtime_directory, different_uid(current_uid)) ==
            OB_DESKTOP_POSIX_ENDPOINT_INSECURE,
        "foreign runtime owner was accepted");
    CHECK(chmod(runtime_directory, 0755) == 0, "could not make runtime public");
    CHECK(
        ob_desktop_prepare_runtime_directory(runtime_directory, current_uid) ==
            OB_DESKTOP_POSIX_ENDPOINT_INSECURE,
        "public runtime mode was repaired instead of rejected");
    CHECK(chmod(runtime_directory, 0700) == 0, "could not restore runtime mode");

    CHECK(mkdir(real_directory, 0700) == 0, "could not create symlink target");
    CHECK(symlink(real_directory, linked_directory) == 0, "could not create runtime symlink");
    CHECK(
        ob_desktop_prepare_runtime_directory(linked_directory, current_uid) ==
            OB_DESKTOP_POSIX_ENDPOINT_INSECURE,
        "runtime symlink was accepted");

    const enum acl_test_result directory_acl = add_directory_acl(runtime_directory);
    CHECK(directory_acl != ACL_TEST_FAILED, "could not add runtime ACL");
    if (directory_acl == ACL_TEST_ADDED) {
        CHECK(
            ob_desktop_prepare_runtime_directory(runtime_directory, current_uid) ==
                OB_DESKTOP_POSIX_ENDPOINT_OK,
            "runtime ACL was not removed during preparation");
        CHECK(directory_acl_is_absent(runtime_directory), "runtime ACL remains after preparation");
    }

    char long_runtime_directory[256];
    char long_component[81];
    memset(long_component, 'x', sizeof(long_component) - 1);
    long_component[sizeof(long_component) - 1] = '\0';
    CHECK(
        snprintf(
            long_runtime_directory,
            sizeof(long_runtime_directory),
            "%s/%s",
            test_root,
            long_component) > 0,
        "could not format overlong runtime path");
    CHECK(
        strlen(long_runtime_directory) + 1 + 34 >=
            sizeof(((struct sockaddr_un *)NULL)->sun_path),
        "overlong runtime fixture still fits the socket path");
    CHECK(mkdir(long_runtime_directory, 0700) == 0, "could not create overlong runtime");
    int connected_fd = -1;
    CHECK(
        ob_desktop_connect_authenticated(
            long_runtime_directory,
            endpoint_id,
            current_uid,
            &connected_fd) == OB_DESKTOP_POSIX_ENDPOINT_INVALID_ARGUMENT,
        "overlong endpoint path was accepted");
    CHECK(connected_fd == -1, "overlong endpoint returned a descriptor");
    CHECK(rmdir(long_runtime_directory) == 0, "could not remove overlong runtime");

    CHECK(
        ob_desktop_connect_authenticated(
            runtime_directory,
            endpoint_id,
            current_uid,
            &connected_fd) == OB_DESKTOP_POSIX_ENDPOINT_UNAVAILABLE,
        "missing endpoint did not report unavailable");
    CHECK(connected_fd == -1, "missing endpoint returned a descriptor");

    const int regular_fd = open(endpoint_path, O_CREAT | O_EXCL | O_RDWR, 0600);
    CHECK(regular_fd >= 0, "could not create non-socket endpoint");
    close(regular_fd);
    CHECK(
        ob_desktop_connect_authenticated(
            runtime_directory,
            endpoint_id,
            current_uid,
            &connected_fd) == OB_DESKTOP_POSIX_ENDPOINT_INSECURE,
        "non-socket endpoint was accepted");
    CHECK(unlink(endpoint_path) == 0, "could not remove non-socket endpoint");

    const int listener_fd = create_listener(endpoint_path);
    CHECK(listener_fd >= 0, "could not create private listener");
    CHECK(
        ob_desktop_connect_authenticated(
            runtime_directory,
            endpoint_id,
            current_uid,
            &connected_fd) == OB_DESKTOP_POSIX_ENDPOINT_OK,
        "same-UID listener authentication failed");
    CHECK(connected_fd >= 0, "authenticated connection returned no descriptor");
    const int accepted_fd = accept(listener_fd, NULL, NULL);
    CHECK(accepted_fd >= 0, "listener did not accept authenticated connection");
    close(accepted_fd);
    close(connected_fd);

    CHECK(chmod(endpoint_path, 0666) == 0, "could not make endpoint public");
    CHECK(
        ob_desktop_connect_authenticated(
            runtime_directory,
            endpoint_id,
            current_uid,
            &connected_fd) == OB_DESKTOP_POSIX_ENDPOINT_INSECURE,
        "public endpoint was accepted");
    CHECK(chmod(endpoint_path, 0600) == 0, "could not restore endpoint mode");

    const enum acl_test_result endpoint_acl = add_endpoint_acl(endpoint_path);
    CHECK(endpoint_acl != ACL_TEST_FAILED, "could not add endpoint ACL");
    if (endpoint_acl == ACL_TEST_ADDED) {
        CHECK(
            ob_desktop_connect_authenticated(
                runtime_directory,
                endpoint_id,
                current_uid,
                &connected_fd) == OB_DESKTOP_POSIX_ENDPOINT_INSECURE,
            "ACL-accessible endpoint was accepted");
    }
    close(listener_fd);
    CHECK(unlink(endpoint_path) == 0, "could not remove listener endpoint");

    int peers[2];
    CHECK(socketpair(AF_UNIX, SOCK_STREAM, 0, peers) == 0, "socketpair failed");
    CHECK(
        ob_desktop_require_peer_effective_uid(peers[0], current_uid) ==
            OB_DESKTOP_POSIX_ENDPOINT_OK,
        "same-UID socket peer was rejected");
    CHECK(
        ob_desktop_require_peer_effective_uid(peers[0], different_uid(current_uid)) ==
            OB_DESKTOP_POSIX_ENDPOINT_PEER_IDENTITY_MISMATCH,
        "different expected UID was accepted");
    close(peers[0]);
    close(peers[1]);

    int pipe_descriptors[2];
    CHECK(pipe(pipe_descriptors) == 0, "pipe creation failed");
    CHECK(
        ob_desktop_require_peer_effective_uid(pipe_descriptors[0], current_uid) ==
            OB_DESKTOP_POSIX_ENDPOINT_PEER_IDENTITY_UNAVAILABLE,
        "non-socket peer credentials were accepted");
    close(pipe_descriptors[0]);
    close(pipe_descriptors[1]);

    CHECK(
        ob_desktop_connect_authenticated(
            runtime_directory,
            "not-an-endpoint-id",
            current_uid,
            &connected_fd) == OB_DESKTOP_POSIX_ENDPOINT_INVALID_ARGUMENT,
        "invalid endpoint ID was accepted");
    CHECK(unlink(linked_directory) == 0, "could not remove runtime symlink");
    CHECK(rmdir(real_directory) == 0, "could not remove symlink target");
    CHECK(rmdir(runtime_directory) == 0, "could not remove runtime directory");
    CHECK(rmdir(test_root) == 0, "could not remove test root");
    puts("Desktop POSIX Host endpoint native tests passed.");
    return 0;
}
