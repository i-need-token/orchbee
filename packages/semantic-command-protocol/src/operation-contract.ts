import { z } from 'zod';

import {
	SemanticCommandCoordinatorGenerationSchema,
	SemanticCommandOperationIdSchema,
	SemanticCommandProtocolVersionSchema,
	SemanticCommandSchemaVersionSchema,
} from './identity-contract';

export const SemanticCommandOperationKindSchema = z.enum(['query', 'command']);
export const SemanticCommandConfirmationPolicySchema = z.enum([
	'noninteractive-allowed',
	'explicit-acknowledgement',
	'interactive-only',
]);
export const SemanticCommandRetryPolicySchema = z.enum(['never', 'same-request-while-retained']);

export const SemanticCommandContractReferenceSchema = z.strictObject({
	name: z
		.string()
		.min(3)
		.max(128)
		.regex(/^[a-z][a-z0-9-]*(?:\.[a-z][a-z0-9-]*)+$/u),
	schemaVersion: SemanticCommandSchemaVersionSchema,
});

export const SemanticCommandOperationDescriptorSchema = z.strictObject({
	id: SemanticCommandOperationIdSchema,
	schemaVersion: SemanticCommandSchemaVersionSchema,
	kind: SemanticCommandOperationKindSchema,
	confirmation: SemanticCommandConfirmationPolicySchema,
	retry: SemanticCommandRetryPolicySchema,
	input: SemanticCommandContractReferenceSchema,
	output: SemanticCommandContractReferenceSchema,
});

export const SemanticCommandCoordinatorStatusSchema = z.strictObject({
	protocolVersion: SemanticCommandProtocolVersionSchema,
	coordinatorGeneration: SemanticCommandCoordinatorGenerationSchema,
	kind: z.literal('ready'),
});

export const SemanticCommandOperationListSchema = z.strictObject({
	protocolVersion: SemanticCommandProtocolVersionSchema,
	coordinatorGeneration: SemanticCommandCoordinatorGenerationSchema,
	kind: z.literal('operation-list'),
	operations: z.array(SemanticCommandOperationDescriptorSchema),
});

const operationDescriptionSchema = z.strictObject({
	protocolVersion: SemanticCommandProtocolVersionSchema,
	coordinatorGeneration: SemanticCommandCoordinatorGenerationSchema,
	kind: z.literal('operation-description'),
	operation: SemanticCommandOperationDescriptorSchema,
});

const operationUnavailableSchema = z.strictObject({
	protocolVersion: SemanticCommandProtocolVersionSchema,
	coordinatorGeneration: SemanticCommandCoordinatorGenerationSchema,
	kind: z.literal('operation-unavailable'),
	operationId: SemanticCommandOperationIdSchema,
});

export const SemanticCommandOperationDescriptionResultSchema = z.discriminatedUnion('kind', [
	operationDescriptionSchema,
	operationUnavailableSchema,
]);

export type SemanticCommandConfirmationPolicy = z.infer<
	typeof SemanticCommandConfirmationPolicySchema
>;
export type SemanticCommandContractReference = z.infer<
	typeof SemanticCommandContractReferenceSchema
>;
export type SemanticCommandCoordinatorStatus = z.infer<
	typeof SemanticCommandCoordinatorStatusSchema
>;
export type SemanticCommandOperationDescriptionResult = z.infer<
	typeof SemanticCommandOperationDescriptionResultSchema
>;
export type SemanticCommandOperationDescriptor = z.infer<
	typeof SemanticCommandOperationDescriptorSchema
>;
export type SemanticCommandOperationKind = z.infer<typeof SemanticCommandOperationKindSchema>;
export type SemanticCommandOperationList = z.infer<typeof SemanticCommandOperationListSchema>;
export type SemanticCommandRetryPolicy = z.infer<typeof SemanticCommandRetryPolicySchema>;
