import { z } from 'zod';

const serializableIdentitySchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9][A-Za-z0-9:._-]*$/u);

export const SEMANTIC_COMMAND_PROTOCOL_VERSION = 1 as const;
export const SemanticCommandProtocolVersionSchema = z.literal(SEMANTIC_COMMAND_PROTOCOL_VERSION);

export const SemanticCommandRequestIdentitySchema =
	serializableIdentitySchema.brand<'SemanticCommandRequestIdentity'>();
export const SemanticCommandExecutionIdentitySchema =
	serializableIdentitySchema.brand<'SemanticCommandExecutionIdentity'>();
export const SemanticCommandCoordinatorGenerationSchema =
	serializableIdentitySchema.brand<'SemanticCommandCoordinatorGeneration'>();

export const SemanticCommandOperationIdSchema = z
	.string()
	.min(5)
	.max(128)
	.regex(/^[a-z][a-z0-9-]*(?:\.[a-z][a-z0-9-]*){2,}$/u)
	.brand<'SemanticCommandOperationId'>();

export const SemanticCommandSchemaVersionSchema = z
	.number()
	.int()
	.positive()
	.max(2_147_483_647)
	.brand<'SemanticCommandSchemaVersion'>();

export type SemanticCommandCoordinatorGeneration = z.infer<
	typeof SemanticCommandCoordinatorGenerationSchema
>;
export type SemanticCommandExecutionIdentity = z.infer<
	typeof SemanticCommandExecutionIdentitySchema
>;
export type SemanticCommandOperationId = z.infer<typeof SemanticCommandOperationIdSchema>;
export type SemanticCommandRequestIdentity = z.infer<typeof SemanticCommandRequestIdentitySchema>;
export type SemanticCommandSchemaVersion = z.infer<typeof SemanticCommandSchemaVersionSchema>;
