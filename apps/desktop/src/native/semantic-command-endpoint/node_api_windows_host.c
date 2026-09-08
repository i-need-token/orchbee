#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <node_api.h>

/* Match the pinned Windows GNU compiler's imported C function slots, including x86 decoration. */
#if defined(__i386__)
#define OB_IMPORT_PREFIX "__imp__"
#elif defined(__x86_64__) || defined(__aarch64__)
#define OB_IMPORT_PREFIX "__imp_"
#else
#error Unsupported Windows Node-API architecture
#endif

/* An additional Node-API call without a corresponding slot fails at link time. */
#define OB_NODE_API_IMPORTS(APPLY) \
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
    APPLY(napi_throw) \
    APPLY(napi_throw_error)

#define OB_DECLARE_NODE_API(name) \
    __typeof__(&name) ob_import_##name __asm__(OB_IMPORT_PREFIX #name);
OB_NODE_API_IMPORTS(OB_DECLARE_NODE_API)
#undef OB_DECLARE_NODE_API

BOOL WINAPI DllMain(HINSTANCE module, DWORD reason, LPVOID reserved) {
    (void)module;
    (void)reserved;
    if (reason != DLL_PROCESS_ATTACH) return TRUE;

    /* Borrow the process image for its lifetime. Never search for or load a second Node runtime. */
    HMODULE host = GetModuleHandleW(NULL);
    if (host == NULL) return FALSE;

    /* Windows finishes this initialization before exposing module registration to any consumer. */
#define OB_RESOLVE_NODE_API(name) \
    ob_import_##name = (__typeof__(&name))GetProcAddress(host, #name); \
    if (ob_import_##name == NULL) return FALSE;
    OB_NODE_API_IMPORTS(OB_RESOLVE_NODE_API)
#undef OB_RESOLVE_NODE_API
    return TRUE;
}

#undef OB_NODE_API_IMPORTS
#undef OB_IMPORT_PREFIX
