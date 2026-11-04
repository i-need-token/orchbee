import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { ProjectWorkspaceState } from '../../../../project-workspace';
import {
	INITIAL_WORKSPACE_CREATION_ACTIVATION_STATE,
	transitionWorkspaceCreationActivation,
	type WorkspaceCreationActivationCommand,
	type WorkspaceCreationActivationEvent,
	type WorkspaceCreationActivationState,
} from '../model/workspace-creation-activation';

interface UseWorkspaceCreationActivationOptions {
	readonly activateWorkspace: (workspaceId: string) => void;
	readonly projectWorkspaceState: ProjectWorkspaceState;
	readonly subscribeNavigation: (listener: () => void) => () => void;
}

export function useWorkspaceCreationActivation({
	activateWorkspace,
	projectWorkspaceState,
	subscribeNavigation,
}: UseWorkspaceCreationActivationOptions): {
	readonly begin: () => (workspaceId: string) => void;
	readonly cancel: () => void;
} {
	const modelReference = React.useRef<WorkspaceCreationActivationState>(
		INITIAL_WORKSPACE_CREATION_ACTIVATION_STATE,
	);

	const executeCommand = useMemoizedFn((command: WorkspaceCreationActivationCommand) => {
		activateWorkspace(command.workspaceId);
	});
	const applyEvent = useMemoizedFn((event: WorkspaceCreationActivationEvent) => {
		const transition = transitionWorkspaceCreationActivation(modelReference.current, event);
		modelReference.current = transition.state;
		for (const command of transition.commands) executeCommand(command);
	});
	React.useEffect(
		() => subscribeNavigation(() => applyEvent({ kind: 'navigation' })),
		[applyEvent, subscribeNavigation],
	);
	const intentSequence = React.useRef(0);
	const observedLocation = React.useRef(projectWorkspaceState.activeLocation);
	const observe = useMemoizedFn((state: ProjectWorkspaceState) => {
		if (observedLocation.current !== state.activeLocation) {
			observedLocation.current = state.activeLocation;
			applyEvent({ kind: 'navigation' });
		}
		applyEvent({
			kind: 'projection-observed',
			workspaceIds: state.workspaces.map((workspace) => workspace.id),
		});
	});

	const observeCurrent = useMemoizedFn(() => observe(projectWorkspaceState));

	React.useEffect(() => {
		applyEvent({ kind: 'mount' });
		observeCurrent();
		return () => applyEvent({ kind: 'dispose' });
	}, [applyEvent, observeCurrent]);
	// Observe committed membership before passive Action registration cleanup retires the current scope.
	React.useLayoutEffect(() => observeCurrent(), [observeCurrent, projectWorkspaceState]);

	return {
		begin: useMemoizedFn(() => {
			observeCurrent();
			const intent = ++intentSequence.current;
			applyEvent({ kind: 'begin', intent });
			return (workspaceId) => {
				observeCurrent();
				applyEvent({ kind: 'completed', intent, workspaceId });
				observeCurrent();
			};
		}),
		cancel: useMemoizedFn(() => applyEvent({ kind: 'navigation' })),
	};
}
