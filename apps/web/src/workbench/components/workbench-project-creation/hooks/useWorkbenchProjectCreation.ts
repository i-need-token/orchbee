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
	readonly createWorkspaceAtDirectory: (projectId: string, path: string) => void;
	readonly createProject: (path: string) => ActionExecution<WorkbenchProjectCreationReceipt>;
	readonly isCreatedProjectActive: (receipt: WorkbenchProjectCreationReceipt) => boolean;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}

interface WorkbenchProjectPathForm {
	readonly title?: string | undefined;
	readonly close: () => void;
	readonly path: string;
	readonly setPath: (path: string) => void;
	readonly submit: (path: string) => void;
}

/** Shared Folder acquisition for new Projects and Workspaces in an existing Project. */
export interface WorkbenchProjectCreationView {
	readonly requestWorkspaceDirectory: (projectId: string, title: string) => void;
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
	createWorkspaceAtDirectory,
	isCreatedProjectActive,
	runtime,
	scope,
}: UseWorkbenchProjectCreationOptions): WorkbenchProjectCreationView {
	const [pathFormOpen, setPathFormOpen] = React.useState(false);
	const [path, setPath] = React.useState('');
	const [pathTitle, setPathTitle] = React.useState<string>();
	const workspaceTarget = React.useRef<{ projectId: string } | null>(null);
	const createFromPath = useMemoizedFn((selectedPath: string) => {
		const target = workspaceTarget.current;
		if (target === null) createProject(selectedPath);
		else createWorkspaceAtDirectory(target.projectId, selectedPath);
	});
	const folderAcquisition = useProjectFolderAcquisition({
		acquisition,
		createProject: createFromPath,
	});
	const newFolder = useNewProjectFolder({
		activateCreatedProject,
		capability: acquisition?.newFolder,
		createProject,
		isCreatedProjectActive,
		selectExisting: folderAcquisition.requestCreateProject,
	});
	const open = useMemoizedFn(({ folder }: WorkbenchProjectNewInput) => {
		if (pending) return;
		workspaceTarget.current = null;
		setPathTitle(undefined);
		if (folder === 'new') {
			newFolder.open();
		} else if (acquisition === undefined) {
			setPath('');
			setPathFormOpen(true);
		} else {
			folderAcquisition.requestCreateProject();
		}
	});
	const pending = folderAcquisition.selecting || newFolder.busy || pathFormOpen;
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
		createFromPath(submitted);
		setPathFormOpen(false);
	});
	return {
		requestWorkspaceDirectory: useMemoizedFn((projectId, title) => {
			if (pending) return;
			workspaceTarget.current = { projectId };
			setPathTitle(title);
			if (acquisition === undefined) {
				setPath('');
				setPathFormOpen(true);
			} else folderAcquisition.requestCreateProject();
		}),
		dismissFailure: useMemoizedFn(() => {
			folderAcquisition.dismissFailure();
			newFolder.close();
		}),
		failure:
			folderAcquisition.failure ??
			(newFolder.state.kind === 'failed' ? newFolder.state.code : null),
		newFolder,
		pathForm: pathFormOpen
			? { close: closePathForm, path, setPath, submit: submitPath, title: pathTitle }
			: null,
	};
}
