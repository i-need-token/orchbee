#include "semantic_command_endpoint.h"

#include <node_api.h>

#include <limits.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#ifndef PATH_MAX
#define PATH_MAX 32768
#endif

#define OB_ENDPOINT_ID_CAPACITY 33
#define OB_MAX_NATIVE_REQUEST_BYTES (80 * 1024)
#define OB_MAX_NATIVE_RESPONSE_BYTES ((1024 + 16) * 1024)

typedef struct ObExchangeTask {
    napi_async_work work;
    napi_deferred deferred;
    int status;
    char runtime_directory[PATH_MAX];
    char endpoint_id[OB_ENDPOINT_ID_CAPACITY];
    uint8_t *request;
    uint32_t request_length;
    uint8_t *response;
    uint32_t response_capacity;
    uint32_t response_length;
    uint32_t timeout_ms;
} ObExchangeTask;

typedef struct ObServerHandle {
    ObSemanticCommandServer *server;
} ObServerHandle;

static const char *status_code(int status) {
    switch (status) {
        case OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT:
            return "ERR_ORCHBEE_SEMANTIC_ENDPOINT_INVALID_ARGUMENT";
        case OB_SEMANTIC_ENDPOINT_INSECURE:
            return "ERR_ORCHBEE_SEMANTIC_ENDPOINT_INSECURE";
        case OB_SEMANTIC_ENDPOINT_UNAVAILABLE:
            return "ERR_ORCHBEE_SEMANTIC_ENDPOINT_UNAVAILABLE";
        case OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_MISMATCH:
            return "ERR_ORCHBEE_SEMANTIC_ENDPOINT_IDENTITY_MISMATCH";
        case OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_UNAVAILABLE:
            return "ERR_ORCHBEE_SEMANTIC_ENDPOINT_IDENTITY_UNAVAILABLE";
        case OB_SEMANTIC_ENDPOINT_TIMEOUT:
            return "ERR_ORCHBEE_SEMANTIC_ENDPOINT_TIMEOUT";
        case OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED:
            return "ERR_ORCHBEE_SEMANTIC_ENDPOINT_CONNECTION_CLOSED";
        case OB_SEMANTIC_ENDPOINT_CAPACITY_EXCEEDED:
            return "ERR_ORCHBEE_SEMANTIC_ENDPOINT_CAPACITY_EXCEEDED";
        default:
            return "ERR_ORCHBEE_SEMANTIC_ENDPOINT_SYSTEM";
    }
}

static const char *status_message(int status) {
    switch (status) {
        case OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT:
            return "Semantic command endpoint input is invalid.";
        case OB_SEMANTIC_ENDPOINT_INSECURE:
            return "Semantic command endpoint ownership is insecure.";
        case OB_SEMANTIC_ENDPOINT_UNAVAILABLE:
            return "Semantic command endpoint is unavailable.";
        case OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_MISMATCH:
            return "Semantic command endpoint peer identity does not match.";
        case OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_UNAVAILABLE:
            return "Semantic command endpoint peer identity is unavailable.";
        case OB_SEMANTIC_ENDPOINT_TIMEOUT:
            return "Semantic command endpoint operation timed out.";
        case OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED:
            return "Semantic command endpoint connection closed.";
        case OB_SEMANTIC_ENDPOINT_CAPACITY_EXCEEDED:
            return "Semantic command endpoint capacity was exceeded.";
        default:
            return "Semantic command endpoint operation failed.";
    }
}

static napi_value create_status_error(napi_env environment, int status) {
    napi_value code;
    napi_value error;
    napi_value message;
    if (napi_create_string_utf8(
            environment,
            status_message(status),
            NAPI_AUTO_LENGTH,
            &message) != napi_ok ||
        napi_create_error(environment, NULL, message, &error) != napi_ok ||
        napi_create_string_utf8(
            environment,
            status_code(status),
            NAPI_AUTO_LENGTH,
            &code) != napi_ok ||
        napi_set_named_property(environment, error, "code", code) != napi_ok) {
        return NULL;
    }
    return error;
}

static napi_value throw_status(napi_env environment, int status) {
    napi_value error = create_status_error(environment, status);
    if (error == NULL) {
        napi_throw_error(
            environment,
            "ERR_ORCHBEE_SEMANTIC_ENDPOINT_SYSTEM",
            "Semantic command endpoint operation failed.");
        return NULL;
    }
    napi_throw(environment, error);
    return NULL;
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

static int read_utf8(
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

static int read_uint32(napi_env environment, napi_value value, uint32_t *result) {
    return napi_get_value_uint32(environment, value, result) == napi_ok;
}

static int read_buffer(
    napi_env environment,
    napi_value value,
    uint8_t **data,
    size_t *length) {
    bool is_buffer = false;
    return napi_is_buffer(environment, value, &is_buffer) == napi_ok && is_buffer &&
        napi_get_buffer_info(environment, value, (void **)data, length) == napi_ok;
}

static int read_server(
    napi_env environment,
    napi_value value,
    ObSemanticCommandServer **server) {
    ObServerHandle *handle = NULL;
    if (napi_get_value_external(environment, value, (void **)&handle) != napi_ok ||
        handle == NULL || handle->server == NULL) {
        return 0;
    }
    *server = handle->server;
    return 1;
}

static napi_value undefined_value(napi_env environment) {
    napi_value value;
    return napi_get_undefined(environment, &value) == napi_ok ? value : NULL;
}

static void finalize_server(
    napi_env environment,
    void *data,
    void *hint) {
    (void)environment;
    (void)hint;
    ObServerHandle *handle = data;
    if (handle == NULL) return;
    ob_semantic_endpoint_server_destroy(handle->server);
    free(handle);
}

static napi_value prepare_runtime_directory(
    napi_env environment,
    napi_callback_info information) {
    napi_value arguments[1];
    char runtime_directory[PATH_MAX];
    if (!read_arguments(environment, information, 1, arguments) ||
        !read_utf8(environment, arguments[0], runtime_directory, sizeof(runtime_directory))) {
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT);
    }
    const int status = ob_semantic_endpoint_prepare_runtime_directory(runtime_directory);
    return status == OB_SEMANTIC_ENDPOINT_OK ? undefined_value(environment)
                                             : throw_status(environment, status);
}

static napi_value create_server(napi_env environment, napi_callback_info information) {
    napi_value arguments[2];
    char runtime_directory[PATH_MAX];
    char endpoint_id[OB_ENDPOINT_ID_CAPACITY];
    if (!read_arguments(environment, information, 2, arguments) ||
        !read_utf8(environment, arguments[0], runtime_directory, sizeof(runtime_directory)) ||
        !read_utf8(environment, arguments[1], endpoint_id, sizeof(endpoint_id))) {
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT);
    }
    int status = OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
    ObSemanticCommandServer *server = ob_semantic_endpoint_server_create(
        runtime_directory,
        endpoint_id,
        &status);
    if (server == NULL) return throw_status(environment, status);
    ObServerHandle *handle = calloc(1, sizeof(*handle));
    if (handle == NULL) {
        ob_semantic_endpoint_server_destroy(server);
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR);
    }
    handle->server = server;
    napi_value external;
    if (napi_create_external(
            environment,
            handle,
            finalize_server,
            NULL,
            &external) != napi_ok) {
        ob_semantic_endpoint_server_destroy(server);
        free(handle);
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR);
    }
    return external;
}

static napi_value close_server(napi_env environment, napi_callback_info information) {
    napi_value arguments[1];
    ObServerHandle *handle = NULL;
    if (!read_arguments(environment, information, 1, arguments) ||
        napi_get_value_external(environment, arguments[0], (void **)&handle) != napi_ok ||
        handle == NULL) {
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT);
    }
    ob_semantic_endpoint_server_destroy(handle->server);
    handle->server = NULL;
    return undefined_value(environment);
}

static napi_value accept_before(napi_env environment, napi_callback_info information) {
    napi_value arguments[3];
    ObSemanticCommandServer *server = NULL;
    uint32_t timeout_ms;
    uint32_t connection_budget_ms;
    if (!read_arguments(environment, information, 3, arguments) ||
        !read_server(environment, arguments[0], &server) ||
        !read_uint32(environment, arguments[1], &timeout_ms) ||
        !read_uint32(environment, arguments[2], &connection_budget_ms) || connection_budget_ms == 0) {
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT);
    }
    uint32_t remaining_request_ms = 0;
    const int status = ob_semantic_endpoint_server_accept_before(server, timeout_ms, connection_budget_ms, &remaining_request_ms);
    napi_value result;
    if (status == OB_SEMANTIC_ENDPOINT_TIMEOUT) {
        return napi_get_null(environment, &result) == napi_ok ? result : NULL;
    }
    if (status != OB_SEMANTIC_ENDPOINT_OK) return throw_status(environment, status);
    return napi_create_uint32(environment, remaining_request_ms, &result) == napi_ok ? result : NULL;
}

static napi_value read_before(napi_env environment, napi_callback_info information) {
    napi_value arguments[3];
    ObSemanticCommandServer *server = NULL;
    uint32_t capacity;
    uint32_t timeout_ms;
    if (!read_arguments(environment, information, 3, arguments) ||
        !read_server(environment, arguments[0], &server) ||
        !read_uint32(environment, arguments[1], &capacity) || capacity == 0 ||
        capacity > OB_MAX_NATIVE_REQUEST_BYTES ||
        !read_uint32(environment, arguments[2], &timeout_ms)) {
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT);
    }
    uint8_t *buffer = malloc(capacity);
    if (buffer == NULL) return throw_status(environment, OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR);
    uint32_t read_bytes = 0;
    const int status = ob_semantic_endpoint_server_read_before(
        server,
        buffer,
        capacity,
        timeout_ms,
        &read_bytes);
    if (status == OB_SEMANTIC_ENDPOINT_TIMEOUT) {
        free(buffer);
        napi_value result;
        return napi_get_null(environment, &result) == napi_ok ? result : NULL;
    }
    if (status != OB_SEMANTIC_ENDPOINT_OK) {
        free(buffer);
        return throw_status(environment, status);
    }
    napi_value result;
    const napi_status copy_status = napi_create_buffer_copy(
        environment,
        read_bytes,
        buffer,
        NULL,
        &result);
    free(buffer);
    return copy_status == napi_ok ? result
                                  : throw_status(environment, OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR);
}

static napi_value write_before(napi_env environment, napi_callback_info information) {
    napi_value arguments[3];
    ObSemanticCommandServer *server = NULL;
    uint8_t *buffer = NULL;
    size_t length = 0;
    uint32_t timeout_ms;
    if (!read_arguments(environment, information, 3, arguments) ||
        !read_server(environment, arguments[0], &server) ||
        !read_buffer(environment, arguments[1], &buffer, &length) || length == 0 ||
        length > OB_MAX_NATIVE_RESPONSE_BYTES ||
        !read_uint32(environment, arguments[2], &timeout_ms)) {
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT);
    }
    uint32_t written_bytes = 0;
    const int status = ob_semantic_endpoint_server_write_before(
        server,
        buffer,
        (uint32_t)length,
        timeout_ms,
        &written_bytes);
    if (status != OB_SEMANTIC_ENDPOINT_OK) return throw_status(environment, status);
    napi_value result;
    return napi_create_uint32(environment, written_bytes, &result) == napi_ok
        ? result
        : throw_status(environment, OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR);
}

static napi_value disconnect_server(napi_env environment, napi_callback_info information) {
    napi_value arguments[1];
    ObSemanticCommandServer *server = NULL;
    if (!read_arguments(environment, information, 1, arguments) ||
        !read_server(environment, arguments[0], &server)) {
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT);
    }
    ob_semantic_endpoint_server_disconnect(server);
    return undefined_value(environment);
}

static napi_value publish_locator(napi_env environment, napi_callback_info information) {
    napi_value arguments[3];
    char runtime_directory[PATH_MAX];
    char endpoint_id[OB_ENDPOINT_ID_CAPACITY];
    uint8_t *content = NULL;
    size_t content_length = 0;
    if (!read_arguments(environment, information, 3, arguments) ||
        !read_utf8(environment, arguments[0], runtime_directory, sizeof(runtime_directory)) ||
        !read_utf8(environment, arguments[1], endpoint_id, sizeof(endpoint_id)) ||
        !read_buffer(environment, arguments[2], &content, &content_length) ||
        content_length == 0 || content_length > 4096) {
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT);
    }
    const int status = ob_semantic_endpoint_publish_locator(
        runtime_directory,
        endpoint_id,
        content,
        (uint32_t)content_length);
    return status == OB_SEMANTIC_ENDPOINT_OK ? undefined_value(environment)
                                             : throw_status(environment, status);
}

static napi_value read_locator(napi_env environment, napi_callback_info information) {
    napi_value arguments[2];
    char runtime_directory[PATH_MAX];
    char endpoint_id[OB_ENDPOINT_ID_CAPACITY];
    if (!read_arguments(environment, information, 2, arguments) ||
        !read_utf8(environment, arguments[0], runtime_directory, sizeof(runtime_directory)) ||
        !read_utf8(environment, arguments[1], endpoint_id, sizeof(endpoint_id))) {
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT);
    }
    uint8_t buffer[4096];
    uint32_t read_bytes = 0;
    const int status = ob_semantic_endpoint_read_locator(
        runtime_directory,
        endpoint_id,
        buffer,
        sizeof(buffer),
        &read_bytes);
    if (status != OB_SEMANTIC_ENDPOINT_OK) return throw_status(environment, status);
    napi_value result;
    return napi_create_buffer_copy(environment, read_bytes, buffer, NULL, &result) == napi_ok
        ? result
        : throw_status(environment, OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR);
}

static napi_value remove_locator(napi_env environment, napi_callback_info information) {
    napi_value arguments[2];
    char runtime_directory[PATH_MAX];
    char endpoint_id[OB_ENDPOINT_ID_CAPACITY];
    if (!read_arguments(environment, information, 2, arguments) ||
        !read_utf8(environment, arguments[0], runtime_directory, sizeof(runtime_directory)) ||
        !read_utf8(environment, arguments[1], endpoint_id, sizeof(endpoint_id))) {
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT);
    }
    const int status = ob_semantic_endpoint_remove_locator(runtime_directory, endpoint_id);
    return status == OB_SEMANTIC_ENDPOINT_OK ? undefined_value(environment)
                                             : throw_status(environment, status);
}

static void execute_exchange(napi_env environment, void *data) {
    (void)environment;
    ObExchangeTask *task = data;
    task->status = ob_semantic_endpoint_client_exchange(
        task->runtime_directory,
        task->endpoint_id,
        task->request,
        task->request_length,
        task->response,
        task->response_capacity,
        task->timeout_ms,
        &task->response_length);
}

static void complete_exchange(napi_env environment, napi_status async_status, void *data) {
    ObExchangeTask *task = data;
    if (async_status == napi_ok && task->status == OB_SEMANTIC_ENDPOINT_OK) {
        napi_value response;
        if (napi_create_buffer_copy(
                environment,
                task->response_length,
                task->response,
                NULL,
                &response) == napi_ok) {
            (void)napi_resolve_deferred(environment, task->deferred, response);
        } else {
            napi_value error = create_status_error(
                environment,
                OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR);
            if (error != NULL) (void)napi_reject_deferred(environment, task->deferred, error);
        }
    } else {
        const int status = async_status == napi_ok ? task->status
                                                   : OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR;
        napi_value error = create_status_error(environment, status);
        if (error != NULL) (void)napi_reject_deferred(environment, task->deferred, error);
    }
    (void)napi_delete_async_work(environment, task->work);
    free(task->request);
    free(task->response);
    free(task);
}

static napi_value exchange_authenticated(napi_env environment, napi_callback_info information) {
    napi_value arguments[5];
    char runtime_directory[PATH_MAX];
    char endpoint_id[OB_ENDPOINT_ID_CAPACITY];
    uint8_t *request = NULL;
    size_t request_length = 0;
    uint32_t response_capacity;
    uint32_t timeout_ms;
    if (!read_arguments(environment, information, 5, arguments) ||
        !read_utf8(environment, arguments[0], runtime_directory, sizeof(runtime_directory)) ||
        !read_utf8(environment, arguments[1], endpoint_id, sizeof(endpoint_id)) ||
        !read_buffer(environment, arguments[2], &request, &request_length) ||
        request_length == 0 || request_length > OB_MAX_NATIVE_REQUEST_BYTES ||
        !read_uint32(environment, arguments[3], &response_capacity) || response_capacity == 0 ||
        response_capacity > OB_MAX_NATIVE_RESPONSE_BYTES ||
        !read_uint32(environment, arguments[4], &timeout_ms) || timeout_ms == 0) {
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT);
    }
    ObExchangeTask *task = calloc(1, sizeof(*task));
    if (task == NULL) return throw_status(environment, OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR);
    memcpy(task->runtime_directory, runtime_directory, strlen(runtime_directory) + 1);
    memcpy(task->endpoint_id, endpoint_id, strlen(endpoint_id) + 1);
    task->request = malloc(request_length);
    task->response = malloc(response_capacity);
    if (task->request == NULL || task->response == NULL) {
        free(task->request);
        free(task->response);
        free(task);
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR);
    }
    memcpy(task->request, request, request_length);
    task->request_length = (uint32_t)request_length;
    task->response_capacity = response_capacity;
    task->timeout_ms = timeout_ms;
    napi_value promise;
    napi_value resource_name;
    if (napi_create_promise(environment, &task->deferred, &promise) != napi_ok ||
        napi_create_string_utf8(
            environment,
            "orchbee-semantic-command-exchange",
            NAPI_AUTO_LENGTH,
            &resource_name) != napi_ok ||
        napi_create_async_work(
            environment,
            NULL,
            resource_name,
            execute_exchange,
            complete_exchange,
            task,
            &task->work) != napi_ok ||
        napi_queue_async_work(environment, task->work) != napi_ok) {
        if (task->work != NULL) (void)napi_delete_async_work(environment, task->work);
        free(task->request);
        free(task->response);
        free(task);
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR);
    }
    return promise;
}

static napi_value initialize(napi_env environment, napi_value exports) {
    const napi_property_descriptor properties[] = {
        {"acceptBefore", NULL, accept_before, NULL, NULL, NULL, napi_default, NULL},
        {"closeServer", NULL, close_server, NULL, NULL, NULL, napi_default, NULL},
        {"createServer", NULL, create_server, NULL, NULL, NULL, napi_default, NULL},
        {"disconnectServer", NULL, disconnect_server, NULL, NULL, NULL, napi_default, NULL},
        {"exchangeAuthenticated", NULL, exchange_authenticated, NULL, NULL, NULL, napi_default, NULL},
        {"prepareRuntimeDirectory", NULL, prepare_runtime_directory, NULL, NULL, NULL, napi_default, NULL},
        {"publishLocator", NULL, publish_locator, NULL, NULL, NULL, napi_default, NULL},
        {"readBefore", NULL, read_before, NULL, NULL, NULL, napi_default, NULL},
        {"readLocator", NULL, read_locator, NULL, NULL, NULL, napi_default, NULL},
        {"removeLocator", NULL, remove_locator, NULL, NULL, NULL, napi_default, NULL},
        {"writeBefore", NULL, write_before, NULL, NULL, NULL, napi_default, NULL},
    };
    if (napi_define_properties(
            environment,
            exports,
            sizeof(properties) / sizeof(properties[0]),
            properties) != napi_ok) {
        return throw_status(environment, OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR);
    }
    return exports;
}

NAPI_MODULE(NODE_GYP_MODULE_NAME, initialize)
