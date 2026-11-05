import {
	createTypedActionRuntime,
	defineAction,
	type ActionExecution,
	type ActionRuntime,
	type ActionScope,
} from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';

import {
	WORKBENCH_ACTION_IDS,
	workbenchProjectNewInput,
	type WorkbenchActionCatalog,
	type WorkbenchProjectNewInput,
} from '../../../actions/workbench-action-ids';
import type {
	WorkbenchProjectCreationReceipt,
	WorkbenchProjectFolderAcquisition,
	WorkbenchProjectFolderAcquisitionFailureCode,
} from '../../../types/workbench-project-folder-acquisition';
import { useNewProjectFolder, type NewProjectFolderView } from './useNewProjectFolder';
import { useProjectFolderAcquisition } from './useProjectFolderAcquisition';

interface UseWorkbenchProjectCreationOptions {
	/** Absent where the host offers no folder picker, so a person enters the folder's path. */
	readonly acquisition: WorkbenchProjectFolderAcquisition | undefined;
	readonly activateCreatedProject: (
		receipt: WorkbenchProjectCreationReceipt,
	) => ActionExecution<void>;
	readonly createProject: (path: string) => ActionExecution<WorkbenchProjectCreationReceipt>;
	readonly isCreatedProjectActive: (receipt: WorkbenchProjectCreationReceipt) => boolean;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}

interface WorkbenchProjectPathForm {
	readonly close: () => void;
	readonly path: string;
	readonly setPath: (path: string) => void;
	readonly submit: (path: string) => void;
}

/** What the Project creation dialogs show, and the semantic actions they invoke. */
export interface WorkbenchProjectCreationView {
	readonly dismissFailure: () => void;
	readonly failure: WorkbenchProjectFolderAcquisitionFailureCode | null;
	readonly newFolder: NewProjectFolderView;
	/** The folder path form, open where the host has no folder picker. */
	readonly pathForm: WorkbenchProjectPathForm | null;
}

/**
 * Starts a Project from a folder a person selects or creates, through the New Project Action that
 * Search offers. The workflow belongs to the window rather than the sidebar, so it runs while the
 * sidebar is hidden.
 */
export function useWorkbenchProjectCreation({
	acquisition,
	activateCreatedProject,
	createProject,
	isCreatedProjectActive,
	runtime,
	scope,
}: UseWorkbenchProjectCreationOptions): WorkbenchProjectCreationView {
	const [pathFormOpen, setPathFormOpen] = React.useState(false);
	const [path, setPath] = React.useState('');
	const folderAcquisition = useProjectFolderAcquisition({ acquisition, createProject });
	const newFolder = useNewProjectFolder({
		activateCreatedProject,
		capability: acquisition?.newFolder,
		createProject,
		isCreatedProjectActive,
		selectExisting: folderAcquisition.requestCreateProject,
	});
	const open = useMemoizedFn(({ folder }: WorkbenchProjectNewInput) => {
		if (folder === 'new') {
			newFolder.open();
		} else if (acquisition === undefined) {
			setPath('');
			setPathFormOpen(true);
		} else {
			folderAcquisition.requestCreateProject();
		}
	});
	const pending = folderAcquisition.selecting || newFolder.busy;
	const newFolderAvailable = acquisition?.newFolder !== undefined;

	React.useEffect(() => {
		const unavailable = { kind: 'disabled', reason: workbenchMessages.actionUnavailable } as const;
		const registration = createTypedActionRuntime<WorkbenchActionCatalog>(runtime).register(
			scope,
			defineAction({
				category: { id: 'projects', title: workbenchMessages.actionCategoryProjects },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'project-new',
					mode: 'foreground',
				},
				getDiscoveryAvailability: () => (pending ? unavailable : { kind: 'enabled' }),
				getAvailability: (_context, input) =>
					pending || (input.folder === 'new' && !newFolderAvailable)
						? unavailable
						: { kind: 'enabled' },
				handler: {
					execute: (context, input) => {
						context.markCommitted();
						open(input);
						return { kind: 'completed' };
					},
				},
				id: WORKBENCH_ACTION_IDS.newProject,
				input: workbenchProjectNewInput,
				title: workbenchMessages.projectNew,
			}),
		);
		return () => registration.dispose();
	}, [newFolderAvailable, open, pending, runtime, scope]);

	const closePathForm = useMemoizedFn(() => setPathFormOpen(false));
	const submitPath = useMemoizedFn((submitted: string) => {
		createProject(submitted);
		setPathFormOpen(false);
	});
	return {
		dismissFailure: useMemoizedFn(() => {
			folderAcquisition.dismissFailure();
			newFolder.close();
		}),
		failure:
			folderAcquisition.failure ??
			(newFolder.state.kind === 'failed' ? newFolder.state.code : null),
		newFolder,
		pathForm: pathFormOpen ? { close: closePathForm, path, setPath, submit: submitPath } : null,
	};
}
