#include "posix_endpoint_security_test.h"

#include <stddef.h>
#include <sys/acl.h>
#include <uuid/uuid.h>

static acl_t everyone_acl(int inheritable) {
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
        acl_add_perm(permissions, ACL_LIST_DIRECTORY) != 0 ||
        acl_add_perm(permissions, ACL_ADD_FILE) != 0 ||
        acl_add_perm(permissions, ACL_SEARCH) != 0 ||
        acl_add_perm(permissions, ACL_DELETE_CHILD) != 0 ||
        acl_set_permset(entry, permissions) != 0 ||
        acl_get_flagset_np(entry, &flags) != 0 ||
        acl_clear_flags_np(flags) != 0) {
        if (acl != NULL) acl_free(acl);
        return NULL;
    }
    if (inheritable != 0 &&
        (acl_add_flag_np(flags, ACL_ENTRY_FILE_INHERIT) != 0 ||
         acl_add_flag_np(flags, ACL_ENTRY_DIRECTORY_INHERIT) != 0)) {
        acl_free(acl);
        return NULL;
    }
    if (acl_set_flagset_np(entry, flags) != 0) {
        acl_free(acl);
        return NULL;
    }
    return acl;
}

int ob_test_add_darwin_everyone_acl_fd(int descriptor, int inheritable) {
    acl_t acl = everyone_acl(inheritable);
    if (acl == NULL) return -1;
    const int result = acl_set_fd_np(descriptor, acl, ACL_TYPE_EXTENDED);
    acl_free(acl);
    return result;
}

int ob_test_add_darwin_everyone_acl_link(const char *path) {
    acl_t acl = everyone_acl(0);
    if (acl == NULL) return -1;
    const int result = acl_set_link_np(path, ACL_TYPE_EXTENDED, acl);
    acl_free(acl);
    return result;
}

int ob_test_darwin_fd_has_extended_acl(int descriptor) {
    acl_t acl = acl_get_fd_np(descriptor, ACL_TYPE_EXTENDED);
    if (acl == NULL) return 0;
    acl_free(acl);
    return 1;
}

int ob_test_darwin_link_has_extended_acl(const char *path) {
    acl_t acl = acl_get_link_np(path, ACL_TYPE_EXTENDED);
    if (acl == NULL) return 0;
    acl_free(acl);
    return 1;
}
