import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { ProjectWorkspaceState } from '../../../../project-workspace';
import {
	INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE,
	transitionWorkbenchProjectFolderAcquisition,
	type WorkbenchProjectFolderAcquisitionCommand,
	type WorkbenchProjectFolderAcquisitionEvent,
	type WorkbenchProjectFolderAcquisitionState,
} from '../../../model/project-folder-acquisition';
import type {
	WorkbenchProjectFolderAcquisition,
	WorkbenchProjectFolderAcquisitionFailureCode,
	WorkbenchProjectFolderAcquisitionResult,
} from '../../../types/workbench-project-folder-acquisition';

interface UseProjectFolderAcquisitionOptions {
	readonly acquisition: WorkbenchProjectFolderAcquisition | undefined;
	readonly addResourceDirectory: (resourceScopeId: string, path: string) => void;
	readonly createProject: (path: string) => void;
	readonly projectWorkspaceState: ProjectWorkspaceState;
}

interface ProjectFolderAcquisitionView {
	readonly dismissFailure: () => void;
	readonly failure: WorkbenchProjectFolderAcquisitionFailureCode | null;
	readonly requestAddResourceFolder: (resourceScopeId: string) => void;
	readonly requestCreateProject: () => void;
	readonly selecting: boolean;
}

export function useProjectFolderAcquisition({
	acquisition,
	addResourceDirectory,
	createProject,
	projectWorkspaceState,
}: UseProjectFolderAcquisitionOptions): ProjectFolderAcquisitionView {
	const [state, setState] = React.useState<WorkbenchProjectFolderAcquisitionState>(
		INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE,
	);
	const modelReference = React.useRef<WorkbenchProjectFolderAcquisitionState>(state);
	const mountedReference = React.useRef(false);
	const projectWorkspaceStateReference = React.useRef(projectWorkspaceState);
	projectWorkspaceStateReference.current = projectWorkspaceState;

	const applyEvent = useMemoizedFn((event: WorkbenchProjectFolderAcquisitionEvent) => {
		const transition = transitionWorkbenchProjectFolderAcquisition(modelReference.current, event);
		modelReference.current = transition.state;
		if (mountedReference.current) setState(transition.state);
		for (const command of transition.commands) executeCommand(command);
	});
	const settleSelection = useMemoizedFn((result: WorkbenchProjectFolderAcquisitionResult) => {
		const current = modelReference.current;
		let addTargetStatus: 'available' | 'duplicate' | 'missing' | undefined;
		if (
			result.kind === 'selected' &&
			current.kind === 'selecting' &&
			current.target.kind === 'add-project-folder'
		) {
			const resourceScopeId = current.target.resourceScopeId;
			const project = projectWorkspaceStateReference.current.resourceScopes.find(
				(candidate) => candidate.id === resourceScopeId,
			);
			if (project === undefined) addTargetStatus = 'missing';
			else if (project.directories.some((directory) => directory.path === result.path)) {
				addTargetStatus = 'duplicate';
			} else addTargetStatus = 'available';
		}
		applyEvent({ addTargetStatus, kind: 'selection-settled', result });
	});
	const executeCommand = useMemoizedFn((command: WorkbenchProjectFolderAcquisitionCommand) => {
		switch (command.kind) {
			case 'add-project-folder':
				addResourceDirectory(command.resourceScopeId, command.path);
				break;
			case 'create-project':
				createProject(command.path);
				break;
			case 'select-existing-folder':
				if (acquisition === undefined) {
					settleSelection({ code: 'control-unavailable', kind: 'failed' });
					break;
				}
				void acquisition
					.selectExistingFolder()
					.then(settleSelection)
					.catch(() => settleSelection({ code: 'control-unavailable', kind: 'failed' }));
		}
	});

	React.useEffect(() => {
		mountedReference.current = true;
		return () => {
			mountedReference.current = false;
			modelReference.current = transitionWorkbenchProjectFolderAcquisition(modelReference.current, {
				kind: 'dispose',
			}).state;
		};
	}, []);

	return {
		dismissFailure: useMemoizedFn(() => applyEvent({ kind: 'dismiss-failure' })),
		failure: state.kind === 'failed' ? state.code : null,
		requestAddResourceFolder: useMemoizedFn((resourceScopeId: string) =>
			applyEvent({
				kind: 'selection-requested',
				target: { kind: 'add-project-folder', resourceScopeId },
			}),
		),
		requestCreateProject: useMemoizedFn(() =>
			applyEvent({ kind: 'selection-requested', target: { kind: 'create-project' } }),
		),
		selecting: state.kind === 'selecting',
	};
}
