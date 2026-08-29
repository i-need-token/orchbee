import { z } from 'zod';

import {
	SemanticCommandCoordinatorGenerationSchema,
	SemanticCommandExecutionIdentitySchema,
	SemanticCommandOperationIdSchema,
	SemanticCommandProtocolVersionSchema,
	SemanticCommandRequestIdentitySchema,
	SemanticCommandSchemaVersionSchema,
} from './identity-contract';
import { SemanticCommandJsonValueSchema } from './json-value';

export const SemanticCommandResultCodeSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/u)
	.brand<'SemanticCommandResultCode'>();

const resultWithOutputShape = {
	code: SemanticCommandResultCodeSchema,
	output: SemanticCommandJsonValueSchema,
};
const resultWithoutOutputShape = {
	code: SemanticCommandResultCodeSchema,
};

export const SemanticCommandTerminalResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('completed'), ...resultWithOutputShape }),
	z.strictObject({ kind: z.literal('unchanged'), ...resultWithOutputShape }),
	z.strictObject({ kind: z.literal('blocked'), ...resultWithoutOutputShape }),
	z.strictObject({ kind: z.literal('cancelled'), ...resultWithoutOutputShape }),
	z.strictObject({ kind: z.literal('failed'), ...resultWithoutOutputShape }),
	z.strictObject({ kind: z.literal('unsupported'), ...resultWithoutOutputShape }),
]);

const executionIdentityShape = {
	protocolVersion: SemanticCommandProtocolVersionSchema,
	coordinatorGeneration: SemanticCommandCoordinatorGenerationSchema,
	executionIdentity: SemanticCommandExecutionIdentitySchema,
	requestIdentity: SemanticCommandRequestIdentitySchema,
	operationId: SemanticCommandOperationIdSchema,
	operationSchemaVersion: SemanticCommandSchemaVersionSchema,
};

const activeExecutionSnapshotSchema = z.strictObject({
	...executionIdentityShape,
	phase: z.literal('active'),
	cancellationRequested: z.boolean(),
});

const terminalExecutionSnapshotSchema = z.strictObject({
	...executionIdentityShape,
	phase: z.literal('terminal'),
	result: SemanticCommandTerminalResultSchema,
});

export const SemanticCommandExecutionSnapshotSchema = z.discriminatedUnion('phase', [
	activeExecutionSnapshotSchema,
	terminalExecutionSnapshotSchema,
]);

export const SemanticCommandInvocationRequestSchema = z.strictObject({
	protocolVersion: SemanticCommandProtocolVersionSchema,
	requestIdentity: SemanticCommandRequestIdentitySchema,
	operationId: SemanticCommandOperationIdSchema,
	operationSchemaVersion: SemanticCommandSchemaVersionSchema,
	input: SemanticCommandJsonValueSchema,
});

export const SemanticCommandInvocationRejectionCodeSchema = z.enum([
	'invalid-request',
	'operation-unavailable',
	'operation-version-unsupported',
	'input-invalid',
	'caller-unauthorized',
	'operation-confirmation-required',
	'request-identity-conflict',
	'execution-capacity-exhausted',
	'coordinator-unavailable',
]);

const invocationAcceptedSchema = z.strictObject({
	kind: z.literal('accepted'),
	replayed: z.boolean(),
	execution: SemanticCommandExecutionSnapshotSchema,
});

const invocationRejectedSchema = z.strictObject({
	protocolVersion: SemanticCommandProtocolVersionSchema,
	coordinatorGeneration: SemanticCommandCoordinatorGenerationSchema,
	kind: z.literal('rejected'),
	requestIdentity: SemanticCommandRequestIdentitySchema,
	code: SemanticCommandInvocationRejectionCodeSchema,
});

export const SemanticCommandInvocationResultSchema = z.discriminatedUnion('kind', [
	invocationAcceptedSchema,
	invocationRejectedSchema,
]);

export const SemanticCommandExecutionReferenceSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('request'),
		requestIdentity: SemanticCommandRequestIdentitySchema,
	}),
	z.strictObject({
		kind: z.literal('execution'),
		executionIdentity: SemanticCommandExecutionIdentitySchema,
	}),
]);

const executionFoundSchema = z.strictObject({
	kind: z.literal('found'),
	execution: SemanticCommandExecutionSnapshotSchema,
});

const executionReceiptUnavailableSchema = z.strictObject({
	protocolVersion: SemanticCommandProtocolVersionSchema,
	coordinatorGeneration: SemanticCommandCoordinatorGenerationSchema,
	kind: z.literal('execution-receipt-unavailable'),
	reference: SemanticCommandExecutionReferenceSchema,
});

export const SemanticCommandExecutionLookupResultSchema = z.discriminatedUnion('kind', [
	executionFoundSchema,
	executionReceiptUnavailableSchema,
]);

const executionCancellationObservedSchema = z.strictObject({
	kind: z.literal('cancellation-observed'),
	forwarded: z.boolean(),
	execution: SemanticCommandExecutionSnapshotSchema,
});

export const SemanticCommandExecutionCancellationResultSchema = z.discriminatedUnion('kind', [
	executionCancellationObservedSchema,
	executionReceiptUnavailableSchema,
]);

export type SemanticCommandExecutionCancellationResult = z.infer<
	typeof SemanticCommandExecutionCancellationResultSchema
>;
export type SemanticCommandExecutionLookupResult = z.infer<
	typeof SemanticCommandExecutionLookupResultSchema
>;
export type SemanticCommandExecutionReference = z.infer<
	typeof SemanticCommandExecutionReferenceSchema
>;
export type SemanticCommandExecutionSnapshot = z.infer<
	typeof SemanticCommandExecutionSnapshotSchema
>;
export type SemanticCommandInvocationRejectionCode = z.infer<
	typeof SemanticCommandInvocationRejectionCodeSchema
>;
export type SemanticCommandInvocationRequest = z.infer<
	typeof SemanticCommandInvocationRequestSchema
>;
export type SemanticCommandInvocationResult = z.infer<typeof SemanticCommandInvocationResultSchema>;
export type SemanticCommandResultCode = z.infer<typeof SemanticCommandResultCodeSchema>;
export type SemanticCommandTerminalResult = z.infer<typeof SemanticCommandTerminalResultSchema>;
