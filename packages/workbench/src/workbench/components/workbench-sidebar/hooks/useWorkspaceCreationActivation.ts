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
	readonly activateWorkspace: (projectId: string, workspaceId: string) => void;
	readonly projectWorkspaceState: ProjectWorkspaceState;
}

export function useWorkspaceCreationActivation({
	activateWorkspace,
	projectWorkspaceState,
}: UseWorkspaceCreationActivationOptions): (projectId: string, workspaceId: string) => void {
	const modelReference = React.useRef<WorkspaceCreationActivationState>(
		INITIAL_WORKSPACE_CREATION_ACTIVATION_STATE,
	);
	const projectWorkspaceStateReference = React.useRef(projectWorkspaceState);
	projectWorkspaceStateReference.current = projectWorkspaceState;

	const executeCommand = useMemoizedFn((command: WorkspaceCreationActivationCommand) => {
		activateWorkspace(command.target.projectId, command.target.workspaceId);
	});
	const applyEvent = useMemoizedFn((event: WorkspaceCreationActivationEvent) => {
		const transition = transitionWorkspaceCreationActivation(modelReference.current, event);
		modelReference.current = transition.state;
		for (const command of transition.commands) executeCommand(command);
	});
	const observe = useMemoizedFn((state: ProjectWorkspaceState) => {
		applyEvent({
			kind: 'projection-observed',
			targets: state.projects.flatMap((project) =>
				project.workspaces.map((workspace) => ({
					projectId: project.id,
					workspaceId: workspace.id,
				})),
			),
		});
	});

	React.useEffect(() => {
		applyEvent({ kind: 'mount' });
		observe(projectWorkspaceStateReference.current);
		return () => applyEvent({ kind: 'dispose' });
	}, [applyEvent, observe]);
	// Observe committed membership before passive Action registration cleanup retires the current scope.
	React.useLayoutEffect(() => observe(projectWorkspaceState), [observe, projectWorkspaceState]);

	return useMemoizedFn((projectId: string, workspaceId: string) => {
		applyEvent({ kind: 'request', target: { projectId, workspaceId } });
		observe(projectWorkspaceStateReference.current);
	});
}
