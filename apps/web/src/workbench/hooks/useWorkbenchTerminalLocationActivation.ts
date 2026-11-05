import type { ActionExecution } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchActiveLocation } from '../../project-workspace';
import {
	INITIAL_WORKBENCH_TERMINAL_LOCATION_ACTIVATION_STATE,
	transitionWorkbenchTerminalLocationActivation,
	type WorkbenchTerminalLocationActivationCommand,
	type WorkbenchTerminalLocationActivationEvent,
	type WorkbenchTerminalLocationActivationOrigin,
	type WorkbenchTerminalLocationActivationState,
} from '../model/workbench-terminal-location-activation';

interface UseWorkbenchTerminalLocationActivationOptions {
	readonly activeLocation: WorkbenchActiveLocation;
	readonly invokeLocationActivation: (
		targetLocation: WorkbenchActiveLocation,
		source: 'button' | 'keyboard' | 'pointer',
	) => ActionExecution;
	readonly requestTerminalFocus: (targetLocation: WorkbenchActiveLocation) => boolean;
	readonly terminalTargetAvailable: boolean;
}

export function useWorkbenchTerminalLocationActivation({
	activeLocation,
	invokeLocationActivation,
	requestTerminalFocus,
	terminalTargetAvailable,
}: UseWorkbenchTerminalLocationActivationOptions): (
	targetLocation: WorkbenchActiveLocation,
	origin: WorkbenchTerminalLocationActivationOrigin,
) => void {
	const modelReference = React.useRef<WorkbenchTerminalLocationActivationState>(
		INITIAL_WORKBENCH_TERMINAL_LOCATION_ACTIVATION_STATE,
	);
	const requestSequenceReference = React.useRef(0);
	const applyEventReference = React.useRef<
		(event: WorkbenchTerminalLocationActivationEvent) => void
	>(() => undefined);

	const executeCommand = useMemoizedFn(
		(command: WorkbenchTerminalLocationActivationCommand): void => {
			if (command.kind === 'request-terminal-focus') {
				requestTerminalFocus(command.targetLocation);
				return;
			}
			let execution: ActionExecution;
			try {
				execution = invokeLocationActivation(command.targetLocation, command.source);
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
	const applyEvent = useMemoizedFn((event: WorkbenchTerminalLocationActivationEvent): void => {
		const transition = transitionWorkbenchTerminalLocationActivation(modelReference.current, event);
		modelReference.current = transition.state;
		for (const command of transition.commands) executeCommand(command);
	});
	applyEventReference.current = applyEvent;
	const observeProjection = useMemoizedFn((): void => {
		applyEvent({
			activeLocation,
			kind: 'projection-observed',
			terminalTargetAvailable,
		});
	});

	React.useEffect(() => {
		applyEvent({ kind: 'mount' });
		observeProjection();
		return () => applyEvent({ kind: 'dispose' });
	}, [applyEvent, observeProjection]);
	React.useLayoutEffect(
		() => observeProjection(),
		[activeLocation, observeProjection, terminalTargetAvailable],
	);

	return useMemoizedFn(
		(
			targetLocation: WorkbenchActiveLocation,
			origin: WorkbenchTerminalLocationActivationOrigin,
		): void => {
			requestSequenceReference.current += 1;
			applyEvent({
				kind: 'activate',
				origin,
				originLocation: activeLocation,
				requestId: requestSequenceReference.current,
				targetLocation,
			});
			observeProjection();
		},
	);
}
