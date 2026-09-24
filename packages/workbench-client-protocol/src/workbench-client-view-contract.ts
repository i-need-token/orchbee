import { ClientPanelLayoutSchema } from './workbench-client-layout-contract';
import { z } from 'zod';
import {
	SiteIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	WorkbenchTerminalContextIdSchema,
	WorkbenchTabIdSchema,
	WorkbenchPanelIdSchema,
	WorkbenchRevisionSchema,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_CLIENT_VIEWS = 32;
export const WORKBENCH_MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES = 64 * 1024;
export const WorkbenchClientIdSchema = z
	.string()
	.regex(/^[0-9a-f]{32}$/u)
	.refine((value) => value !== '00000000000000000000000000000000')
	.brand<'WorkbenchClientId'>();
export const ClientViewSequenceSchema = WorkbenchRevisionSchema.refine((value) => value !== '0');
export const ClientViewPhaseSchema = z.enum(['starting', 'ready', 'busy', 'unavailable']);
export const ClientViewSelectionSchema = z
	.strictObject({
		terminalContextId: WorkbenchTerminalContextIdSchema,
		tabId: WorkbenchTabIdSchema.nullable(),
		panelId: WorkbenchPanelIdSchema.nullable(),
		zoomed: z.boolean(),
		panelCount: z.number().int().min(0).max(65535),
	})
	.readonly();
export const ClientViewObservationSchema = z
	.strictObject({
		workbenchRevision: WorkbenchRevisionSchema,
		phase: ClientViewPhaseSchema,
		selection: ClientViewSelectionSchema.nullable(),
	})
	.superRefine((view, context) => {
		const selection = view.selection;
		if (
			(view.phase === 'ready' && selection === null) ||
			(selection !== null &&
				((selection.tabId !== null) !== selection.panelCount > 0 ||
					(selection.panelId !== null && selection.tabId === null) ||
					(selection.zoomed && selection.panelId === null)))
		)
			context.addIssue({ code: 'custom', message: 'Invalid Client view selection' });
	})
	.readonly();
export const ClientViewRecordSchema = z
	.strictObject({
		clientId: WorkbenchClientIdSchema,
		heartbeat: ClientViewSequenceSchema,
		viewRevision: ClientViewSequenceSchema,
		view: ClientViewObservationSchema,
		availability: z.enum(['current', 'stale']),
	})
	.readonly();
const contextTarget = { terminalContextId: WorkbenchTerminalContextIdSchema };
const tabTarget = { ...contextTarget, tabId: WorkbenchTabIdSchema };
const panelTarget = { ...tabTarget, panelId: WorkbenchPanelIdSchema };
export const ClientViewIntentSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('layout'), ...panelTarget }).readonly(),
	z.strictObject({ kind: z.literal('context'), ...contextTarget }).readonly(),
	z.strictObject({ kind: z.literal('tab'), ...tabTarget }).readonly(),
	z.strictObject({ kind: z.literal('panel'), ...panelTarget }).readonly(),
	z.strictObject({ kind: z.literal('zoom'), ...panelTarget, enabled: z.boolean() }).readonly(),
	z.strictObject({ kind: z.literal('toggle-zoom'), ...panelTarget }).readonly(),
]);
export const ClientViewControlSchema = z
	.strictObject({
		operationId: WorkbenchRequestIdSchema,
		clientId: WorkbenchClientIdSchema,
		expectedViewRevision: ClientViewSequenceSchema,
		minimumWorkbenchRevision: WorkbenchRevisionSchema,
		intent: ClientViewIntentSchema,
	})
	.readonly();
export const ClientViewControlReasonSchema = z.enum([
	'busy',
	'stale-view',
	'target-unavailable',
	'projection-unavailable',
	'interrupted',
	'client-disconnected',
	'expired',
	'unsupported',
]);
export const ClientViewCompletionSchema = z
	.strictObject({
		operationId: WorkbenchRequestIdSchema,
		outcome: z.discriminatedUnion('kind', [
			z
				.strictObject({ kind: z.literal('layout-read'), layout: ClientPanelLayoutSchema })
				.readonly(),
			z.strictObject({ kind: z.literal('zoom-toggled'), previouslyZoomed: z.boolean() }).readonly(),
			z.strictObject({ kind: z.enum(['applied', 'zoom-unchanged']) }).readonly(),
			z
				.strictObject({ kind: z.literal('rejected'), reason: ClientViewControlReasonSchema })
				.readonly(),
			z
				.strictObject({ kind: z.literal('incomplete'), reason: ClientViewControlReasonSchema })
				.readonly(),
		]),
	})
	.readonly();
export const ClientViewControlStatusSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('layout-read'),
			layout: ClientPanelLayoutSchema,
			view: ClientViewObservationSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('zoom-toggled'),
			previouslyZoomed: z.boolean(),
			view: ClientViewObservationSchema,
		})
		.readonly(),
	z.strictObject({ kind: z.enum(['queued', 'delivered']) }).readonly(),
	z
		.strictObject({
			kind: z.enum(['applied', 'zoom-unchanged']),
			view: ClientViewObservationSchema,
		})
		.readonly(),
	z.strictObject({ kind: z.literal('rejected'), reason: ClientViewControlReasonSchema }).readonly(),
	z
		.strictObject({
			kind: z.literal('incomplete'),
			reason: ClientViewControlReasonSchema,
			view: ClientViewObservationSchema.nullable(),
		})
		.readonly(),
]);
export const ClientViewOperationSchema = z
	.strictObject({
		command: ClientViewControlSchema,
		status: ClientViewControlStatusSchema,
	})
	.superRefine(({ command, status }, context) => {
		if (status.kind === 'layout-read') {
			const { layout, view } = status;
			const { intent } = command;
			const { selection } = view;
			if (intent.kind !== 'layout') {
				context.addIssue({ code: 'custom', message: 'Layout receipt requires layout intent' });
				return;
			}
			const selected =
				selection !== null &&
				selection.terminalContextId === intent.terminalContextId &&
				selection.tabId === intent.tabId;
			if (
				layout.terminalContextId !== intent.terminalContextId ||
				layout.tabId !== intent.tabId ||
				layout.sourcePanelId !== intent.panelId ||
				BigInt(layout.workbenchRevision) < BigInt(command.minimumWorkbenchRevision) ||
				layout.selected !== selected ||
				(selected &&
					(selection === null ||
						layout.focusedPanelId !== selection.panelId ||
						layout.zoomed !== selection.zoomed ||
						layout.panels.length !== selection.panelCount ||
						layout.workbenchRevision !== view.workbenchRevision))
			) {
				context.addIssue({ code: 'custom', message: 'Layout target is not observed' });
			}
			return;
		}
		if (
			status.kind !== 'applied' &&
			status.kind !== 'zoom-unchanged' &&
			status.kind !== 'zoom-toggled'
		)
			return;
		const { view } = status;
		const { selection } = view;
		const { intent } = command;
		let zoomMatches: boolean;
		switch (status.kind) {
			case 'zoom-toggled':
				zoomMatches =
					intent.kind === 'toggle-zoom' &&
					selection !== null &&
					selection.panelCount > 1 &&
					selection.zoomed !== status.previouslyZoomed;
				break;
			case 'zoom-unchanged':
				zoomMatches =
					(intent.kind === 'toggle-zoom' || (intent.kind === 'zoom' && intent.enabled)) &&
					selection?.panelCount === 1 &&
					!selection.zoomed;
				break;
			case 'applied':
				zoomMatches =
					intent.kind !== 'toggle-zoom' &&
					intent.kind !== 'layout' &&
					(intent.kind !== 'zoom' || selection?.zoomed === intent.enabled);
				break;
		}

		const observed =
			BigInt(view.workbenchRevision) >= BigInt(command.minimumWorkbenchRevision) &&
			view.phase !== 'starting' &&
			view.phase !== 'busy' &&
			selection !== null &&
			selection.terminalContextId === intent.terminalContextId &&
			(intent.kind === 'context' ||
				(selection.tabId === intent.tabId &&
					(intent.kind === 'tab' || (selection.panelId === intent.panelId && zoomMatches))));
		if (!observed || !zoomMatches)
			context.addIssue({ code: 'custom', message: 'Applied target is not observed' });
	})
	.readonly();
export const ClientViewActionSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('synchronize'),
			heartbeat: ClientViewSequenceSchema,
			viewRevision: ClientViewSequenceSchema,
			view: ClientViewObservationSchema,
			completion: ClientViewCompletionSchema.nullable(),
		})
		.readonly(),
	z.strictObject({ kind: z.literal('list') }).readonly(),
	z.strictObject({ kind: z.literal('get'), clientId: WorkbenchClientIdSchema }).readonly(),
	z.strictObject({ kind: z.literal('submit'), command: ClientViewControlSchema }).readonly(),
	z.strictObject({ kind: z.literal('read'), operationId: WorkbenchRequestIdSchema }).readonly(),
]);
export const ClientViewOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('synchronized'),
			client: ClientViewRecordSchema,
			control: ClientViewControlSchema.nullable(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('listed'),
			clients: z.array(ClientViewRecordSchema).max(WORKBENCH_MAXIMUM_CLIENT_VIEWS).readonly(),
		})
		.readonly(),
	z.strictObject({ kind: z.literal('found'), client: ClientViewRecordSchema }).readonly(),
	z.strictObject({ kind: z.literal('operation'), operation: ClientViewOperationSchema }).readonly(),
	z.strictObject({ kind: z.enum(['not-found', 'conflict', 'denied']) }).readonly(),
	z
		.strictObject({
			kind: z.literal('unavailable'),
			reason: z.enum([
				'client-unavailable',
				'server-busy',
				'receipt-capacity',
				'unsupported',
				'sequence-exhausted',
			]),
		})
		.readonly(),
]);
export const WorkbenchClientViewRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.client-view'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		siteId: SiteIdSchema,
		action: ClientViewActionSchema,
	})
	.readonly();
export const WorkbenchClientViewResultSchema = z
	.strictObject({
		kind: z.literal('workbench.client-view.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		siteId: SiteIdSchema,
		outcome: ClientViewOutcomeSchema,
	})
	.superRefine(({ outcome }, context) => {
		if (
			outcome.kind === 'listed' &&
			new Set(outcome.clients.map((client) => client.clientId)).size !== outcome.clients.length
		)
			context.addIssue({ code: 'custom', message: 'Duplicate Client identities' });
		if (
			outcome.kind === 'synchronized' &&
			outcome.control !== null &&
			(outcome.control.clientId !== outcome.client.clientId ||
				outcome.control.expectedViewRevision !== outcome.client.viewRevision ||
				outcome.client.availability !== 'current')
		)
			context.addIssue({ code: 'custom', message: 'Control does not match current Client view' });
	})
	.readonly();

export type WorkbenchClientId = z.infer<typeof WorkbenchClientIdSchema>;
export type ClientViewSequence = z.infer<typeof ClientViewSequenceSchema>;
export type ClientViewPhase = z.infer<typeof ClientViewPhaseSchema>;
export type ClientViewSelection = z.infer<typeof ClientViewSelectionSchema>;
export type ClientViewObservation = z.infer<typeof ClientViewObservationSchema>;
export type ClientViewRecord = z.infer<typeof ClientViewRecordSchema>;
export type ClientViewIntent = z.infer<typeof ClientViewIntentSchema>;
export type ClientViewControl = z.infer<typeof ClientViewControlSchema>;
export type ClientViewControlReason = z.infer<typeof ClientViewControlReasonSchema>;
export type ClientViewCompletion = z.infer<typeof ClientViewCompletionSchema>;
export type ClientViewControlStatus = z.infer<typeof ClientViewControlStatusSchema>;
export type ClientViewOperation = z.infer<typeof ClientViewOperationSchema>;
export type ClientViewAction = z.infer<typeof ClientViewActionSchema>;
export type ClientViewOutcome = z.infer<typeof ClientViewOutcomeSchema>;
export type WorkbenchClientViewRequest = z.infer<typeof WorkbenchClientViewRequestSchema>;
export type WorkbenchClientViewResult = z.infer<typeof WorkbenchClientViewResultSchema>;
