#ifndef ORCHBEE_POSIX_ENDPOINT_SECURITY_TEST_H
#define ORCHBEE_POSIX_ENDPOINT_SECURITY_TEST_H

int ob_test_add_darwin_everyone_acl_fd(int descriptor, int inheritable);
int ob_test_add_darwin_everyone_acl_link(const char *path);
int ob_test_darwin_fd_has_extended_acl(int descriptor);
int ob_test_darwin_link_has_extended_acl(const char *path);

#endif
