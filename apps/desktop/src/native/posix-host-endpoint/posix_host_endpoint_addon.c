#define _POSIX_C_SOURCE 200809L

#include "posix_host_endpoint.h"

#include <node_api.h>

#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <sys/types.h>
#include <unistd.h>

static napi_value throw_endpoint_error(
    napi_env environment,
    const char *code,
    const char *message) {
    napi_value code_value;
    napi_value error;
    napi_value message_value;
    if (napi_create_string_utf8(environment, message, NAPI_AUTO_LENGTH, &message_value) != napi_ok ||
        napi_create_error(environment, NULL, message_value, &error) != napi_ok ||
        napi_create_string_utf8(environment, code, NAPI_AUTO_LENGTH, &code_value) != napi_ok ||
        napi_set_named_property(environment, error, "code", code_value) != napi_ok) {
        napi_throw_error(environment, "ERR_ORCHBEE_POSIX_ENDPOINT", message);
        return NULL;
    }
    napi_throw(environment, error);
    return NULL;
}

static napi_value throw_status(napi_env environment, int status) {
    switch (status) {
        case OB_DESKTOP_POSIX_ENDPOINT_INVALID_ARGUMENT:
            return throw_endpoint_error(
                environment,
                "ERR_ORCHBEE_POSIX_ENDPOINT_INVALID_ARGUMENT",
                "POSIX Terminal Host endpoint input is invalid.");
        case OB_DESKTOP_POSIX_ENDPOINT_INSECURE:
            return throw_endpoint_error(
                environment,
                "ERR_ORCHBEE_POSIX_ENDPOINT_INSECURE",
                "POSIX Terminal Host runtime directory or endpoint is insecure.");
        case OB_DESKTOP_POSIX_ENDPOINT_UNAVAILABLE:
            return throw_endpoint_error(
                environment,
                "ERR_ORCHBEE_POSIX_ENDPOINT_UNAVAILABLE",
                "POSIX Terminal Host endpoint is unavailable.");
        case OB_DESKTOP_POSIX_ENDPOINT_PEER_IDENTITY_MISMATCH:
            return throw_endpoint_error(
                environment,
                "ERR_ORCHBEE_POSIX_ENDPOINT_IDENTITY_MISMATCH",
                "POSIX Terminal Host peer effective UID does not match Desktop.");
        case OB_DESKTOP_POSIX_ENDPOINT_PEER_IDENTITY_UNAVAILABLE:
            return throw_endpoint_error(
                environment,
                "ERR_ORCHBEE_POSIX_ENDPOINT_IDENTITY_UNAVAILABLE",
                "POSIX Terminal Host peer credentials are unavailable.");
        default:
            return throw_endpoint_error(
                environment,
                "ERR_ORCHBEE_POSIX_ENDPOINT_SYSTEM",
                "POSIX Terminal Host endpoint authentication failed.");
    }
}

static int read_utf8_argument(
    napi_env environment,
    napi_value value,
    char *buffer,
    size_t capacity) {
    size_t length = 0;
    if (napi_get_value_string_utf8(environment, value, NULL, 0, &length) != napi_ok ||
        length == 0 || length >= capacity) {
        return 0;
    }
    size_t copied = 0;
    return napi_get_value_string_utf8(
               environment,
               value,
               buffer,
               capacity,
               &copied) == napi_ok &&
        copied == length;
}

static int read_arguments(
    napi_env environment,
    napi_callback_info information,
    size_t expected_count,
    napi_value *arguments) {
    size_t count = expected_count;
    return napi_get_cb_info(
               environment,
               information,
               &count,
               arguments,
               NULL,
               NULL) == napi_ok &&
        count == expected_count;
}

static int current_effective_uid(uint32_t *uid) {
    if (uid == NULL) return 0;
    const uid_t current = geteuid();
    if ((uintmax_t)current > UINT32_MAX) return 0;
    *uid = (uint32_t)current;
    return 1;
}

static napi_value prepare_runtime_directory(
    napi_env environment,
    napi_callback_info information) {
    napi_value arguments[1];
    char runtime_directory[PATH_MAX];
    uint32_t expected_uid;
    if (!read_arguments(environment, information, 1, arguments) ||
        !read_utf8_argument(
            environment,
            arguments[0],
            runtime_directory,
            sizeof(runtime_directory)) ||
        !current_effective_uid(&expected_uid)) {
        return throw_status(environment, OB_DESKTOP_POSIX_ENDPOINT_INVALID_ARGUMENT);
    }
    const int status = ob_desktop_prepare_runtime_directory(
        runtime_directory,
        expected_uid);
    if (status != OB_DESKTOP_POSIX_ENDPOINT_OK) return throw_status(environment, status);

    napi_value undefined_value;
    if (napi_get_undefined(environment, &undefined_value) != napi_ok) {
        return throw_status(environment, OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR);
    }
    return undefined_value;
}

static napi_value connect_authenticated(
    napi_env environment,
    napi_callback_info information) {
    napi_value arguments[2];
    char runtime_directory[PATH_MAX];
    char endpoint_id[33];
    uint32_t expected_uid;
    if (!read_arguments(environment, information, 2, arguments) ||
        !read_utf8_argument(
            environment,
            arguments[0],
            runtime_directory,
            sizeof(runtime_directory)) ||
        !read_utf8_argument(environment, arguments[1], endpoint_id, sizeof(endpoint_id)) ||
        !current_effective_uid(&expected_uid)) {
        return throw_status(environment, OB_DESKTOP_POSIX_ENDPOINT_INVALID_ARGUMENT);
    }

    int connected_fd = -1;
    const int status = ob_desktop_connect_authenticated(
        runtime_directory,
        endpoint_id,
        expected_uid,
        &connected_fd);
    if (status != OB_DESKTOP_POSIX_ENDPOINT_OK) return throw_status(environment, status);

    napi_value descriptor_value;
    if (napi_create_int32(environment, connected_fd, &descriptor_value) != napi_ok) {
        close(connected_fd);
        return throw_status(environment, OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR);
    }
    return descriptor_value;
}

static napi_value initialize(napi_env environment, napi_value exports) {
    const napi_property_descriptor properties[] = {
        {
            "connectAuthenticated",
            NULL,
            connect_authenticated,
            NULL,
            NULL,
            NULL,
            napi_default,
            NULL,
        },
        {
            "prepareRuntimeDirectory",
            NULL,
            prepare_runtime_directory,
            NULL,
            NULL,
            NULL,
            napi_default,
            NULL,
        },
    };
    if (napi_define_properties(
            environment,
            exports,
            sizeof(properties) / sizeof(properties[0]),
            properties) != napi_ok) {
        return throw_status(environment, OB_DESKTOP_POSIX_ENDPOINT_SYSTEM_ERROR);
    }
    return exports;
}

NAPI_MODULE(NODE_GYP_MODULE_NAME, initialize)
