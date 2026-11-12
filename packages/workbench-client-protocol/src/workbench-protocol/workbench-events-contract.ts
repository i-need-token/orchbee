import { z } from 'zod';
import {
	ClientViewObservationSchema,
	WorkbenchClientIdSchema,
} from './workbench-client-view-contract';
import { ClientViewportObservationSchema } from './workbench-client-viewport-contract';
import { PanelAgentStateSchema, PanelAgentViewSchema } from './workbench-panel-agent-contract';
import { MetadataTokensSchema } from './workbench-workspace-metadata-contract';
import {
	WorkbenchWorktreeOutcomeSchema,
	WorkbenchWorktreeResultSchema,
} from './workbench-worktree-contract';
import {
	SessionIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchWorkspaceIdSchema,
	WorkbenchTerminalContextIdSchema,
	WorkbenchTabIdSchema,
	WorkbenchPanelIdSchema,
	TerminalPresentationIdSchema,
	WorkbenchLabelSchema,
	WorkbenchPresentationBindingSchema,
	WorkbenchPanelNodeSchema,
	WorkbenchWorkspaceSchema,
	WorkbenchSnapshotSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_EVENTS_BYTES = 256 * 1024;
const encoder = new TextEncoder();
const bounded = (value: unknown) =>
	encoder.encode(JSON.stringify(value)).byteLength <= WORKBENCH_MAXIMUM_EVENTS_BYTES;
const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	sessionId: SessionIdSchema,
};
const SemanticEventKindSchema = z.enum([
	'workspace.created',
	'workspace.updated',
	'workspace.metadata_updated',
	'workspace.renamed',
	'workspace.moved',
	'workspace.reordered',
	'workspace.closed',
	'workspace.focused',
	'worktree.created',
	'worktree.opened',
	'worktree.removed',
	'tab.created',
	'tab.closed',
	'tab.focused',
	'tab.renamed',
	'tab.moved',
	'panel.created',
	'panel.closed',
	'panel.updated',
	'panel.focused',
	'panel.moved',
	'panel.exited',
	'panel.agent_detected',
	'panel.agent_status_changed',
	'panel.output_changed',
	'panel.output_matched',
	'panel.scroll_changed',
	'layout.updated',
]);
const SemanticEventCursorSchema = z
	.strictObject({ epoch: WorkbenchRequestIdSchema, sequence: WorkbenchRevisionSchema })
	.readonly();
const eventPosition = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('journal'),
		cursor: SemanticEventCursorSchema.refine((cursor) => cursor.sequence !== '0'),
	}),
	z.strictObject({
		kind: z.literal('probe'),
		subscriptionId: WorkbenchRequestIdSchema,
		sequence: WorkbenchRevisionSchema.refine((value) => value !== '0'),
	}),
]);
const SemanticPaneSchema = z
	.strictObject({
		terminalContextId: WorkbenchTerminalContextIdSchema,
		tabId: WorkbenchTabIdSchema,
		panelId: WorkbenchPanelIdSchema,
		presentationId: TerminalPresentationIdSchema,
		binding: WorkbenchPresentationBindingSchema.nullable(),
		label: WorkbenchLabelSchema,
	})
	.readonly();
const worktree = z
	.strictObject({
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		operationId: WorkbenchRequestIdSchema,
		outcome: WorkbenchWorktreeOutcomeSchema,
	})
	.refine(
		(value) =>
			WorkbenchWorktreeResultSchema.safeParse({
				kind: 'workbench.worktree.operation.result',
				...value,
			}).success,
	);
const eventData = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('workspace'),
		workspace: WorkbenchWorkspaceSchema,
	}),
	z.strictObject({
		kind: z.literal('workspace-order'),
		workspaceIds: z
			.array(WorkbenchWorkspaceIdSchema)
			.max(4096)
			.refine((ids) => new Set(ids).size === ids.length),
	}),
	z.strictObject({
		kind: z.literal('workspace-metadata'),
		workspaceId: WorkbenchWorkspaceIdSchema,
		tokens: MetadataTokensSchema,
	}),
	z.strictObject({
		kind: z.literal('tab'),
		terminalContextId: WorkbenchTerminalContextIdSchema,
		tabId: WorkbenchTabIdSchema,
		label: WorkbenchLabelSchema,
		index: z.number().int().min(0).max(4294967295),
	}),
	z.strictObject({ kind: z.literal('panel'), panel: SemanticPaneSchema }),
	z.strictObject({
		kind: z.literal('layout'),
		terminalContextId: WorkbenchTerminalContextIdSchema,
		tabId: WorkbenchTabIdSchema,
		root: WorkbenchPanelNodeSchema,
	}),
	z.strictObject({ kind: z.literal('worktree'), result: worktree }),
	z.strictObject({
		kind: z.literal('client'),
		clientId: WorkbenchClientIdSchema,
		view: ClientViewObservationSchema,
		workspaceId: WorkbenchWorkspaceIdSchema.nullable(),
	}),
	z.strictObject({
		kind: z.literal('agent'),
		panel: SemanticPaneSchema,
		report: PanelAgentViewSchema,
		exited: z.literal(false),
	}),
	z.strictObject({
		kind: z.literal('output'),
		panel: SemanticPaneSchema,
		contentGeneration: WorkbenchRevisionSchema.refine((value) => value !== '0'),
		contentRevision: WorkbenchRevisionSchema,
		matchedLine: z
			.string()
			.refine((value) => !value.includes('\0') && encoder.encode(value).byteLength <= 64 * 1024)
			.nullable(),
	}),
	z.strictObject({
		kind: z.literal('exited'),
		panel: SemanticPaneSchema,
		lifecycleRevision: WorkbenchRevisionSchema,
	}),
	z.strictObject({
		kind: z.literal('scroll'),
		clientId: WorkbenchClientIdSchema,
		panel: SemanticPaneSchema,
		observation: ClientViewportObservationSchema,
	}),
]);
const SemanticEventSchema = z
	.strictObject({
		workspaceId: WorkbenchWorkspaceIdSchema.nullable(),
		position: eventPosition,
		workbenchRevision: WorkbenchRevisionSchema,
		event: SemanticEventKindSchema,
		data: eventData,
	})
	.refine((value) => {
		const data = value.data;
		switch (data.kind) {
			case 'workspace':
				return (
					value.workspaceId === data.workspace.workspaceId &&
					[
						'workspace.created',
						'workspace.updated',
						'workspace.renamed',
						'workspace.moved',
						'workspace.closed',
					].includes(value.event)
				);
			case 'workspace-order':
				return value.workspaceId === null && value.event === 'workspace.reordered';
			case 'workspace-metadata':
				return (
					value.workspaceId === data.workspaceId && value.event === 'workspace.metadata_updated'
				);
			case 'tab':
				return ['tab.created', 'tab.closed', 'tab.renamed', 'tab.moved'].includes(value.event);
			case 'panel':
				return ['panel.created', 'panel.closed', 'panel.updated', 'panel.moved'].includes(
					value.event,
				);
			case 'layout':
				return (
					value.event === 'layout.updated' &&
					WorkbenchSnapshotSchema.safeParse({
						protocolVersion: 1,
						sessionId: '1'.repeat(32),
						revision: value.workbenchRevision,
						navigation: {
							directories: [],
							projects: [],
							workspaces: [
								{
									workspaceId: data.terminalContextId,
									label: 'Layout',
									projectId: null,
									primaryDirectoryId: null,
									terminalContextId: data.terminalContextId,
								},
							],
						},
						terminalContexts: [
							{
								terminalContextId: data.terminalContextId,
								groups: [],
								tabs: [
									{
										tabId: data.tabId,
										label: 'Layout',
										pinned: false,
										automaticLabel: false,
										groupId: null,
										root: data.root,
									},
								],
							},
						],
					}).success
				);
			case 'worktree':
				return (
					data.result.outcome.kind === 'available' &&
					((['worktree.created', 'worktree.opened'].includes(value.event) &&
						data.result.outcome.status.kind === 'opened') ||
						(value.event === 'worktree.removed' && data.result.outcome.status.kind === 'removed'))
				);
			case 'client':
				return (
					value.workspaceId === data.workspaceId &&
					['workspace.focused', 'tab.focused', 'panel.focused', 'layout.updated'].includes(
						value.event,
					)
				);
			case 'agent':
				return ['panel.agent_detected', 'panel.agent_status_changed', 'panel.updated'].includes(
					value.event,
				);
			case 'output':
				return (
					data.panel.binding !== null &&
					['panel.output_changed', 'panel.output_matched', 'panel.updated'].includes(value.event) &&
					(value.event !== 'panel.output_matched' || data.matchedLine !== null)
				);
			case 'exited':
				return data.panel.binding !== null && value.event === 'panel.exited';
			case 'scroll':
				return value.event === 'panel.scroll_changed';
		}
	})
	.readonly();
const scope = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('all') }),
	z.strictObject({ kind: z.literal('workspace'), workspaceId: WorkbenchWorkspaceIdSchema }),
	z.strictObject({
		kind: z.literal('terminal-context'),
		terminalContextId: WorkbenchTerminalContextIdSchema,
	}),
	z.strictObject({ kind: z.literal('tab'), tabId: WorkbenchTabIdSchema }),
	z.strictObject({ kind: z.literal('panel'), panelId: WorkbenchPanelIdSchema }),
	z.strictObject({ kind: z.literal('client'), clientId: WorkbenchClientIdSchema }),
]);
const condition = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('any') }),
	z.strictObject({ kind: z.literal('label'), label: WorkbenchLabelSchema }),
	z.strictObject({ kind: z.literal('output-revision'), minimumRevision: WorkbenchRevisionSchema }),
	z.strictObject({
		kind: z.literal('agent'),
		agent: z
			.string()
			.min(1)
			.refine((value) => encoder.encode(value).byteLength <= 241 && !/\p{Cc}/u.test(value)),
	}),
	z.strictObject({ kind: z.literal('agent-state'), state: PanelAgentStateSchema }),
]);
const target = z.strictObject({
	panelId: WorkbenchPanelIdSchema,
	presentationId: TerminalPresentationIdSchema.nullable(),
});
const matchValue = z
	.string()
	.min(1)
	.refine((value) => encoder.encode(value).byteLength <= 4096 && !value.includes('\0'));
const SemanticEventFilterSchema = z
	.discriminatedUnion('kind', [
		z.strictObject({ kind: z.literal('event'), event: SemanticEventKindSchema, scope, condition }),
		z.strictObject({
			kind: z.literal('output-matched'),
			target,
			source: z.enum(['recent', 'recent-unwrapped', 'history', 'viewport', 'detection']),
			lines: z.number().int().min(1).max(4096),
			matcher: z.discriminatedUnion('kind', [
				z.strictObject({ kind: z.literal('substring'), value: matchValue }),
				z.strictObject({ kind: z.literal('regex'), value: matchValue }),
			]),
			stripAnsi: z.boolean(),
			clientId: WorkbenchClientIdSchema.nullable(),
		}),
		z.strictObject({
			kind: z.literal('agent-status-changed'),
			target,
			state: PanelAgentStateSchema.nullable(),
		}),
		z.strictObject({
			kind: z.literal('scroll-changed'),
			target,
			clientId: WorkbenchClientIdSchema,
		}),
	])
	.refine((filter) => {
		if (filter.kind === 'output-matched')
			return filter.source !== 'viewport' || filter.clientId !== null;
		if (filter.kind !== 'event') return true;
		switch (filter.condition.kind) {
			case 'any':
				return !['panel.output_matched', 'panel.scroll_changed'].includes(filter.event);
			case 'label':
				return ['workspace.renamed', 'tab.renamed', 'panel.updated'].includes(filter.event);
			case 'output-revision':
				return filter.event === 'panel.output_changed';
			case 'agent':
				return filter.event === 'panel.agent_detected';
			case 'agent-state':
				return filter.event === 'panel.agent_status_changed';
		}
	})
	.readonly();
const subscription = { subscriptionId: WorkbenchRequestIdSchema };
export const SemanticEventsRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.events'),
		...correlation,
		action: z.discriminatedUnion('kind', [
			z.strictObject({
				kind: z.literal('subscribe'),
				...subscription,
				filters: z.array(SemanticEventFilterSchema).min(1).max(32),
				after: SemanticEventCursorSchema.nullable(),
			}),
			z.strictObject({
				kind: z.literal('read'),
				...subscription,
				limit: z.number().int().min(1).max(32),
			}),
			z.strictObject({
				kind: z.literal('wait'),
				...subscription,
				timeoutMilliseconds: z.number().int().min(0).max(1000),
			}),
			z.strictObject({ kind: z.literal('close'), ...subscription }),
		]),
	})
	.refine(bounded)
	.readonly();
const gap = z
	.strictObject({ after: WorkbenchRevisionSchema, through: WorkbenchRevisionSchema })
	.refine((value) => BigInt(value.after) < BigInt(value.through));
const cursor = { cursor: SemanticEventCursorSchema };
const outcome = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('opened'), ...subscription, ...cursor, existing: z.boolean() }),
	z.strictObject({
		kind: z.literal('events'),
		...subscription,
		...cursor,
		events: z.array(SemanticEventSchema).max(32),
		gap: gap.nullable(),
		more: z.boolean(),
	}),
	z.strictObject({
		kind: z.literal('matched'),
		...subscription,
		...cursor,
		event: SemanticEventSchema,
	}),
	z.strictObject({ kind: z.literal('timed-out'), ...subscription, ...cursor }),
	z.strictObject({ kind: z.literal('gap'), ...subscription, ...cursor, gap }),
	z.strictObject({ kind: z.literal('closed'), ...subscription }),
	z.strictObject({
		kind: z.literal('failed'),
		reason: z.enum([
			'denied',
			'invalid-intent',
			'invalid-regex',
			'session-mismatch',
			'unavailable',
			'busy',
			'conflict',
			'not-found',
			'cursor-epoch',
			'cursor-ahead',
			'source-changed',
			'source-unavailable',
			'physical-unknown',
			'revision-exhausted',
		]),
	}),
]);
export const SemanticEventsResultSchema = z
	.strictObject({ kind: z.literal('workbench.events.result'), ...correlation, outcome })
	.refine(bounded)
	.refine((value) => {
		const result = value.outcome;
		if (
			result.kind === 'closed' ||
			result.kind === 'failed' ||
			result.kind === 'opened' ||
			result.kind === 'timed-out'
		)
			return true;
		if (result.kind === 'gap') return BigInt(result.gap.through) <= BigInt(result.cursor.sequence);
		if (
			result.kind === 'events' &&
			result.gap !== null &&
			BigInt(result.gap.through) > BigInt(result.cursor.sequence)
		)
			return false;
		let journal = 0n,
			probe = 0n;
		const events = result.kind === 'matched' ? [result.event] : result.events;
		return events.every((event) => {
			const position = event.position;
			if (position.kind === 'probe') {
				const sequence = BigInt(position.sequence);
				if (position.subscriptionId !== result.subscriptionId || sequence <= probe) return false;
				probe = sequence;
				return true;
			}
			const sequence = BigInt(position.cursor.sequence);
			if (
				position.cursor.epoch !== result.cursor.epoch ||
				sequence <= journal ||
				sequence > BigInt(result.cursor.sequence)
			)
				return false;
			journal = sequence;
			return true;
		});
	})
	.readonly();
