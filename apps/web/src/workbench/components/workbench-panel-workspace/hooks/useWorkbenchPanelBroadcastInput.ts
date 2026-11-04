import type { ActionDisposable } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import type {
	WorkbenchTerminalBroadcastInput,
	WorkbenchTerminalBroadcastInputSink,
} from '../../../types/workbench-terminal-surface';
import type { PanelWorkspaceStore } from '../model/PanelWorkspaceStore';
import type { PanelWorkspaceState } from '../model/panel-workspace-state';
import {
	dispatchPanelBroadcastInput,
	preflightPanelBroadcast,
	resolvePanelBroadcastStart,
	transitionPanelBroadcastInput,
	type PanelBroadcastCommand,
	type PanelBroadcastEnvironment,
	type PanelBroadcastEvent,
	type PanelBroadcastSessionState,
	type PanelBroadcastTransition,
} from '../model/panel-broadcast-input';

interface UseWorkbenchPanelBroadcastInputOptions {
	readonly activeTabId: string | null;
	readonly conflictingInteraction: boolean;
	readonly store: PanelWorkspaceStore;
	readonly workspaceState: PanelWorkspaceState;
	readonly workspaceReference: React.RefObject<HTMLDivElement | null>;
}

interface WorkbenchPanelBroadcastInputController {
	readonly state: PanelBroadcastSessionState;
	bindSink(
		presentationId: TerminalPresentationId,
		sink: WorkbenchTerminalBroadcastInputSink,
	): ActionDisposable;
	canStart(panelId: string): boolean;
	dismiss(): void;
	end(): void;
	enqueue(input: WorkbenchTerminalBroadcastInput): void;
	start(panelId: string): boolean;
	toggleVisibility(): void;
}

export function useWorkbenchPanelBroadcastInput({
	activeTabId,
	conflictingInteraction,
	store,
	workspaceState,
	workspaceReference,
}: UseWorkbenchPanelBroadcastInputOptions): WorkbenchPanelBroadcastInputController {
	const sinks = React.useRef(
		new Map<TerminalPresentationId, WorkbenchTerminalBroadcastInputSink>(),
	);
	const sinkTokens = React.useRef(new Map<TerminalPresentationId, symbol>());
	const [sinkRevision, setSinkRevision] = React.useState(0);
	const [state, setState] = React.useState<PanelBroadcastSessionState>({ kind: 'idle' });
	const epoch = React.useRef(0);
	const sequence = React.useRef(0);
	const stateReference = React.useRef(state);
	// Events own accepted state; a delayed React projection must never roll it back.
	const commitEvent = useMemoizedFn((event: PanelBroadcastEvent): PanelBroadcastTransition => {
		const transition = transitionPanelBroadcastInput(stateReference.current, event);
		stateReference.current = transition.state;
		setState(transition.state);
		return transition;
	});

	const environment = useMemoizedFn(
		(): PanelBroadcastEnvironment => ({
			activeTabId: store.getActiveTabId(),
			conflictingInteraction,
			sinks: sinks.current,
			state: store.getSnapshot(),
		}),
	);
	const restoreFocus = useMemoizedFn((tabId: string, panelId: string): void => {
		if (store.getActiveTabId() !== tabId) return;
		const tab = store.getSnapshot().tabs.find((candidate) => candidate.tabId === tabId);
		if (tab?.focusedPanelId !== panelId) return;
		const workspaceElement = workspaceReference.current;
		if (workspaceElement === null) return;
		const viewport = [
			...workspaceElement.querySelectorAll<HTMLElement>(
				'[data-presentation-mode="active"] [data-slot="workbench-terminal-viewport"]',
			),
		].find((candidate) => candidate.dataset['panelId'] === panelId);
		const focusTarget =
			viewport?.querySelector<HTMLElement>('textarea, [role="textbox"]') ?? viewport;
		focusTarget?.focus({ preventScroll: true });
	});
	const executeCommand = useMemoizedFn((command: PanelBroadcastCommand | null): void => {
		if (command === null) return;
		if (command.kind === 'restore-focus') {
			const focusEpoch = epoch.current;
			globalThis.queueMicrotask(() => {
				if (epoch.current !== focusEpoch || stateReference.current.kind !== 'idle') return;
				restoreFocus(command.tabId, command.sourcePanelId);
			});
			return;
		}
		void dispatchPanelBroadcastInput(environment(), command.snapshot, command.envelope).then(
			(event) => {
				commitEvent(event);
			},
			() => {
				commitEvent({
					deliveredTargetCount: 0,
					epoch: command.envelope.epoch,
					sequence: command.envelope.sequence,
					targetCount: command.snapshot.targets.length,
					type: 'delivery-completed',
				});
			},
		);
	});

	React.useEffect(() => {
		if (state.kind !== 'active' || state.inFlight !== null || state.queue.length === 0) return;
		const transition = commitEvent({ type: 'dispatch-requested' });
		executeCommand(transition.command);
	}, [commitEvent, executeCommand, state]);

	React.useLayoutEffect(() => {
		const current = stateReference.current;
		if (current.kind !== 'active') return;
		if (preflightPanelBroadcast(environment(), current.snapshot).kind === 'ready') return;
		commitEvent({ type: 'invalidated' });
	}, [
		activeTabId,
		commitEvent,
		conflictingInteraction,
		environment,
		sinkRevision,
		state,
		store,
		workspaceState,
	]);

	const bindSink = useMemoizedFn(
		(
			presentationId: TerminalPresentationId,
			sink: WorkbenchTerminalBroadcastInputSink,
		): ActionDisposable => {
			const token = Symbol(presentationId);
			const retire = (): void => {
				if (sinkTokens.current.get(presentationId) !== token) return;
				sinks.current.delete(presentationId);
				sinkTokens.current.delete(presentationId);
				setSinkRevision((current) => current + 1);
			};
			sinks.current.set(presentationId, sink);
			sinkTokens.current.set(presentationId, token);
			setSinkRevision((current) => current + 1);
			let subscription: ActionDisposable;
			try {
				subscription = sink.subscribe(() => {
					if (sinkTokens.current.get(presentationId) === token) {
						setSinkRevision((current) => current + 1);
					}
				});
			} catch (error) {
				retire();
				throw error;
			}
			let disposed = false;
			return {
				dispose: () => {
					if (disposed) return;
					disposed = true;
					retire();
					subscription.dispose();
				},
			};
		},
	);
	const canStart = useMemoizedFn((panelId: string): boolean => {
		return (
			stateReference.current.kind === 'idle' &&
			resolvePanelBroadcastStart(environment(), panelId).kind === 'ready'
		);
	});
	const start = useMemoizedFn((panelId: string): boolean => {
		if (stateReference.current.kind !== 'idle') return false;
		const resolution = resolvePanelBroadcastStart(environment(), panelId);
		if (resolution.kind !== 'ready') return false;
		epoch.current += 1;
		sequence.current = 0;
		commitEvent({
			epoch: epoch.current,
			snapshot: resolution.snapshot,
			type: 'started',
		});
		return true;
	});
	const enqueue = useMemoizedFn((input: WorkbenchTerminalBroadcastInput): void => {
		const current = stateReference.current;
		if (current.kind !== 'active') return;
		sequence.current += 1;
		commitEvent({
			envelope: { epoch: current.epoch, input, sequence: sequence.current },
			type: 'enqueued',
		});
	});
	const transitionWithoutPayload = useMemoizedFn(
		(type: 'dismissed' | 'ended' | 'visibility-toggled'): void => {
			const transition = commitEvent({ type });
			executeCommand(transition.command);
		},
	);

	return {
		bindSink,
		canStart,
		dismiss: () => transitionWithoutPayload('dismissed'),
		end: () => transitionWithoutPayload('ended'),
		enqueue,
		start,
		state,
		toggleVisibility: () => transitionWithoutPayload('visibility-toggled'),
	};
}
