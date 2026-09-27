import { z } from 'zod';
import {
	SiteIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchPanelIdSchema,
	WorkbenchPresentationBindingSchema,
} from './workbench-semantic-contract';
import {
	MetadataSourceSchema,
	MetadataTokenValueSchema,
	MetadataTokenPatchSchema,
	MetadataTokensSchema,
} from './workbench-workspace-metadata-contract';

export const WORKBENCH_MAXIMUM_ENCODED_PANEL_METADATA_RESULT_BYTES = 512 * 1024;
export const MetadataStateSchema = z.enum(['idle', 'working', 'blocked', 'done', 'unknown']);
const labels = z.partialRecord(MetadataStateSchema, MetadataTokenValueSchema).readonly();
const optionalValue = MetadataTokenValueSchema.nullish().transform((value) => value ?? undefined);
export const PanelMetadataPatchSchema = z
	.strictObject({
		agent: optionalValue,
		appliesToSource: MetadataSourceSchema.nullish().transform((value) => value ?? undefined),
		title: optionalValue,
		displayAgent: optionalValue,
		stateLabels: labels.default({}),
		clearTitle: z.boolean().default(false),
		clearDisplayAgent: z.boolean().default(false),
		clearStateLabels: z.boolean().default(false),
		tokens: MetadataTokenPatchSchema.nullish().transform((value) => value ?? undefined),
	})
	.refine(
		(p) =>
			!(p.clearTitle && p.title !== undefined) &&
			!(p.clearDisplayAgent && p.displayAgent !== undefined) &&
			!(p.clearStateLabels && Object.keys(p.stateLabels).length > 0) &&
			(p.clearTitle ||
				p.clearDisplayAgent ||
				p.clearStateLabels ||
				p.title !== undefined ||
				p.displayAgent !== undefined ||
				Object.keys(p.stateLabels).length > 0 ||
				p.tokens !== undefined),
		'Empty report or conflicting set and clear',
	)
	.readonly();
export const PanelMetadataActionSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('read'),
			panelIds: z
				.array(WorkbenchPanelIdSchema)
				.max(4)
				.refine((ids) => new Set(ids).size === ids.length)
				.readonly(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('report'),
			panelId: WorkbenchPanelIdSchema,
			expectedBinding: WorkbenchPresentationBindingSchema,
			source: MetadataSourceSchema,
			patch: PanelMetadataPatchSchema,
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
export const PanelMetadataFailureSchema = z.enum([
	'invalid-request',
	'site-mismatch',
	'not-found',
	'unbound',
	'binding-changed',
	'denied',
	'unavailable',
	'token-limit',
	'source-limit',
	'resource-limit',
	'revision-exhausted',
	'deadline-overflow',
	'result-too-large',
]);
export const PanelMetadataFieldSchema = z
	.strictObject({
		value: MetadataTokenValueSchema,
		order: WorkbenchRevisionSchema.refine((value) => value !== '0'),
	})
	.readonly();
export const PanelMetadataPresentationSchema = z
	.strictObject({
		source: MetadataSourceSchema,
		agent: MetadataTokenValueSchema.nullable(),
		appliesToSource: MetadataSourceSchema.nullable(),
		title: PanelMetadataFieldSchema.nullable(),
		displayAgent: PanelMetadataFieldSchema.nullable(),
		stateLabels: z.partialRecord(MetadataStateSchema, PanelMetadataFieldSchema).readonly(),
	})
	.refine(
		(p) => p.title !== null || p.displayAgent !== null || Object.keys(p.stateLabels).length > 0,
	)
	.readonly();
export const PanelMetadataRowSchema = z
	.strictObject({
		panelId: WorkbenchPanelIdSchema,
		binding: WorkbenchPresentationBindingSchema.nullable(),
		tokens: MetadataTokensSchema,
		presentations: z
			.array(PanelMetadataPresentationSchema)
			.max(32)
			.refine((rows) => new Set(rows.map((row) => row.source)).size === rows.length)
			.readonly(),
	})
	.refine(
		(row) =>
			row.binding !== null ||
			(Object.keys(row.tokens).length === 0 && row.presentations.length === 0),
	)
	.readonly();
export const PanelMetadataOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('read'),
			workbenchRevision: WorkbenchRevisionSchema,
			metadataRevision: WorkbenchRevisionSchema,
			panels: z
				.array(PanelMetadataRowSchema)
				.max(4)
				.refine((rows) => new Set(rows.map((row) => row.panelId)).size === rows.length)
				.readonly(),
		})
		.superRefine((read, context) => {
			for (const row of read.panels) {
				const writers = new Map<string, string>();
				for (const record of row.presentations) {
					for (const field of [
						record.title,
						record.displayAgent,
						...Object.values(record.stateLabels),
					]) {
						if (field === null || field === undefined) continue;
						const previous = writers.get(field.order);
						if (
							BigInt(field.order) > BigInt(read.metadataRevision) ||
							(previous !== undefined && previous !== record.source)
						)
							context.addIssue({ code: 'custom', message: 'Invalid metadata field write order' });
						writers.set(field.order, record.source);
					}
				}
			}
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('reported'),
			workbenchRevision: WorkbenchRevisionSchema,
			metadataRevision: WorkbenchRevisionSchema,
			panelId: WorkbenchPanelIdSchema,
			binding: WorkbenchPresentationBindingSchema,
			disposition: z.enum(['applied', 'stale']),
		})
		.readonly(),
	z.strictObject({ kind: z.literal('failed'), reason: PanelMetadataFailureSchema }).readonly(),
]);
export const PanelMetadataRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.panel-metadata'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		siteId: SiteIdSchema,
		action: PanelMetadataActionSchema,
	})
	.readonly();
export const PanelMetadataResultSchema = z
	.strictObject({
		kind: z.literal('workbench.panel-metadata.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		siteId: SiteIdSchema,
		outcome: PanelMetadataOutcomeSchema,
	})
	.readonly();
export type MetadataState = z.infer<typeof MetadataStateSchema>;
export type PanelMetadataPatch = z.infer<typeof PanelMetadataPatchSchema>;
export type PanelMetadataAction = z.infer<typeof PanelMetadataActionSchema>;
export type PanelMetadataFailure = z.infer<typeof PanelMetadataFailureSchema>;
export type PanelMetadataField = z.infer<typeof PanelMetadataFieldSchema>;
export type PanelMetadataPresentation = z.infer<typeof PanelMetadataPresentationSchema>;
export type PanelMetadataRow = z.infer<typeof PanelMetadataRowSchema>;
export type PanelMetadataOutcome = z.infer<typeof PanelMetadataOutcomeSchema>;
export type PanelMetadataRequest = z.infer<typeof PanelMetadataRequestSchema>;
export type PanelMetadataResult = z.infer<typeof PanelMetadataResultSchema>;
