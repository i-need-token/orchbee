import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

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
	readonly createProject: (path: string) => void;
}

interface ProjectFolderAcquisitionView {
	readonly dismissFailure: () => void;
	readonly failure: WorkbenchProjectFolderAcquisitionFailureCode | null;
	readonly requestCreateProject: () => void;
	readonly selecting: boolean;
}

export function useProjectFolderAcquisition({
	acquisition,
	createProject,
}: UseProjectFolderAcquisitionOptions): ProjectFolderAcquisitionView {
	const [state, setState] = React.useState<WorkbenchProjectFolderAcquisitionState>(
		INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE,
	);
	const modelReference = React.useRef<WorkbenchProjectFolderAcquisitionState>(state);
	const mountedReference = React.useRef(false);

	const applyEvent = useMemoizedFn((event: WorkbenchProjectFolderAcquisitionEvent) => {
		const transition = transitionWorkbenchProjectFolderAcquisition(modelReference.current, event);
		modelReference.current = transition.state;
		if (mountedReference.current) setState(transition.state);
		for (const command of transition.commands) executeCommand(command);
	});
	const settleSelection = useMemoizedFn((result: WorkbenchProjectFolderAcquisitionResult) =>
		applyEvent({ kind: 'selection-settled', result }),
	);
	const executeCommand = useMemoizedFn((command: WorkbenchProjectFolderAcquisitionCommand) => {
		switch (command.kind) {
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
		requestCreateProject: useMemoizedFn(() => applyEvent({ kind: 'selection-requested' })),
		selecting: state.kind === 'selecting',
	};
}
