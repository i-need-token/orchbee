import { z } from 'zod';

import {
	WorkbenchTabCloseReadSchema,
	WorkbenchTabCloseReadResultSchema,
	WorkbenchTabCloseResumeSchema,
	WorkbenchTabCloseResumeResultSchema,
} from './workbench-tab-close-contract';

import {
	SiteIdSchema,
	WORKBENCH_MAXIMUM_ENCODED_INTENT_BYTES,
	WORKBENCH_MAXIMUM_ENCODED_PROJECTION_BYTES,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchLabelSchema,
	WorkbenchPanelIdSchema,
	WorkbenchProjectIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchSnapshotReadIntentSchema,
	WorkbenchSnapshotSchema,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
	WorkbenchWorkspaceIdSchema,
	TerminalPresentationIdSchema,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES =
	WORKBENCH_MAXIMUM_ENCODED_PROJECTION_BYTES + 1024;

const terminalGeometryDimensionSchema = z.number().int().min(1).max(4096);

export const WorkbenchTerminalGeometrySchema = z
	.strictObject({
		columns: terminalGeometryDimensionSchema,
		rows: terminalGeometryDimensionSchema,
		cellPixelWidth: terminalGeometryDimensionSchema,
		cellPixelHeight: terminalGeometryDimensionSchema,
	})
	.superRefine((geometry, context) => {
		if (geometry.columns * geometry.rows > 262_144) {
			context.addIssue({
				code: 'custom',
				message: 'Workbench terminal grid exceeds 262144 cells',
				path: ['columns'],
			});
		}
		if (geometry.columns * geometry.cellPixelWidth > 65_535) {
			context.addIssue({
				code: 'custom',
				message: 'Workbench terminal pixel width exceeds 65535',
				path: ['cellPixelWidth'],
			});
		}
		if (geometry.rows * geometry.cellPixelHeight > 65_535) {
			context.addIssue({
				code: 'custom',
				message: 'Workbench terminal pixel height exceeds 65535',
				path: ['cellPixelHeight'],
			});
		}
	})
	.readonly();

export const WorkbenchTabMoveDirectionSchema = z.enum(['previous', 'next', 'first', 'last']);

export const WorkbenchActionSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('tab.close'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
		})
		.readonly(),
	z.strictObject({ kind: z.literal('project.create'), label: WorkbenchLabelSchema }).readonly(),
	z
		.strictObject({
			kind: z.literal('project.rename'),
			projectId: WorkbenchProjectIdSchema,
			label: WorkbenchLabelSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace.create'),
			projectId: WorkbenchProjectIdSchema,
			label: WorkbenchLabelSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace.rename'),
			projectId: WorkbenchProjectIdSchema,
			workspaceId: WorkbenchWorkspaceIdSchema,
			label: WorkbenchLabelSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tab.rename'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			label: WorkbenchLabelSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tab.create'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			label: WorkbenchLabelSchema,
			geometry: WorkbenchTerminalGeometrySchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tab.move'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			direction: WorkbenchTabMoveDirectionSchema,
		})
		.readonly(),
]);

const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
};

export const WorkbenchCommandSchema = z
	.strictObject({
		kind: z.literal('workbench.command'),
		...correlation,
		siteId: SiteIdSchema,
		expectedRevision: WorkbenchRevisionSchema,
		action: WorkbenchActionSchema,
	})
	.readonly();

export const WorkbenchRequestSchema = z
	.discriminatedUnion('kind', [
		WorkbenchSnapshotReadIntentSchema,
		WorkbenchCommandSchema,
		WorkbenchTabCloseReadSchema,
		WorkbenchTabCloseResumeSchema,
	])
	.refine(
		(value) => encodedBytes(value) <= WORKBENCH_MAXIMUM_ENCODED_INTENT_BYTES,
		'Workbench request exceeds its encoded limit',
	);

export const WorkbenchRejectionReasonSchema = z.enum([
	'site-mismatch',
	'revision-mismatch',
	'project-unavailable',
	'workspace-unavailable',
	'terminal-context-unavailable',
	'tab-unavailable',
	'resource-limit',
	'identity-exhausted',
	'revision-exhausted',
	'request-identity-conflict',
]);

export const WorkbenchCommandUnavailableReasonSchema = z.enum([
	'request-ledger-full',
	'no-compatible-worker',
	'terminal-busy',
	'terminal-runtime-unavailable',
	'server-busy',
	'service-unavailable',
]);
export const WorkbenchSnapshotUnavailableReasonSchema = z.enum(['service-unavailable']);
export const WorkbenchTerminalUnavailableReasonSchema = z.enum([
	'terminal-runtime-unavailable',
	'server-busy',
]);
export const WorkbenchTerminalFailureReasonSchema = z.enum(['terminal-create-failed']);
const deniedSchema = z.strictObject({ kind: z.literal('denied') }).readonly();
const commandUnavailableSchema = z
	.strictObject({ kind: z.literal('unavailable'), reason: WorkbenchCommandUnavailableReasonSchema })
	.readonly();

export const WorkbenchTerminalOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({ kind: z.literal('started'), geometry: WorkbenchTerminalGeometrySchema })
		.readonly(),
	z
		.strictObject({
			kind: z.literal('unavailable'),
			reason: WorkbenchTerminalUnavailableReasonSchema,
		})
		.readonly(),
	z
		.strictObject({ kind: z.literal('failed'), reason: WorkbenchTerminalFailureReasonSchema })
		.readonly(),
	z.strictObject({ kind: z.literal('outcome-unknown') }).readonly(),
]);

export const WorkbenchCommandOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('tab-close-accepted'),
			closeRequestId: WorkbenchRequestIdSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			acceptedRevision: WorkbenchRevisionSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('project-created'),
			revision: WorkbenchRevisionSchema,
			projectId: WorkbenchProjectIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('workspace-created'),
			revision: WorkbenchRevisionSchema,
			workspaceId: WorkbenchWorkspaceIdSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
		})
		.readonly(),
	z.strictObject({ kind: z.literal('renamed'), revision: WorkbenchRevisionSchema }).readonly(),
	z.strictObject({ kind: z.literal('unchanged'), revision: WorkbenchRevisionSchema }).readonly(),
	z
		.strictObject({
			kind: z.literal('tab-moved'),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tab-create-pending'),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			presentationId: TerminalPresentationIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('tab-created'),
			revision: WorkbenchRevisionSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			panelId: WorkbenchPanelIdSchema,
			presentationId: TerminalPresentationIdSchema,
			terminalOutcome: WorkbenchTerminalOutcomeSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('rejected'),
			revision: WorkbenchRevisionSchema,
			reason: WorkbenchRejectionReasonSchema,
		})
		.readonly(),
	deniedSchema,
	commandUnavailableSchema,
]);

export const WorkbenchSnapshotOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('available'), snapshot: WorkbenchSnapshotSchema }).readonly(),
	deniedSchema,
	z
		.strictObject({
			kind: z.literal('unavailable'),
			reason: WorkbenchSnapshotUnavailableReasonSchema,
		})
		.readonly(),
]);

export const WorkbenchCommandResultSchema = z
	.strictObject({
		kind: z.literal('workbench.command.result'),
		...correlation,
		outcome: WorkbenchCommandOutcomeSchema,
	})
	.readonly();
export const WorkbenchSnapshotResultSchema = z
	.strictObject({
		kind: z.literal('workbench.snapshot.result'),
		...correlation,
		outcome: WorkbenchSnapshotOutcomeSchema,
	})
	.readonly();

export const WorkbenchResponseSchema = z
	.discriminatedUnion('kind', [
		WorkbenchCommandResultSchema,
		WorkbenchSnapshotResultSchema,
		WorkbenchTabCloseReadResultSchema,
		WorkbenchTabCloseResumeResultSchema,
	])
	.superRefine((value, context) => {
		const maximum =
			value.kind === 'workbench.snapshot.result'
				? WORKBENCH_MAXIMUM_ENCODED_SNAPSHOT_RESULT_BYTES
				: WORKBENCH_MAXIMUM_ENCODED_INTENT_BYTES;
		if (encodedBytes(value) > maximum)
			context.addIssue({ code: 'custom', message: 'Workbench result exceeds its encoded limit' });
		if (
			value.kind === 'workbench.snapshot.result' &&
			value.outcome.kind === 'available' &&
			encodedBytes(value.outcome.snapshot) > WORKBENCH_MAXIMUM_ENCODED_PROJECTION_BYTES
		) {
			context.addIssue({
				code: 'custom',
				message: 'Nested Workbench snapshot exceeds its encoded limit',
			});
		}
	});

export type WorkbenchTerminalGeometry = z.infer<typeof WorkbenchTerminalGeometrySchema>;
export type WorkbenchAction = z.infer<typeof WorkbenchActionSchema>;
export type WorkbenchTabMoveDirection = z.infer<typeof WorkbenchTabMoveDirectionSchema>;
export type WorkbenchCommand = z.infer<typeof WorkbenchCommandSchema>;
export type WorkbenchRequest = z.infer<typeof WorkbenchRequestSchema>;
export type WorkbenchCommandOutcome = z.infer<typeof WorkbenchCommandOutcomeSchema>;
export type WorkbenchCommandResult = z.infer<typeof WorkbenchCommandResultSchema>;
export type WorkbenchSnapshotOutcome = z.infer<typeof WorkbenchSnapshotOutcomeSchema>;
export type WorkbenchSnapshotResult = z.infer<typeof WorkbenchSnapshotResultSchema>;
export type WorkbenchResponse = z.infer<typeof WorkbenchResponseSchema>;
export type WorkbenchRejectionReason = z.infer<typeof WorkbenchRejectionReasonSchema>;
export type WorkbenchCommandUnavailableReason = z.infer<
	typeof WorkbenchCommandUnavailableReasonSchema
>;
export type WorkbenchSnapshotUnavailableReason = z.infer<
	typeof WorkbenchSnapshotUnavailableReasonSchema
>;
export type WorkbenchTerminalUnavailableReason = z.infer<
	typeof WorkbenchTerminalUnavailableReasonSchema
>;
export type WorkbenchTerminalFailureReason = z.infer<typeof WorkbenchTerminalFailureReasonSchema>;
export type WorkbenchTerminalOutcome = z.infer<typeof WorkbenchTerminalOutcomeSchema>;

export function parseWorkbenchRequest(input: unknown): WorkbenchRequest {
	return WorkbenchRequestSchema.parse(input);
}

export function parseWorkbenchResponse(input: unknown): WorkbenchResponse {
	return WorkbenchResponseSchema.parse(input);
}

function encodedBytes(value: unknown): number {
	return new TextEncoder().encode(JSON.stringify(value)).length;
}
