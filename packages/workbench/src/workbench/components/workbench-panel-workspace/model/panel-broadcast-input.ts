import type { TerminalPresentationId } from '../../../../terminal-presentation';
import type {
	WorkbenchTerminalBroadcastInput,
	WorkbenchTerminalBroadcastInputSink,
} from '../../../types/workbench-terminal-surface';
import {
	collectPanelLeaves,
	getDisplayedTerminal,
	type PanelLayoutNode,
	type PanelWorkspaceState,
} from './panel-workspace-state';

interface PanelBroadcastTarget {
	readonly panelId: string;
	readonly presentationId: TerminalPresentationId;
	readonly sinkRevision: string;
}

export interface PanelBroadcastTargetSnapshot {
	readonly sourcePanelId: string;
	readonly tabId: string;
	readonly targets: readonly PanelBroadcastTarget[];
	readonly topology: string;
}

export interface PanelBroadcastInputEnvelope {
	readonly epoch: number;
	readonly input: WorkbenchTerminalBroadcastInput;
	readonly sequence: number;
}

type PanelBroadcastStopReason = 'delivery-failed' | 'partial-delivery' | 'target-invalidated';

export type PanelBroadcastSessionState =
	| { readonly kind: 'idle' }
	| {
			readonly epoch: number;
			readonly hidden: boolean;
			readonly inFlight: PanelBroadcastInputEnvelope | null;
			readonly kind: 'active';
			readonly queue: readonly PanelBroadcastInputEnvelope[];
			readonly snapshot: PanelBroadcastTargetSnapshot;
	  }
	| {
			readonly deliveredTargetCount: number;
			readonly epoch: number;
			readonly kind: 'stopped';
			readonly reason: PanelBroadcastStopReason;
			readonly targetCount: number;
	  };

export type PanelBroadcastCommand =
	| {
			readonly envelope: PanelBroadcastInputEnvelope;
			readonly kind: 'dispatch';
			readonly snapshot: PanelBroadcastTargetSnapshot;
	  }
	| {
			readonly kind: 'restore-focus';
			readonly sourcePanelId: string;
			readonly tabId: string;
	  };

export type PanelBroadcastEvent =
	| {
			readonly epoch: number;
			readonly snapshot: PanelBroadcastTargetSnapshot;
			readonly type: 'started';
	  }
	| { readonly envelope: PanelBroadcastInputEnvelope; readonly type: 'enqueued' }
	| { readonly type: 'dispatch-requested' }
	| {
			readonly deliveredTargetCount: number;
			readonly epoch: number;
			readonly sequence: number;
			readonly targetCount: number;
			readonly type: 'delivery-completed';
	  }
	| {
			readonly epoch: number;
			readonly sequence: number;
			readonly type: 'delivery-invalidated';
	  }
	| { readonly type: 'visibility-toggled' }
	| { readonly type: 'ended' }
	| { readonly type: 'invalidated' }
	| { readonly type: 'dismissed' };

export interface PanelBroadcastTransition {
	readonly command: PanelBroadcastCommand | null;
	readonly state: PanelBroadcastSessionState;
}

type PanelBroadcastStartResolution =
	| { readonly kind: 'ready'; readonly snapshot: PanelBroadcastTargetSnapshot }
	| {
			readonly kind: 'unavailable';
			readonly reason:
				| 'active-tab-unavailable'
				| 'conflicting-interaction'
				| 'invalid-panel-order'
				| 'panel-maximized'
				| 'requires-multiple-panels'
				| 'source-panel-unavailable'
				| 'target-not-running'
				| 'target-not-write-ready';
	  };

export interface PanelBroadcastEnvironment {
	readonly activeTabId: string | null;
	readonly conflictingInteraction: boolean;
	readonly sinks: ReadonlyMap<TerminalPresentationId, WorkbenchTerminalBroadcastInputSink>;
	readonly state: PanelWorkspaceState;
}

type PanelBroadcastPreflight =
	| {
			readonly kind: 'ready';
			readonly targets: readonly {
				readonly sink: WorkbenchTerminalBroadcastInputSink;
				readonly target: PanelBroadcastTarget;
			}[];
	  }
	| { readonly kind: 'invalidated' };

type PanelBroadcastDispatchResult =
	| {
			readonly deliveredTargetCount: number;
			readonly epoch: number;
			readonly sequence: number;
			readonly targetCount: number;
			readonly type: 'delivery-completed';
	  }
	| {
			readonly epoch: number;
			readonly sequence: number;
			readonly type: 'delivery-invalidated';
	  };

export function resolvePanelBroadcastStart(
	environment: PanelBroadcastEnvironment,
	sourcePanelId: string,
): PanelBroadcastStartResolution {
	if (environment.conflictingInteraction) {
		return { kind: 'unavailable', reason: 'conflicting-interaction' };
	}
	const activeTabId = environment.activeTabId;
	if (activeTabId === null) {
		return { kind: 'unavailable', reason: 'active-tab-unavailable' };
	}
	const activeTab = environment.state.tabs.find((tab) => tab.tabId === activeTabId);
	if (activeTab === undefined) {
		return { kind: 'unavailable', reason: 'active-tab-unavailable' };
	}
	if (activeTab.maximizedPanelId !== null) {
		return { kind: 'unavailable', reason: 'panel-maximized' };
	}
	const panels = collectPanelLeaves(activeTab.root);
	if (panels.length < 2) {
		return { kind: 'unavailable', reason: 'requires-multiple-panels' };
	}
	if (!panels.some((panel) => panel.id === sourcePanelId)) {
		return { kind: 'unavailable', reason: 'source-panel-unavailable' };
	}
	if (!panels.some((panel) => panel.id === activeTab.focusedPanelId)) {
		return { kind: 'unavailable', reason: 'invalid-panel-order' };
	}
	if (new Set(panels.map((panel) => panel.id)).size !== panels.length) {
		return { kind: 'unavailable', reason: 'invalid-panel-order' };
	}

	const targets: PanelBroadcastTarget[] = [];
	const presentationIds = new Set<TerminalPresentationId>();
	for (const panel of panels) {
		const terminal = getDisplayedTerminal(panel);
		if (terminal.lifecycle.kind !== 'running') {
			return { kind: 'unavailable', reason: 'target-not-running' };
		}
		if (presentationIds.has(terminal.presentationId)) {
			return { kind: 'unavailable', reason: 'invalid-panel-order' };
		}
		presentationIds.add(terminal.presentationId);
		const readiness = environment.sinks.get(terminal.presentationId)?.getReadiness();
		if (readiness?.kind !== 'ready') {
			return { kind: 'unavailable', reason: 'target-not-write-ready' };
		}
		targets.push({
			panelId: panel.id,
			presentationId: terminal.presentationId,
			sinkRevision: readiness.revision,
		});
	}

	return {
		kind: 'ready',
		snapshot: {
			sourcePanelId: activeTab.focusedPanelId,
			tabId: activeTabId,
			targets,
			topology: panelTopology(activeTab.root),
		},
	};
}

export function preflightPanelBroadcast(
	environment: PanelBroadcastEnvironment,
	snapshot: PanelBroadcastTargetSnapshot,
): PanelBroadcastPreflight {
	const resolution = resolvePanelBroadcastStart(environment, snapshot.sourcePanelId);
	if (resolution.kind !== 'ready' || !samePanelBroadcastSnapshot(resolution.snapshot, snapshot)) {
		return { kind: 'invalidated' };
	}
	const targets = snapshot.targets.map((target) => {
		const sink = environment.sinks.get(target.presentationId);
		const readiness = sink?.getReadiness();
		return sink !== undefined &&
			readiness?.kind === 'ready' &&
			readiness.revision === target.sinkRevision
			? { sink, target }
			: null;
	});
	return targets.some((target) => target === null)
		? { kind: 'invalidated' }
		: {
				kind: 'ready',
				targets: targets.filter(
					(target): target is NonNullable<(typeof targets)[number]> => target !== null,
				),
			};
}

export async function dispatchPanelBroadcastInput(
	environment: PanelBroadcastEnvironment,
	snapshot: PanelBroadcastTargetSnapshot,
	envelope: PanelBroadcastInputEnvelope,
): Promise<PanelBroadcastDispatchResult> {
	const preflight = preflightPanelBroadcast(environment, snapshot);
	if (preflight.kind === 'invalidated') {
		return { epoch: envelope.epoch, sequence: envelope.sequence, type: 'delivery-invalidated' };
	}
	const results = await Promise.all(
		preflight.targets.map(async ({ sink, target }) => {
			try {
				return await sink.send(envelope.input, target.sinkRevision);
			} catch {
				return { kind: 'failed' } as const;
			}
		}),
	);
	return {
		deliveredTargetCount: results.filter((result) => result.kind === 'sent').length,
		epoch: envelope.epoch,
		sequence: envelope.sequence,
		targetCount: preflight.targets.length,
		type: 'delivery-completed',
	};
}

export function transitionPanelBroadcastInput(
	state: PanelBroadcastSessionState,
	event: PanelBroadcastEvent,
): PanelBroadcastTransition {
	switch (event.type) {
		case 'started':
			if (state.kind !== 'idle') return unchanged(state);
			return {
				command: null,
				state: {
					epoch: event.epoch,
					hidden: false,
					inFlight: null,
					kind: 'active',
					queue: [],
					snapshot: event.snapshot,
				},
			};
		case 'enqueued':
			if (state.kind !== 'active' || event.envelope.epoch !== state.epoch) {
				return unchanged(state);
			}
			return { command: null, state: { ...state, queue: [...state.queue, event.envelope] } };
		case 'dispatch-requested': {
			if (state.kind !== 'active' || state.inFlight !== null) return unchanged(state);
			const envelope = state.queue[0];
			if (envelope === undefined) return unchanged(state);
			return {
				command: { envelope, kind: 'dispatch', snapshot: state.snapshot },
				state: { ...state, inFlight: envelope, queue: state.queue.slice(1) },
			};
		}
		case 'delivery-completed': {
			if (!matchesInFlight(state, event.epoch, event.sequence)) return unchanged(state);
			if (
				event.targetCount === state.snapshot.targets.length &&
				event.deliveredTargetCount === event.targetCount
			) {
				return { command: null, state: { ...state, inFlight: null } };
			}
			return stopped(
				state,
				event.deliveredTargetCount > 0 ? 'partial-delivery' : 'delivery-failed',
				event.deliveredTargetCount,
			);
		}
		case 'delivery-invalidated':
			return matchesInFlight(state, event.epoch, event.sequence)
				? stopped(state, 'target-invalidated', 0)
				: unchanged(state);
		case 'visibility-toggled':
			return state.kind === 'active'
				? { command: null, state: { ...state, hidden: !state.hidden } }
				: unchanged(state);
		case 'ended':
			return state.kind === 'active'
				? {
						command: {
							kind: 'restore-focus',
							sourcePanelId: state.snapshot.sourcePanelId,
							tabId: state.snapshot.tabId,
						},
						state: { kind: 'idle' },
					}
				: unchanged(state);
		case 'invalidated':
			return state.kind === 'active' ? stopped(state, 'target-invalidated', 0) : unchanged(state);
		case 'dismissed':
			return state.kind === 'stopped'
				? { command: null, state: { kind: 'idle' } }
				: unchanged(state);
	}
}

function samePanelBroadcastSnapshot(
	left: PanelBroadcastTargetSnapshot,
	right: PanelBroadcastTargetSnapshot,
): boolean {
	return (
		left.tabId === right.tabId &&
		left.sourcePanelId === right.sourcePanelId &&
		left.topology === right.topology &&
		left.targets.length === right.targets.length &&
		left.targets.every((target, index) => {
			const candidate = right.targets[index];
			return (
				candidate !== undefined &&
				target.panelId === candidate.panelId &&
				target.presentationId === candidate.presentationId &&
				target.sinkRevision === candidate.sinkRevision
			);
		})
	);
}

function matchesInFlight(
	state: PanelBroadcastSessionState,
	epoch: number,
	sequence: number,
): state is Extract<PanelBroadcastSessionState, { readonly kind: 'active' }> {
	return state.kind === 'active' && state.epoch === epoch && state.inFlight?.sequence === sequence;
}

function stopped(
	state: Extract<PanelBroadcastSessionState, { readonly kind: 'active' }>,
	reason: PanelBroadcastStopReason,
	deliveredTargetCount: number,
): PanelBroadcastTransition {
	return {
		command: null,
		state: {
			deliveredTargetCount,
			epoch: state.epoch,
			kind: 'stopped',
			reason,
			targetCount: state.snapshot.targets.length,
		},
	};
}

function unchanged(state: PanelBroadcastSessionState): PanelBroadcastTransition {
	return { command: null, state };
}

function panelTopology(node: PanelLayoutNode): string {
	if (node.kind === 'panel') return `panel:${node.id}`;
	return `split:${node.id}:${node.orientation}(${node.children
		.map((child) => `${child.id}:${panelTopology(child.node)}`)
		.join('|')})`;
}
