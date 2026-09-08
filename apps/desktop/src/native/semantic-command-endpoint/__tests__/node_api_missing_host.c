#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include <stdio.h>
#include <stdlib.h>

/* This host deliberately omits napi_throw_error, the last required import. No stub may run. */
#ifdef OB_PARTIAL_NODE_API
#define OB_PARTIAL_IMPORTS(APPLY) \
    APPLY(napi_create_async_work) \
    APPLY(napi_create_buffer_copy) \
    APPLY(napi_create_error) \
    APPLY(napi_create_external) \
    APPLY(napi_create_promise) \
    APPLY(napi_create_string_utf8) \
    APPLY(napi_create_uint32) \
    APPLY(napi_define_properties) \
    APPLY(napi_delete_async_work) \
    APPLY(napi_get_boolean) \
    APPLY(napi_get_buffer_info) \
    APPLY(napi_get_cb_info) \
    APPLY(napi_get_null) \
    APPLY(napi_get_undefined) \
    APPLY(napi_get_value_external) \
    APPLY(napi_get_value_string_utf8) \
    APPLY(napi_get_value_uint32) \
    APPLY(napi_is_buffer) \
    APPLY(napi_queue_async_work) \
    APPLY(napi_reject_deferred) \
    APPLY(napi_resolve_deferred) \
    APPLY(napi_set_named_property) \
    APPLY(napi_throw)
#define OB_STUB(name) __declspec(dllexport) void name(void) { abort(); }
OB_PARTIAL_IMPORTS(OB_STUB)
#undef OB_STUB
#endif

int wmain(int count, wchar_t **arguments) {
    if (count != 2) return 2;
#ifdef OB_PARTIAL_NODE_API
    HMODULE host = GetModuleHandleW(NULL);
#define OB_CHECK_EXPORT(name) \
    if (GetProcAddress(host, #name) == NULL) { \
        fputs("The partial host is missing fixture export " #name ".\n", stderr); \
        return 1; \
    }
    OB_PARTIAL_IMPORTS(OB_CHECK_EXPORT)
#undef OB_CHECK_EXPORT
    if (GetProcAddress(host, "napi_throw_error") != NULL) return 1;
#endif
    HMODULE addon = LoadLibraryW(arguments[1]);
    if (addon != NULL) {
        FreeLibrary(addon);
        fputs("An incomplete Node-API host unexpectedly loaded the addon.\n", stderr);
        return 1;
    }
    DWORD error = GetLastError();
    if (error != ERROR_DLL_INIT_FAILED) {
        fprintf(stderr, "Expected DLL initialization refusal, received %lu.\n", error);
        return 1;
    }
    if (GetModuleHandleW(L"NODE.EXE") != NULL) {
        fputs("The addon loaded a separate Node executable.\n", stderr);
        return 1;
    }
    return 0;
}
