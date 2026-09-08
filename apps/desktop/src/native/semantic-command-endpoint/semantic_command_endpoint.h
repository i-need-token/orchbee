#ifndef ORCHBEE_SEMANTIC_COMMAND_ENDPOINT_H
#define ORCHBEE_SEMANTIC_COMMAND_ENDPOINT_H

#include <stddef.h>
#include <stdint.h>

enum ob_semantic_endpoint_status {
    OB_SEMANTIC_ENDPOINT_OK = 0,
    OB_SEMANTIC_ENDPOINT_INVALID_ARGUMENT = 1,
    OB_SEMANTIC_ENDPOINT_SYSTEM_ERROR = 2,
    OB_SEMANTIC_ENDPOINT_INSECURE = 3,
    OB_SEMANTIC_ENDPOINT_UNAVAILABLE = 4,
    OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_MISMATCH = 5,
    OB_SEMANTIC_ENDPOINT_PEER_IDENTITY_UNAVAILABLE = 6,
    OB_SEMANTIC_ENDPOINT_TIMEOUT = 7,
    OB_SEMANTIC_ENDPOINT_CONNECTION_CLOSED = 8,
    OB_SEMANTIC_ENDPOINT_CAPACITY_EXCEEDED = 9,
};

typedef struct ObSemanticCommandServer ObSemanticCommandServer;

int ob_semantic_endpoint_prepare_runtime_directory(const char *runtime_directory);

ObSemanticCommandServer *ob_semantic_endpoint_server_create(
    const char *runtime_directory,
    const char *endpoint_id,
    int *status);
void ob_semantic_endpoint_server_destroy(ObSemanticCommandServer *server);
int ob_semantic_endpoint_server_accept_before(
    ObSemanticCommandServer *server,
    uint32_t timeout_ms,
    uint32_t connection_budget_ms,
    uint32_t *remaining_request_ms);
int ob_semantic_endpoint_server_read_before(
    ObSemanticCommandServer *server,
    uint8_t *buffer,
    uint32_t capacity,
    uint32_t timeout_ms,
    uint32_t *read_bytes);
int ob_semantic_endpoint_server_write_before(
    ObSemanticCommandServer *server,
    const uint8_t *buffer,
    uint32_t length,
    uint32_t timeout_ms,
    uint32_t *written_bytes);
void ob_semantic_endpoint_server_disconnect(ObSemanticCommandServer *server);

int ob_semantic_endpoint_publish_locator(
    const char *runtime_directory,
    const char *endpoint_id,
    const uint8_t *content,
    uint32_t content_length);
int ob_semantic_endpoint_read_locator(
    const char *runtime_directory,
    const char *endpoint_id,
    uint8_t *buffer,
    uint32_t capacity,
    uint32_t *read_bytes);
int ob_semantic_endpoint_remove_locator(
    const char *runtime_directory,
    const char *endpoint_id);

int ob_semantic_endpoint_client_exchange(
    const char *runtime_directory,
    const char *endpoint_id,
    const uint8_t *request,
    uint32_t request_length,
    uint8_t *response,
    uint32_t response_capacity,
    uint32_t timeout_ms,
    uint32_t *response_length);

#endif
