import type { ActionExecution } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchActiveLocation } from '../../project-workspace';
import {
	INITIAL_WORKBENCH_STANDALONE_TERMINAL_ACTIVATION_STATE,
	transitionWorkbenchStandaloneTerminalActivation,
	type WorkbenchStandaloneTerminalActivationCommand,
	type WorkbenchStandaloneTerminalActivationEvent,
	type WorkbenchStandaloneTerminalActivationOrigin,
	type WorkbenchStandaloneTerminalActivationState,
} from '../model/workbench-standalone-terminal-activation';

interface UseWorkbenchStandaloneTerminalActivationOptions {
	readonly activeLocation: WorkbenchActiveLocation;
	readonly invokeStandaloneActivation: (source: 'button' | 'pointer') => ActionExecution;
	readonly requestTerminalFocus: () => boolean;
	readonly terminalTargetAvailable: boolean;
}

interface WorkbenchStandaloneTerminalProjection {
	readonly activeLocation: WorkbenchActiveLocation;
	readonly terminalTargetAvailable: boolean;
}

export function useWorkbenchStandaloneTerminalActivation({
	activeLocation,
	invokeStandaloneActivation,
	requestTerminalFocus,
	terminalTargetAvailable,
}: UseWorkbenchStandaloneTerminalActivationOptions): (
	origin: WorkbenchStandaloneTerminalActivationOrigin,
) => void {
	const modelReference = React.useRef<WorkbenchStandaloneTerminalActivationState>(
		INITIAL_WORKBENCH_STANDALONE_TERMINAL_ACTIVATION_STATE,
	);
	const requestSequenceReference = React.useRef(0);
	const projectionReference = React.useRef<WorkbenchStandaloneTerminalProjection>({
		activeLocation,
		terminalTargetAvailable,
	});
	projectionReference.current = { activeLocation, terminalTargetAvailable };
	const applyEventReference = React.useRef<
		(event: WorkbenchStandaloneTerminalActivationEvent) => void
	>(() => undefined);

	const executeCommand = useMemoizedFn(
		(command: WorkbenchStandaloneTerminalActivationCommand): void => {
			if (command.kind === 'request-terminal-focus') {
				requestTerminalFocus();
				return;
			}
			let execution: ActionExecution;
			try {
				execution = invokeStandaloneActivation(command.source);
			} catch {
				applyEventReference.current({
					kind: 'activation-settled',
					outcome: 'rejected',
					requestId: command.requestId,
				});
				return;
			}
			void execution.completion.then(
				(result) =>
					applyEventReference.current({
						kind: 'activation-settled',
						outcome:
							result.kind === 'completed' || result.kind === 'unchanged' ? 'accepted' : 'rejected',
						requestId: command.requestId,
					}),
				() =>
					applyEventReference.current({
						kind: 'activation-settled',
						outcome: 'rejected',
						requestId: command.requestId,
					}),
			);
		},
	);
	const applyEvent = useMemoizedFn((event: WorkbenchStandaloneTerminalActivationEvent): void => {
		const transition = transitionWorkbenchStandaloneTerminalActivation(
			modelReference.current,
			event,
		);
		modelReference.current = transition.state;
		for (const command of transition.commands) executeCommand(command);
	});
	applyEventReference.current = applyEvent;
	const observeProjection = useMemoizedFn(
		(projection: WorkbenchStandaloneTerminalProjection): void => {
			applyEvent({
				activeLocation: projection.activeLocation,
				kind: 'projection-observed',
				terminalTargetAvailable: projection.terminalTargetAvailable,
			});
		},
	);

	React.useEffect(() => {
		applyEvent({ kind: 'mount' });
		observeProjection(projectionReference.current);
		return () => applyEvent({ kind: 'dispose' });
	}, [applyEvent, observeProjection]);
	React.useLayoutEffect(
		() => observeProjection({ activeLocation, terminalTargetAvailable }),
		[activeLocation, observeProjection, terminalTargetAvailable],
	);

	return useMemoizedFn((origin: WorkbenchStandaloneTerminalActivationOrigin): void => {
		requestSequenceReference.current += 1;
		const projection = projectionReference.current;
		applyEvent({
			kind: 'activate',
			origin,
			originLocation: projection.activeLocation,
			requestId: requestSequenceReference.current,
		});
		observeProjection(projection);
	});
}
