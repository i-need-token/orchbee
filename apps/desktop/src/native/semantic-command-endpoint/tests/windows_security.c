#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <aclapi.h>
#include <sddl.h>
#include <stdio.h>
#include <wchar.h>

// The real NTFS operations run under the ordinary test account. This one API-boundary
// fixture reports a different owner without creating accounts or taking another user's files.
static BOOL report_foreign_owner = FALSE;
static unsigned security_writes = 0;
static BOOL directory_owner_explicit = FALSE;
static BOOL locator_owner_explicit = FALSE;
static PSID expected_user = NULL;
static BYTE foreign_owner[SECURITY_MAX_SID_SIZE];

static BOOL explicit_user_owner(const SECURITY_ATTRIBUTES *security) {
    PSID owner = NULL;
    BOOL defaulted = FALSE;
    return security != NULL && security->lpSecurityDescriptor != NULL &&
        GetSecurityDescriptorOwner(security->lpSecurityDescriptor, &owner, &defaulted) &&
        owner != NULL && !defaulted && EqualSid(owner, expected_user);
}

static BOOL test_create_directory(LPCWSTR path, LPSECURITY_ATTRIBUTES security) {
    directory_owner_explicit = explicit_user_owner(security);
    return CreateDirectoryW(path, security);
}

static HANDLE test_create_file(
    LPCWSTR path, DWORD access, DWORD sharing, LPSECURITY_ATTRIBUTES security,
    DWORD creation, DWORD flags, HANDLE template_file) {
    if (creation == CREATE_NEW) locator_owner_explicit = explicit_user_owner(security);
    return CreateFileW(path, access, sharing, security, creation, flags, template_file);
}

static DWORD test_get_security_info(
    HANDLE handle, SE_OBJECT_TYPE type, SECURITY_INFORMATION information,
    PSID *owner, PSID *group, PACL *dacl, PACL *sacl, PSECURITY_DESCRIPTOR *descriptor) {
    DWORD status = GetSecurityInfo(handle, type, information, owner, group, dacl, sacl, descriptor);
    if (status != ERROR_SUCCESS || !report_foreign_owner) return status;
    SECURITY_DESCRIPTOR replacement;
    DWORD length = 0;
    if (!InitializeSecurityDescriptor(&replacement, SECURITY_DESCRIPTOR_REVISION) ||
        !SetSecurityDescriptorOwner(&replacement, foreign_owner, FALSE) ||
        !SetSecurityDescriptorDacl(&replacement, TRUE, *dacl, FALSE)) {
        return ERROR_INVALID_SECURITY_DESCR;
    }
    MakeSelfRelativeSD(&replacement, NULL, &length);
    PSECURITY_DESCRIPTOR relative = LocalAlloc(LPTR, length);
    if (relative == NULL || !MakeSelfRelativeSD(&replacement, relative, &length)) {
        if (relative != NULL) LocalFree(relative);
        return ERROR_NOT_ENOUGH_MEMORY;
    }
    LocalFree(*descriptor);
    *descriptor = relative;
    BOOL defaulted = FALSE;
    BOOL present = FALSE;
    if (!GetSecurityDescriptorOwner(relative, owner, &defaulted) ||
        !GetSecurityDescriptorDacl(relative, &present, dacl, &defaulted)) {
        return ERROR_INVALID_SECURITY_DESCR;
    }
    return ERROR_SUCCESS;
}

DWORD test_set_security_info(
    HANDLE handle, SE_OBJECT_TYPE type, SECURITY_INFORMATION information,
    PSID owner, PSID group, PACL dacl, PACL sacl) {
    security_writes += 1;
    return SetSecurityInfo(handle, type, information, owner, group, dacl, sacl);
}

DWORD test_set_named_security_info(
    LPWSTR path, SE_OBJECT_TYPE type, SECURITY_INFORMATION information,
    PSID owner, PSID group, PACL dacl, PACL sacl) {
    security_writes += 1;
    return SetNamedSecurityInfoW(path, type, information, owner, group, dacl, sacl);
}

#define CreateDirectoryW test_create_directory
#define CreateFileW test_create_file
#define GetSecurityInfo test_get_security_info
#define SetSecurityInfo test_set_security_info
#define SetNamedSecurityInfoW test_set_named_security_info
#ifndef OB_WINDOWS_ENDPOINT_SOURCE
#define OB_WINDOWS_ENDPOINT_SOURCE "../semantic_command_endpoint_windows.c"
#endif
#include OB_WINDOWS_ENDPOINT_SOURCE
#undef CreateDirectoryW
#undef CreateFileW
#undef GetSecurityInfo
#undef SetSecurityInfo
#undef SetNamedSecurityInfoW

static unsigned failures = 0;
static void require(BOOL condition, const char *claim) {
    if (condition) printf("PASS: %s\n", claim);
    else {
        fprintf(stderr, "FAIL: %s\n", claim);
        failures += 1;
    }
}

static LPWSTR security_text(const wchar_t *path) {
    HANDLE handle = CreateFileW(path, READ_CONTROL, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        NULL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT, NULL);
    if (handle == INVALID_HANDLE_VALUE) return NULL;
    PSECURITY_DESCRIPTOR descriptor = NULL;
    LPWSTR text = NULL;
    if (GetSecurityInfo(handle, SE_FILE_OBJECT, OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
        NULL, NULL, NULL, NULL, &descriptor) == ERROR_SUCCESS) {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(descriptor, SDDL_REVISION_1,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION, &text, NULL);
        LocalFree(descriptor);
    }
    CloseHandle(handle);
    return text;
}

static BOOL grant_public_access(const wchar_t *path, BOOL null_dacl) {
    PSECURITY_DESCRIPTOR descriptor = NULL;
    PACL dacl = NULL;
    BOOL present = FALSE;
    BOOL defaulted = FALSE;
    if (!null_dacl && (!ConvertStringSecurityDescriptorToSecurityDescriptorW(
        L"D:P(A;;FA;;;WD)", SDDL_REVISION_1, &descriptor, NULL) ||
        !GetSecurityDescriptorDacl(descriptor, &present, &dacl, &defaulted))) return FALSE;
    DWORD result = SetNamedSecurityInfoW((LPWSTR)path, SE_FILE_OBJECT,
        DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION, NULL, NULL, dacl, NULL);
    if (descriptor != NULL) LocalFree(descriptor);
    return result == ERROR_SUCCESS;
}

static BOOL private_path(const wchar_t *path, BOOL directory) {
    HANDLE handle = CreateFileW(path, READ_CONTROL, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        NULL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT, NULL);
    if (handle == INVALID_HANDLE_VALUE) return FALSE;
    BOOL valid = private_handle(handle, expected_user, directory);
    CloseHandle(handle);
    return valid;
}

int wmain(int argc, wchar_t **argv) {
    if (argc != 4) return 2; // New runtime, existing junction, and its target (all runner-owned).
    char runtime[4096], junction[4096], target[4096];
    if (!WideCharToMultiByte(CP_UTF8, 0, argv[1], -1, runtime, sizeof(runtime), NULL, NULL) ||
        !WideCharToMultiByte(CP_UTF8, 0, argv[2], -1, junction, sizeof(junction), NULL, NULL) ||
        !WideCharToMultiByte(CP_UTF8, 0, argv[3], -1, target, sizeof(target), NULL, NULL)) return 2;
    BYTE *user = current_token_user();
    if (user == NULL) return 2;
    expected_user = ((TOKEN_USER *)user)->User.Sid;
    DWORD foreign_length = sizeof(foreign_owner);
    if (!CreateWellKnownSid(WinBuiltinAdministratorsSid, NULL, foreign_owner, &foreign_length) ||
        EqualSid(expected_user, foreign_owner)) return 2;

    // Resolve the owner as Windows does when the descriptor omits one, with a scripted
    // TokenOwner=Administrators distinct from TokenUser. No token/privilege is changed.
    PSECURITY_DESCRIPTOR descriptor = current_user_descriptor(expected_user);
    PSID owner = NULL;
    BOOL defaulted = FALSE;
    BOOL has_owner = descriptor != NULL && GetSecurityDescriptorOwner(descriptor, &owner, &defaulted);
    PSID resolved_owner = has_owner && owner != NULL ? owner : foreign_owner;
    require(EqualSid(resolved_owner, expected_user), "TokenOwner != TokenUser: descriptor pins current user");
    SECURITY_DESCRIPTOR_CONTROL control = 0;
    DWORD revision = 0;
    require(descriptor != NULL && GetSecurityDescriptorControl(descriptor, &control, &revision) &&
        (control & SE_DACL_PROTECTED) != 0, "creation DACL is protected from parent inheritance");
    free_current_user_descriptor(descriptor);

    require(ob_semantic_endpoint_prepare_runtime_directory(runtime) == OB_SEMANTIC_ENDPOINT_OK,
        "new runtime directory prepares successfully");
    require(directory_owner_explicit, "directory owner is explicit at creation");
    require(private_path(argv[1], TRUE), "created directory passes real NTFS owner and ACL validation");
    require(ob_semantic_endpoint_prepare_runtime_directory(runtime) == OB_SEMANTIC_ENDPOINT_OK,
        "existing private current-user directory is reusable");
    require(grant_public_access(argv[1], FALSE), "current-user broad DACL fixture is installed");
    require(ob_semantic_endpoint_prepare_runtime_directory(runtime) == OB_SEMANTIC_ENDPOINT_OK &&
        private_path(argv[1], TRUE), "existing current-user broad DACL is tightened");
    require(grant_public_access(argv[1], TRUE), "current-user null DACL fixture is installed");
    require(ob_semantic_endpoint_prepare_runtime_directory(runtime) == OB_SEMANTIC_ENDPOINT_OK &&
        private_path(argv[1], TRUE), "existing current-user null DACL is tightened");

    const char *endpoint = "abababababababababababababababab";
    const uint8_t content[] = "owner-test";
    uint8_t buffer[32];
    uint32_t read = 0;
    require(ob_semantic_endpoint_publish_locator(runtime, endpoint, content, sizeof(content)) == OB_SEMANTIC_ENDPOINT_OK,
        "locator publication succeeds");
    require(locator_owner_explicit, "locator owner is explicit at CREATE_NEW");
    require(ob_semantic_endpoint_read_locator(runtime, endpoint, buffer, sizeof(buffer), &read) == OB_SEMANTIC_ENDPOINT_OK &&
        read == sizeof(content) && memcmp(buffer, content, sizeof(content)) == 0,
        "locator passes real owner validation and bytes round-trip");

    require(grant_public_access(argv[1], FALSE), "foreign-owner boundary fixture starts with observable broad ACL");
    LPWSTR before = security_text(argv[1]);
    report_foreign_owner = TRUE;
    security_writes = 0;
    int status = ob_semantic_endpoint_prepare_runtime_directory(runtime);
    report_foreign_owner = FALSE;
    LPWSTR after = security_text(argv[1]);
    require(status == OB_SEMANTIC_ENDPOINT_INSECURE, "foreign-owner API result is rejected");
    require(security_writes == 0 && before != NULL && after != NULL && wcscmp(before, after) == 0,
        "foreign-owner refusal performs no security write and preserves real object security");
    if (before != NULL) LocalFree(before);
    if (after != NULL) LocalFree(after);

    require(grant_public_access(argv[3], FALSE), "junction target starts with observable broad ACL");
    before = security_text(argv[3]);
    security_writes = 0;
    status = ob_semantic_endpoint_prepare_runtime_directory(junction);
    after = security_text(argv[3]);
    require(status == OB_SEMANTIC_ENDPOINT_INSECURE, "real Windows directory junction is rejected");
    require(security_writes == 0 && before != NULL && after != NULL && wcscmp(before, after) == 0,
        "junction refusal performs no security write and preserves its target");
    if (before != NULL) LocalFree(before);
    if (after != NULL) LocalFree(after);

    HeapFree(GetProcessHeap(), 0, user);
    printf("Windows semantic endpoint security: %u failure(s).\n", failures);
    return failures == 0 ? 0 : 1;
}
