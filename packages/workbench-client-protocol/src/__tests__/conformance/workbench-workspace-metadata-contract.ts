import { z } from 'zod';
import {
	SiteIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchWorkspaceIdSchema,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_METADATA_RESULT_BYTES = 128 * 1024;
export const MetadataSourceSchema = z.string().regex(/^[a-zA-Z0-9:._-]{1,80}$(?![\s\S])/u);
const MetadataTokenKeySchema = z.string().regex(/^[a-zA-Z0-9_-]{1,32}$(?![\s\S])/u);
// Rust str::trim uses Unicode White_Space, not ECMAScript's BOM-inclusive trim.
const edgeWhitespace = /^[\p{White_Space}]|[\p{White_Space}]$/u;
export const MetadataTokenValueSchema = z
	.string()
	.min(1)
	.refine(
		(value) =>
			Array.from(value).length <= 80 &&
			!edgeWhitespace.test(value) &&
			!/[\p{Cc}\uD800-\uDFFF]/u.test(value),
		'Metadata value must already be normalized and contain at most 80 Unicode scalars',
	);
// Zod record drops __proto__. Validate entries and create own data properties instead;
// every legal token key, including __proto__ and constructor, must survive decoding.
const tokenEntries = z.unknown().transform((input, context) => {
	if (
		input === null ||
		typeof input !== 'object' ||
		Array.isArray(input) ||
		(Object.getPrototypeOf(input) !== Object.prototype && Object.getPrototypeOf(input) !== null)
	) {
		context.addIssue({ code: 'custom', message: 'Metadata tokens must be an object' });
		return z.NEVER;
	}
	return Object.entries(input);
});
export const MetadataTokenPatchSchema = tokenEntries
	.pipe(
		z
			.array(z.tuple([MetadataTokenKeySchema, MetadataTokenValueSchema.nullable()]))
			.min(1)
			.max(16),
	)
	.transform((entries) => Object.fromEntries(entries))
	.readonly();
export const MetadataTokensSchema = tokenEntries
	.pipe(z.array(z.tuple([MetadataTokenKeySchema, MetadataTokenValueSchema])).max(32))
	.transform((entries) => Object.fromEntries(entries))
	.readonly();
const WorkspaceMetadataActionSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('read'),
			workspaceIds: z
				.array(WorkbenchWorkspaceIdSchema)
				.max(8)
				.refine((ids) => new Set(ids).size === ids.length)
				.readonly(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('report'),
			workspaceId: WorkbenchWorkspaceIdSchema,
			source: MetadataSourceSchema,
			tokens: MetadataTokenPatchSchema,
			sequence: WorkbenchRevisionSchema.nullish().transform((value) => value ?? undefined),
			ttlMilliseconds: z
				.number()
				.int()
				.min(1)
				.max(86_400_000)
				.nullish()
				.transform((value) => value ?? undefined),
		})
		.readonly(),
]);
const WorkspaceMetadataFailureSchema = z.enum([
	'invalid-request',
	'site-mismatch',
	'not-found',
	'denied',
	'unavailable',
	'token-limit',
	'sequence-source-limit',
	'resource-limit',
	'revision-exhausted',
	'deadline-overflow',
	'result-too-large',
]);
const WorkspaceMetadataOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('read'),
			workbenchRevision: WorkbenchRevisionSchema,
			metadataRevision: WorkbenchRevisionSchema,
			workspaces: z
				.array(
					z
						.strictObject({ workspaceId: WorkbenchWorkspaceIdSchema, tokens: MetadataTokensSchema })
						.readonly(),
				)
				.max(8)
				.refine((rows) => new Set(rows.map((row) => row.workspaceId)).size === rows.length)
				.readonly(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('reported'),
			workbenchRevision: WorkbenchRevisionSchema,
			metadataRevision: WorkbenchRevisionSchema,
			workspaceId: WorkbenchWorkspaceIdSchema,
			disposition: z.enum(['applied', 'stale']),
		})
		.readonly(),
	z.strictObject({ kind: z.literal('failed'), reason: WorkspaceMetadataFailureSchema }).readonly(),
]);
export const WorkspaceMetadataRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.workspace-metadata'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		siteId: SiteIdSchema,
		action: WorkspaceMetadataActionSchema,
	})
	.readonly();
export const WorkspaceMetadataResultSchema = z
	.strictObject({
		kind: z.literal('workbench.workspace-metadata.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		siteId: SiteIdSchema,
		outcome: WorkspaceMetadataOutcomeSchema,
	})
	.readonly();
