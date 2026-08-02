import {
	WorkbenchSharedDocumentSchema,
	WorkbenchSharedDocumentRevisionSchema,
} from '@orchbee/workbench/document';
import { z } from 'zod';

export const WORKBENCH_DOCUMENT_CONTROL_IPC_CHANNEL =
	'orchbee.desktop.workbench-document-control-port';
export const WORKBENCH_DOCUMENT_CONTROL_WINDOW_MESSAGE =
	'orchbee.desktop.workbench-document-control-ready';

export const WorkbenchDocumentRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'WorkbenchDocumentRequestId'>();

const unavailableLoadResultSchema = z.strictObject({
	kind: z.literal('unavailable'),
	reason: z.enum(['invalid', 'read-failed']),
});

const WorkbenchDocumentLoadResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({ document: WorkbenchSharedDocumentSchema, kind: z.literal('available') }),
	z.strictObject({ kind: z.literal('missing') }),
	unavailableLoadResultSchema,
]);

const WorkbenchDocumentCommitResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('committed'),
		revision: WorkbenchSharedDocumentRevisionSchema,
	}),
	z.strictObject({
		actualRevision: WorkbenchSharedDocumentRevisionSchema.nullable(),
		kind: z.literal('conflict'),
	}),
	z.strictObject({ kind: z.literal('rejected'), reason: z.literal('revision-sequence') }),
	z.strictObject({
		kind: z.literal('unavailable'),
		reason: z.enum(['invalid', 'read-failed', 'write-failed']),
	}),
]);

export const WorkbenchDocumentControlCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		requestId: WorkbenchDocumentRequestIdSchema,
		type: z.literal('workbench-document-load-requested'),
	}),
	z.strictObject({
		document: WorkbenchSharedDocumentSchema,
		expectedRevision: WorkbenchSharedDocumentRevisionSchema.nullable(),
		requestId: WorkbenchDocumentRequestIdSchema,
		type: z.literal('workbench-document-commit-requested'),
	}),
]);

export const WorkbenchDocumentControlEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		requestId: WorkbenchDocumentRequestIdSchema,
		result: WorkbenchDocumentLoadResultSchema,
		type: z.literal('workbench-document-load-completed'),
	}),
	z.strictObject({
		requestId: WorkbenchDocumentRequestIdSchema,
		result: WorkbenchDocumentCommitResultSchema,
		type: z.literal('workbench-document-commit-completed'),
	}),
]);

export const WorkbenchDocumentControlBootstrapSchema = z.strictObject({
	type: z.literal(WORKBENCH_DOCUMENT_CONTROL_IPC_CHANNEL),
});

export const WorkbenchDocumentControlWindowMessageSchema = z.strictObject({
	security: z.strictObject({ contextIsolated: z.boolean(), sandboxed: z.boolean() }),
	type: z.literal(WORKBENCH_DOCUMENT_CONTROL_WINDOW_MESSAGE),
});

export type WorkbenchDocumentRequestId = z.infer<typeof WorkbenchDocumentRequestIdSchema>;
export type WorkbenchDocumentLoadResult = z.infer<typeof WorkbenchDocumentLoadResultSchema>;
export type WorkbenchDocumentCommitResult = z.infer<typeof WorkbenchDocumentCommitResultSchema>;
export type WorkbenchDocumentControlCommand = z.infer<typeof WorkbenchDocumentControlCommandSchema>;
export type WorkbenchDocumentControlEvent = z.infer<typeof WorkbenchDocumentControlEventSchema>;
