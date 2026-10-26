import { ConfiguredCommandPopupSpecSchema } from './workbench-configured-command-spec-contract';
import { PluginPopupSpecSchema } from './workbench-plugin-popup-contract';
import { ScrollbackEditorSpecSchema } from './workbench-terminal-editor-contract';
import { ClientPresentationSnapshotSchema } from './workbench-client-presentation-contract';
import { TerminalAgentBindingSchema } from './workbench-panel-agent-contract';
import {
	ClientPanelDirectionSchema,
	ClientLayoutAdjustmentSchema,
	ClientResizeAmountSchema,
	ClientSplitPathSchema,
	ClientSplitRatioSchema,
	ClientPanelLayoutSchema,
} from './workbench-client-layout-contract';
import { z } from 'zod';
import {
	ClientViewportPanelsSchema,
	ClientViewportSnapshotSchema,
} from './workbench-client-viewport-contract';
import { AgentSessionIdSchema } from '@orchbee/agent';
import {
	SessionIdSchema,
	TerminalPresentationIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	WorkbenchTerminalContextIdSchema,
	WorkbenchTabIdSchema,
	WorkbenchPanelIdSchema,
	WorkbenchPresentationBindingSchema,
	WorkbenchSplitIdSchema,
	WorkbenchPanelSlotIdSchema,
	WorkbenchRevisionSchema,
} from './workbench-semantic-contract';

const WORKBENCH_MAXIMUM_CLIENT_VIEWS = 32;
export const WORKBENCH_MAXIMUM_ENCODED_CLIENT_VIEW_RESULT_BYTES = 64 * 1024;
export const WorkbenchClientIdSchema = z
	.string()
	.regex(/^[0-9a-f]{32}$/u)
	.refine((value) => value !== '00000000000000000000000000000000')
	.brand<'WorkbenchClientId'>();
export const ClientViewSequenceSchema = WorkbenchRevisionSchema.refine((value) => value !== '0');
const ClientViewPhaseSchema = z.enum(['starting', 'ready', 'busy', 'unavailable']);
const ClientViewSelectionSchema = z
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
		splitRevision: WorkbenchRevisionSchema.nullish(),
		phase: ClientViewPhaseSchema,
		selection: ClientViewSelectionSchema.nullable(),
	})
	.superRefine((view, context) => {
		const selection = view.selection;
		if (
			(view.splitRevision !== null && view.splitRevision !== undefined && selection === null) ||
			(selection !== null &&
				((selection.tabId !== null) !== selection.panelCount > 0 ||
					(selection.panelId !== null && selection.tabId === null) ||
					(selection.zoomed && selection.panelId === null)))
		)
			context.addIssue({ code: 'custom', message: 'Invalid Client view selection' });
	})
	.readonly();
const ClientViewRecordSchema = z
	.strictObject({
		clientId: WorkbenchClientIdSchema,
		heartbeat: ClientViewSequenceSchema,
		viewRevision: ClientViewSequenceSchema,
		view: ClientViewObservationSchema,
		availability: z.enum(['current', 'stale']),
	})
	.readonly();
const splitIdentity = z
	.strictObject({
		splitId: WorkbenchSplitIdSchema,
		firstSlotId: WorkbenchPanelSlotIdSchema,
		secondSlotId: WorkbenchPanelSlotIdSchema,
	})
	.refine((value) => value.firstSlotId !== value.secondSlotId)
	.readonly();
const contextTarget = { terminalContextId: WorkbenchTerminalContextIdSchema };
const tabTarget = { ...contextTarget, tabId: WorkbenchTabIdSchema };
const panelTarget = { ...tabTarget, panelId: WorkbenchPanelIdSchema };
export const WORKBENCH_MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES = 64 * 1024;
const attentionSessions = z
	.array(AgentSessionIdSchema)
	.min(1)
	.max(100)
	.refine((sessions) => new Set(sessions).size === sessions.length);
const agentInteger = z.number().int().min(0).max(Number.MAX_SAFE_INTEGER);
const ClientAgentAttentionSnapshotSchema = z
	.strictObject({
		cursor: z.string().regex(/^v1\.[A-Za-z0-9_-]{59}\.[A-Za-z0-9_-]{43}$/u),
		revision: agentInteger,
		outerFocused: z.boolean().nullable(),
		entries: z
			.array(
				z.strictObject({
					sessionId: AgentSessionIdSchema,
					state: z
						.strictObject({
							generation: agentInteger,
							sequence: agentInteger,
							stateChangeRevision: agentInteger,
							seen: z.boolean(),
						})
						.nullable(),
				}),
			)
			.min(1)
			.max(100),
	})
	.superRefine((snapshot, context) => {
		if (
			new Set(snapshot.entries.map((entry) => entry.sessionId)).size !== snapshot.entries.length ||
			snapshot.entries.some(
				(entry) => entry.state !== null && entry.state.stateChangeRevision > snapshot.revision,
			)
		) {
			context.addIssue({ code: 'custom', message: 'Invalid Client attention snapshot' });
		}
	})
	.readonly();
const physicalBindings = z
	.array(TerminalAgentBindingSchema)
	.min(1)
	.max(100)
	.refine((bindings) => new Set(bindings.map((b) => b.presentationId)).size === bindings.length)
	.readonly();
const ClientTerminalAgentAttentionSnapshotSchema = z
	.strictObject({
		reportRevision: WorkbenchRevisionSchema,
		outerFocused: z.boolean().nullable(),
		entries: z
			.array(
				z
					.strictObject({
						binding: TerminalAgentBindingSchema,
						state: z
							.strictObject({ sequence: WorkbenchRevisionSchema, seen: z.boolean() })
							.readonly()
							.nullable(),
					})
					.readonly(),
			)
			.min(1)
			.max(100)
			.refine((rows) => new Set(rows.map((r) => r.binding.presentationId)).size === rows.length)
			.readonly(),
	})
	.readonly();
const selectionContexts = z
	.array(WorkbenchTerminalContextIdSchema)
	.min(1)
	.max(128)
	.refine((ids) => new Set(ids).size === ids.length)
	.readonly();
const clientNavigation = z
	.strictObject({
		workbenchRevision: WorkbenchRevisionSchema,
		contexts: z
			.array(
				z
					.strictObject({
						terminalContextId: WorkbenchTerminalContextIdSchema,
						activeTabId: WorkbenchTabIdSchema.nullable(),
					})
					.readonly(),
			)
			.min(1)
			.max(128)
			.readonly(),
	})
	.superRefine((value, context) => {
		const ids = value.contexts.map((entry) => entry.terminalContextId);
		const tabs = value.contexts.flatMap((entry) =>
			entry.activeTabId === null ? [] : [entry.activeTabId],
		);
		if (new Set(ids).size !== ids.length || new Set(tabs).size !== tabs.length)
			context.addIssue({ code: 'custom', message: 'Duplicate Client navigation identity' });
	})
	.readonly();
const ClientViewIntentSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('popup-open'), spec: PluginPopupSpecSchema }).readonly(),
	z.strictObject({ kind: z.literal('editor-open'), spec: ScrollbackEditorSpecSchema }).readonly(),
	z
		.strictObject({ kind: z.literal('command-popup-open'), spec: ConfiguredCommandPopupSpecSchema })
		.readonly(),
	z.strictObject({ kind: z.literal('popup-close') }).readonly(),
	z
		.strictObject({
			kind: z.literal('overlay'),
			...panelTarget,
			presentationId: TerminalPresentationIdSchema,
			expectedBinding: WorkbenchPresentationBindingSchema,
			returnTo: z.strictObject({
				panelId: WorkbenchPanelIdSchema,
				presentationId: TerminalPresentationIdSchema,
				binding: WorkbenchPresentationBindingSchema.nullable(),
			}),
			previouslyZoomed: z.boolean(),
		})
		.refine(
			(value) =>
				value.panelId !== value.returnTo.panelId &&
				value.presentationId !== value.returnTo.presentationId,
		),
	z
		.strictObject({
			kind: z.literal('agent-focus'),
			...panelTarget,
			presentationId: TerminalPresentationIdSchema,
			target: z.discriminatedUnion('kind', [
				z.strictObject({ kind: z.literal('session'), sessionId: AgentSessionIdSchema }),
				z.strictObject({ kind: z.literal('terminal'), binding: TerminalAgentBindingSchema }),
			]),
		})
		.superRefine((value, context) => {
			if (
				value.target.kind === 'terminal' &&
				value.target.binding.presentationId !== value.presentationId
			)
				context.addIssue({
					code: 'custom',
					message: 'Agent focus presentation does not match its physical binding',
				});
		})
		.readonly(),
	z
		.strictObject({ kind: z.literal('terminal-agent-attention'), bindings: physicalBindings })
		.readonly(),
	z.strictObject({ kind: z.literal('presentation'), panelId: WorkbenchPanelIdSchema }).readonly(),
	z.strictObject({ kind: z.literal('viewports'), panelIds: ClientViewportPanelsSchema }).readonly(),
	z
		.strictObject({ kind: z.literal('selections'), terminalContextIds: selectionContexts })
		.readonly(),
	z.strictObject({ kind: z.literal('agent-attention'), sessionIds: attentionSessions }).readonly(),
	z
		.strictObject({
			kind: z.literal('resize'),
			...panelTarget,
			direction: ClientPanelDirectionSchema,
			amount: ClientResizeAmountSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('set-split-ratio'),
			...tabTarget,
			path: ClientSplitPathSchema,
			ratio: ClientSplitRatioSchema,
			expectedSplit: splitIdentity.optional(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('focus-direction'),
			...panelTarget,
			direction: ClientPanelDirectionSchema,
		})
		.readonly(),
	z.strictObject({ kind: z.literal('layout'), ...panelTarget }).readonly(),
	z.strictObject({ kind: z.literal('context'), ...contextTarget }).readonly(),
	z.strictObject({ kind: z.literal('tab'), ...tabTarget }).readonly(),
	z.strictObject({ kind: z.literal('panel'), ...panelTarget }).readonly(),
	z.strictObject({ kind: z.literal('zoom'), ...panelTarget, enabled: z.boolean() }).readonly(),
	z.strictObject({ kind: z.literal('toggle-zoom'), ...panelTarget }).readonly(),
]);
const ClientViewControlSchema = z
	.strictObject({
		operationId: WorkbenchRequestIdSchema,
		clientId: WorkbenchClientIdSchema,
		expectedViewRevision: ClientViewSequenceSchema,
		minimumWorkbenchRevision: WorkbenchRevisionSchema,
		intent: ClientViewIntentSchema,
	})
	.readonly();
const ClientViewControlReasonSchema = z.enum([
	'busy',
	'stale-view',
	'target-unavailable',
	'projection-unavailable',
	'interrupted',
	'client-disconnected',
	'expired',
	'unsupported',
]);
const ClientViewCompletionSchema = z
	.strictObject({
		operationId: WorkbenchRequestIdSchema,
		outcome: z.discriminatedUnion('kind', [
			z
				.strictObject({ kind: z.literal('popup-opened'), popupId: WorkbenchRequestIdSchema })
				.readonly(),
			z.strictObject({ kind: z.literal('popup-closed'), hadPopup: z.boolean() }).readonly(),
			z
				.strictObject({
					kind: z.literal('presentation-read'),
					presentation: ClientPresentationSnapshotSchema,
				})
				.readonly(),
			z
				.strictObject({ kind: z.literal('viewport-read'), viewport: ClientViewportSnapshotSchema })
				.readonly(),
			z
				.strictObject({ kind: z.literal('navigation-read'), navigation: clientNavigation })
				.readonly(),
			z
				.strictObject({
					kind: z.literal('terminal-attention-read'),
					attention: ClientTerminalAgentAttentionSnapshotSchema,
				})
				.readonly(),
			z
				.strictObject({
					kind: z.literal('attention-read'),
					attention: ClientAgentAttentionSnapshotSchema,
				})
				.readonly(),
			z
				.strictObject({
					kind: z.literal('layout-adjusted'),
					adjustment: ClientLayoutAdjustmentSchema,
				})
				.readonly(),
			z
				.strictObject({ kind: z.literal('direction-focused'), panelId: WorkbenchPanelIdSchema })
				.readonly(),
			z
				.strictObject({
					kind: z.literal('direction-unchanged'),
					layoutRevision: WorkbenchRevisionSchema,
				})
				.readonly(),
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
const ClientViewControlStatusSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('popup-opened'),
			popupId: WorkbenchRequestIdSchema,
			view: ClientViewObservationSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('popup-closed'),
			hadPopup: z.boolean(),
			view: ClientViewObservationSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('presentation-read'),
			presentation: ClientPresentationSnapshotSchema,
			view: ClientViewObservationSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('viewport-read'),
			viewport: ClientViewportSnapshotSchema,
			view: ClientViewObservationSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('navigation-read'),
			navigation: clientNavigation,
			view: ClientViewObservationSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('terminal-attention-read'),
			attention: ClientTerminalAgentAttentionSnapshotSchema,
			view: ClientViewObservationSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('attention-read'),
			attention: ClientAgentAttentionSnapshotSchema,
			view: ClientViewObservationSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('layout-adjusted'),
			adjustment: ClientLayoutAdjustmentSchema,
			view: ClientViewObservationSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('direction-focused'),
			panelId: WorkbenchPanelIdSchema,
			view: ClientViewObservationSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('direction-unchanged'),
			layoutRevision: WorkbenchRevisionSchema,
			view: ClientViewObservationSchema,
		})
		.readonly(),
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
const ClientViewOperationSchema = z
	.strictObject({
		command: ClientViewControlSchema,
		status: ClientViewControlStatusSchema,
	})
	.superRefine(({ command, status }, context) => {
		if (status.kind === 'popup-opened' || status.kind === 'popup-closed') {
			const { intent } = command;
			const { view } = status;
			const valid =
				BigInt(view.workbenchRevision) >= BigInt(command.minimumWorkbenchRevision) &&
				(status.kind === 'popup-opened'
					? (intent.kind === 'popup-open' ||
							intent.kind === 'editor-open' ||
							intent.kind === 'command-popup-open') &&
						status.popupId === command.operationId &&
						view.phase === 'busy' &&
						view.selection?.terminalContextId === intent.spec.source.terminalContextId &&
						view.selection.tabId === intent.spec.source.tabId &&
						view.selection.panelId === intent.spec.source.panelId
					: intent.kind === 'popup-close' &&
						view.phase !== 'starting' &&
						(!status.hadPopup || view.phase !== 'busy'));
			if (!valid)
				context.addIssue({
					code: 'custom',
					message: 'Popup receipt does not match its intent, source or revision',
				});
			return;
		}
		if (status.kind === 'layout-adjusted') {
			const { intent } = command;
			if (intent.kind !== 'resize' && intent.kind !== 'set-split-ratio') {
				context.addIssue({
					code: 'custom',
					message: 'Layout adjustment requires resize or ratio intent',
				});
				return;
			}
			const { adjustment, view } = status;
			const selected =
				view.selection !== null &&
				view.selection.terminalContextId === intent.terminalContextId &&
				view.selection.tabId === intent.tabId;
			const change = adjustment.change;
			let ratioMatches: boolean;
			if (change.kind === 'no-split') ratioMatches = intent.kind === 'resize';
			else if (intent.kind === 'set-split-ratio')
				ratioMatches =
					change.split.ratio === intent.ratio &&
					(intent.expectedSplit === undefined ||
						(change.split.splitId === intent.expectedSplit.splitId &&
							change.split.firstSlotId === intent.expectedSplit.firstSlotId &&
							change.split.secondSlotId === intent.expectedSplit.secondSlotId));
			else {
				const signed =
					intent.direction === 'right' || intent.direction === 'down'
						? intent.amount
						: -intent.amount;
				ratioMatches =
					change.split.ratio === Math.max(0.1, Math.min(0.9, change.split.previousRatio + signed));
			}
			if (
				!ratioMatches ||
				adjustment.terminalContextId !== intent.terminalContextId ||
				adjustment.tabId !== intent.tabId ||
				adjustment.selected !== selected ||
				BigInt(adjustment.workbenchRevision) < BigInt(command.minimumWorkbenchRevision) ||
				(selected && adjustment.workbenchRevision !== view.workbenchRevision) ||
				view.phase === 'starting' ||
				view.phase === 'busy'
			) {
				context.addIssue({
					code: 'custom',
					message: 'Layout adjustment target, revision or ratio is inconsistent',
				});
			}
			return;
		}
		if (status.kind === 'direction-focused' || status.kind === 'direction-unchanged') {
			const { intent } = command;
			const { view } = status;
			const { selection } = view;
			if (intent.kind !== 'focus-direction') {
				context.addIssue({
					code: 'custom',
					message: 'Directional receipt requires directional intent',
				});
				return;
			}
			const selected =
				selection !== null &&
				selection.terminalContextId === intent.terminalContextId &&
				selection.tabId === intent.tabId;
			const settled = view.phase !== 'starting' && view.phase !== 'busy';
			const valid =
				status.kind === 'direction-focused'
					? settled &&
						selected &&
						selection !== null &&
						selection.panelId === status.panelId &&
						status.panelId !== intent.panelId &&
						BigInt(view.workbenchRevision) >= BigInt(command.minimumWorkbenchRevision)
					: view.phase !== 'starting' &&
						BigInt(status.layoutRevision) >= BigInt(command.minimumWorkbenchRevision) &&
						(!selected || status.layoutRevision === view.workbenchRevision);
			if (!valid)
				context.addIssue({
					code: 'custom',
					message: 'Directional target or source revision is not observed',
				});
			return;
		}
		if (status.kind === 'presentation-read') {
			const { intent } = command;
			const { presentation, view } = status;
			if (
				intent.kind !== 'presentation' ||
				!['ready', 'busy'].includes(view.phase) ||
				BigInt(presentation.workbenchRevision) < BigInt(command.minimumWorkbenchRevision) ||
				BigInt(presentation.workbenchRevision) < BigInt(view.workbenchRevision) ||
				presentation.panelId !== intent.panelId
			) {
				context.addIssue({
					code: 'custom',
					message: 'Presentation receipt does not match requested Panel or view',
				});
			}
			return;
		}
		if (status.kind === 'viewport-read') {
			const intent = command.intent,
				viewport = status.viewport,
				view = status.view;
			if (
				intent.kind !== 'viewports' ||
				!['ready', 'busy'].includes(view.phase) ||
				BigInt(viewport.workbenchRevision) < BigInt(command.minimumWorkbenchRevision) ||
				BigInt(viewport.workbenchRevision) < BigInt(view.workbenchRevision) ||
				viewport.panels.length !== intent.panelIds.length ||
				viewport.panels.some((row, index) => row.panelId !== intent.panelIds[index])
			) {
				context.addIssue({
					code: 'custom',
					message: 'Viewport receipt does not match requested Panels or view',
				});
			}
			return;
		}
		if (status.kind === 'navigation-read') {
			const { intent } = command;
			const { navigation, view } = status;
			if (
				intent.kind !== 'selections' ||
				view.phase !== 'ready' ||
				BigInt(navigation.workbenchRevision) < BigInt(command.minimumWorkbenchRevision) ||
				BigInt(navigation.workbenchRevision) < BigInt(view.workbenchRevision) ||
				navigation.contexts.length !== intent.terminalContextIds.length ||
				navigation.contexts.some(
					(entry, index) =>
						entry.terminalContextId !== intent.terminalContextIds[index] ||
						(view.selection?.terminalContextId === entry.terminalContextId &&
							view.selection.tabId !== entry.activeTabId),
				)
			) {
				context.addIssue({
					code: 'custom',
					message: 'Navigation receipt does not match requested contexts or view',
				});
			}
			return;
		}
		if (status.kind === 'terminal-attention-read') {
			const intent = command.intent;
			if (
				intent.kind !== 'terminal-agent-attention' ||
				status.attention.entries.length !== intent.bindings.length ||
				status.attention.entries.some((entry, index) => {
					const expected = intent.bindings[index];
					return (
						expected === undefined ||
						entry.binding.presentationId !== expected.presentationId ||
						entry.binding.generation !== expected.generation ||
						entry.binding.hostEpoch !== expected.hostEpoch ||
						entry.binding.hostTerminalId !== expected.hostTerminalId
					);
				})
			)
				context.addIssue({ code: 'custom', message: 'Physical Client attention binding mismatch' });
			return;
		}
		if (status.kind === 'attention-read') {
			const intent = command.intent;
			if (
				intent.kind !== 'agent-attention' ||
				status.attention.entries.length !== intent.sessionIds.length ||
				status.attention.entries.some(
					(entry, index) => entry.sessionId !== intent.sessionIds[index],
				)
			) {
				context.addIssue({
					code: 'custom',
					message: 'Attention receipt does not match requested sessions',
				});
			}
			return;
		}
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
					intent.kind !== 'focus-direction' &&
					intent.kind !== 'resize' &&
					intent.kind !== 'set-split-ratio' &&
					(intent.kind !== 'zoom' || selection?.zoomed === intent.enabled) &&
					(intent.kind !== 'overlay' || (selection?.zoomed === true && selection.panelCount > 1));
				break;
		}

		if (
			intent.kind === 'popup-open' ||
			intent.kind === 'editor-open' ||
			intent.kind === 'command-popup-open' ||
			intent.kind === 'popup-close' ||
			intent.kind === 'agent-attention' ||
			intent.kind === 'terminal-agent-attention' ||
			intent.kind === 'selections' ||
			intent.kind === 'presentation' ||
			intent.kind === 'viewports'
		) {
			context.addIssue({ code: 'custom', message: 'Read cannot acknowledge navigation' });
			return;
		}
		const observed =
			(intent.kind !== 'agent-focus' || view.phase === 'ready') &&
			BigInt(view.workbenchRevision) >= BigInt(command.minimumWorkbenchRevision) &&
			view.phase !== 'starting' &&
			view.phase !== 'busy' &&
			selection !== null &&
			selection.terminalContextId === intent.terminalContextId &&
			(intent.kind === 'context' ||
				(selection.tabId === intent.tabId &&
					(intent.kind === 'tab' ||
						('panelId' in intent && selection.panelId === intent.panelId && zoomMatches))));
		if (!observed || !zoomMatches)
			context.addIssue({ code: 'custom', message: 'Applied target is not observed' });
	})
	.readonly();
const ClientViewActionSchema = z.discriminatedUnion('kind', [
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
		sessionId: SessionIdSchema,
		action: ClientViewActionSchema,
	})
	.readonly();
export const WorkbenchClientViewResultSchema = z
	.strictObject({
		kind: z.literal('workbench.client-view.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		sessionId: SessionIdSchema,
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
