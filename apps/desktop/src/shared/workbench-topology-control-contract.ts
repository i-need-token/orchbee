import {
	WorkbenchTopologyDocumentSchema,
	WorkbenchTopologyRevisionSchema,
} from '@orchbee/workbench/topology';
import { z } from 'zod';

export const WORKBENCH_TOPOLOGY_CONTROL_IPC_CHANNEL =
	'orchbee.desktop.workbench-topology-control-port';
export const WORKBENCH_TOPOLOGY_CONTROL_WINDOW_MESSAGE =
	'orchbee.desktop.workbench-topology-control-ready';

export const WorkbenchTopologyRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'WorkbenchTopologyRequestId'>();

const unavailableLoadResultSchema = z.strictObject({
	kind: z.literal('unavailable'),
	reason: z.enum(['invalid', 'read-failed']),
});

const WorkbenchTopologyLoadResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({ document: WorkbenchTopologyDocumentSchema, kind: z.literal('available') }),
	z.strictObject({ kind: z.literal('missing') }),
	unavailableLoadResultSchema,
]);

const WorkbenchTopologyCommitResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('committed'),
		revision: WorkbenchTopologyRevisionSchema,
	}),
	z.strictObject({
		actualRevision: WorkbenchTopologyRevisionSchema.nullable(),
		kind: z.literal('conflict'),
	}),
	z.strictObject({ kind: z.literal('rejected'), reason: z.literal('revision-sequence') }),
	z.strictObject({
		kind: z.literal('unavailable'),
		reason: z.enum(['invalid', 'read-failed', 'write-failed']),
	}),
]);

export const WorkbenchTopologyControlCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		requestId: WorkbenchTopologyRequestIdSchema,
		type: z.literal('workbench-topology-load-requested'),
	}),
	z.strictObject({
		document: WorkbenchTopologyDocumentSchema,
		expectedRevision: WorkbenchTopologyRevisionSchema.nullable(),
		requestId: WorkbenchTopologyRequestIdSchema,
		type: z.literal('workbench-topology-commit-requested'),
	}),
]);

export const WorkbenchTopologyControlEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		requestId: WorkbenchTopologyRequestIdSchema,
		result: WorkbenchTopologyLoadResultSchema,
		type: z.literal('workbench-topology-load-completed'),
	}),
	z.strictObject({
		requestId: WorkbenchTopologyRequestIdSchema,
		result: WorkbenchTopologyCommitResultSchema,
		type: z.literal('workbench-topology-commit-completed'),
	}),
]);

export const WorkbenchTopologyControlBootstrapSchema = z.strictObject({
	type: z.literal(WORKBENCH_TOPOLOGY_CONTROL_IPC_CHANNEL),
});

export const WorkbenchTopologyControlWindowMessageSchema = z.strictObject({
	security: z.strictObject({ contextIsolated: z.boolean(), sandboxed: z.boolean() }),
	type: z.literal(WORKBENCH_TOPOLOGY_CONTROL_WINDOW_MESSAGE),
});

export type WorkbenchTopologyRequestId = z.infer<typeof WorkbenchTopologyRequestIdSchema>;
export type WorkbenchTopologyLoadResult = z.infer<typeof WorkbenchTopologyLoadResultSchema>;
export type WorkbenchTopologyCommitResult = z.infer<typeof WorkbenchTopologyCommitResultSchema>;
export type WorkbenchTopologyControlCommand = z.infer<typeof WorkbenchTopologyControlCommandSchema>;
export type WorkbenchTopologyControlEvent = z.infer<typeof WorkbenchTopologyControlEventSchema>;
