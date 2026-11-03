import type { ActionExecution } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchActiveLocation } from '../../project-workspace';
import {
	INITIAL_WORKBENCH_AGENT_NAVIGATION_STATE,
	resolveWorkbenchAgentNavigationProjection,
	transitionWorkbenchAgentNavigation,
	type WorkbenchAgentNavigationCommand,
	type WorkbenchAgentNavigationEvent,
	type WorkbenchAgentNavigationState,
} from '../model/workbench-agent-navigation';
import type { WorkbenchAgentNavigationInput } from '../model/project-workbench-agent-overview';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAgentSource } from '../types/workbench-agent-observation';
import { useAgentSnapshot } from './useAgentSnapshot';

interface UseWorkbenchAgentNavigationOptions {
	readonly activeLocation: WorkbenchActiveLocation;
	readonly announceUnavailable: () => void;
	readonly invokeAgentNavigation: (input: WorkbenchAgentNavigationInput) => ActionExecution;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly requestTerminalFocus: () => boolean;
	readonly source: WorkbenchAgentSource | undefined;
}

export function useWorkbenchAgentNavigation({
	activeLocation,
	announceUnavailable,
	invokeAgentNavigation,
	projectWorkspaceRuntime,
	requestTerminalFocus,
	source,
}: UseWorkbenchAgentNavigationOptions): (input: WorkbenchAgentNavigationInput) => void {
	const snapshot = useAgentSnapshot(source);
	const terminalTopologyRevision = React.useSyncExternalStore(
		projectWorkspaceRuntime.subscribeTerminalTopology,
		projectWorkspaceRuntime.getTerminalTopologyRevision,
		projectWorkspaceRuntime.getTerminalTopologyRevision,
	);
	const modelReference = React.useRef<WorkbenchAgentNavigationState>(
		INITIAL_WORKBENCH_AGENT_NAVIGATION_STATE,
	);
	const requestSequenceReference = React.useRef(0);
	const applyEventReference = React.useRef<(event: WorkbenchAgentNavigationEvent) => void>(
		() => undefined,
	);

	const executeCommand = useMemoizedFn((command: WorkbenchAgentNavigationCommand): void => {
		if (command.kind === 'request-terminal-focus') {
			requestTerminalFocus();
			return;
		}
		if (command.kind === 'announce-unavailable') {
			announceUnavailable();
			return;
		}
		let execution: ActionExecution;
		try {
			execution = invokeAgentNavigation(command.input);
		} catch {
			applyEventReference.current({
				kind: 'action-settled',
				outcome: 'rejected',
				requestId: command.requestId,
			});
			return;
		}
		void execution.completion.then(
			(result) =>
				applyEventReference.current({
					kind: 'action-settled',
					outcome:
						result.kind === 'completed' || result.kind === 'unchanged' ? 'accepted' : 'rejected',
					requestId: command.requestId,
				}),
			() =>
				applyEventReference.current({
					kind: 'action-settled',
					outcome: 'rejected',
					requestId: command.requestId,
				}),
		);
	});
	const applyEvent = useMemoizedFn((event: WorkbenchAgentNavigationEvent): void => {
		const transition = transitionWorkbenchAgentNavigation(modelReference.current, event);
		modelReference.current = transition.state;
		for (const command of transition.commands) executeCommand(command);
	});
	applyEventReference.current = applyEvent;
	const observeProjection = useMemoizedFn((): void => {
		const state = modelReference.current;
		if (state.kind !== 'waiting') return;
		applyEvent({
			activeLocation,
			kind: 'projection-observed',
			projection: resolveWorkbenchAgentNavigationProjection(
				state.input,
				source?.getSnapshot() ?? snapshot,
				projectWorkspaceRuntime,
			),
		});
	});

	React.useEffect(() => {
		applyEvent({ kind: 'mount' });
		return () => applyEvent({ kind: 'dispose' });
	}, [applyEvent]);
	React.useLayoutEffect(() => {
		observeProjection();
	}, [activeLocation, observeProjection, snapshot, terminalTopologyRevision]);

	return useMemoizedFn((input: WorkbenchAgentNavigationInput): void => {
		requestSequenceReference.current += 1;
		applyEvent({
			input,
			kind: 'navigate',
			originLocation: activeLocation,
			requestId: requestSequenceReference.current,
		});
		observeProjection();
	});
}
