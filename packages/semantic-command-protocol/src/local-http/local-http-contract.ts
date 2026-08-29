import { z } from 'zod';

import {
	SemanticCommandCoordinatorGenerationSchema,
	SemanticCommandProtocolVersionSchema,
	type SemanticCommandExecutionIdentitySchema,
	type SemanticCommandOperationIdSchema,
	type SemanticCommandRequestIdentitySchema,
} from '../identity-contract';

export const SEMANTIC_COMMAND_LOCAL_HTTP_LOCATOR_VERSION = 1 as const;
export const SEMANTIC_COMMAND_LOCAL_HTTP_MAX_HEADER_BYTES = 16 * 1024;
export const SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES = 64 * 1024;
export const SEMANTIC_COMMAND_LOCAL_HTTP_MAX_RESPONSE_BODY_BYTES = 1024 * 1024;
export const SEMANTIC_COMMAND_LOCAL_HTTP_MAX_LOCATOR_BYTES = 4 * 1024;

export const SemanticCommandLocalEndpointIdSchema = z
	.string()
	.regex(/^[a-f0-9]{32}$/u)
	.brand<'SemanticCommandLocalEndpointId'>();

export const SemanticCommandLocalLocatorSchema = z.strictObject({
	locatorVersion: z.literal(SEMANTIC_COMMAND_LOCAL_HTTP_LOCATOR_VERSION),
	protocolVersion: SemanticCommandProtocolVersionSchema,
	coordinatorGeneration: SemanticCommandCoordinatorGenerationSchema,
	endpointId: SemanticCommandLocalEndpointIdSchema,
});

export const SemanticCommandLocalHttpErrorCodeSchema = z.enum([
	'content-type-required',
	'connection-capacity-exhausted',
	'malformed-request',
	'method-not-allowed',
	'request-capacity-exceeded',
	'request-timeout',
	'response-capacity-exceeded',
	'route-not-found',
]);

export const SemanticCommandLocalHttpErrorSchema = z.strictObject({
	protocolVersion: SemanticCommandProtocolVersionSchema,
	kind: z.literal('local-http-error'),
	code: SemanticCommandLocalHttpErrorCodeSchema,
});

export type SemanticCommandLocalEndpointId = z.infer<typeof SemanticCommandLocalEndpointIdSchema>;
export type SemanticCommandLocalHttpError = z.infer<typeof SemanticCommandLocalHttpErrorSchema>;
export type SemanticCommandLocalHttpErrorCode = z.infer<
	typeof SemanticCommandLocalHttpErrorCodeSchema
>;
export type SemanticCommandLocalLocator = z.infer<typeof SemanticCommandLocalLocatorSchema>;

export type SemanticCommandLocalHttpRoute =
	| { readonly kind: 'status' }
	| { readonly kind: 'operation-list' }
	| {
			readonly kind: 'operation-describe';
			readonly operationId: z.infer<typeof SemanticCommandOperationIdSchema>;
	  }
	| { readonly kind: 'execution-invoke'; readonly body: unknown }
	| {
			readonly kind: 'request-get';
			readonly requestIdentity: z.infer<typeof SemanticCommandRequestIdentitySchema>;
	  }
	| {
			readonly executionIdentity: z.infer<typeof SemanticCommandExecutionIdentitySchema>;
			readonly kind: 'execution-get';
	  }
	| {
			readonly executionIdentity: z.infer<typeof SemanticCommandExecutionIdentitySchema>;
			readonly kind: 'execution-cancel';
	  };
