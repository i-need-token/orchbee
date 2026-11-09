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
import {
	readWorkbenchDirectoryRefusal,
	type WorkbenchDirectoryRefusal,
} from '../../../model/readWorkbenchDirectoryRefusal';
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
	readonly createWorkspaceAtDirectory: (
		projectId: string,
		path: string,
	) => ActionExecution<unknown>;
	readonly createProject: (path: string) => ActionExecution<WorkbenchProjectCreationReceipt>;
	readonly isCreatedProjectActive: (receipt: WorkbenchProjectCreationReceipt) => boolean;
	/** The path form now says why its folder was refused, so the status line need not. */
	readonly onDirectoryRefusalShown: () => void;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}

interface WorkbenchProjectPathForm {
	readonly title?: string | undefined;
	readonly close: () => void;
	readonly path: string;
	/** Why the Server refused the folder, while the field still holds the refused text. */
	readonly refusal: WorkbenchDirectoryRefusal | null;
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

/** A path form submission whose refusal can reopen the form as it was. */
interface PathFormSubmission {
	readonly path: string;
	readonly target: { readonly projectId: string } | null;
	readonly title: string | undefined;
}

/**
 * Starts a Project from a folder a person selects or creates, through the New Project Action that
 * Search offers. The workflow belongs to the window rather than the sidebar, so it runs while the
 * sidebar is hidden. As in the TUI, a folder typed in the path form that the Server refuses reopens
 * the form with the typed text and the reason under it, unless another creation opened meanwhile.
 */
export function useWorkbenchProjectCreation({
	acquisition,
	activateCreatedProject,
	createProject,
	createWorkspaceAtDirectory,
	isCreatedProjectActive,
	onDirectoryRefusalShown,
	runtime,
	scope,
}: UseWorkbenchProjectCreationOptions): WorkbenchProjectCreationView {
	const [pathFormOpen, setPathFormOpen] = React.useState(false);
	const [path, setPath] = React.useState('');
	const [pathTitle, setPathTitle] = React.useState<string>();
	const [refused, setRefused] = React.useState<{
		readonly path: string;
		readonly reason: WorkbenchDirectoryRefusal;
	} | null>(null);
	const workspaceTarget = React.useRef<{ projectId: string } | null>(null);
	const latestSubmission = React.useRef<PathFormSubmission | null>(null);
	const createFromPath = useMemoizedFn((selectedPath: string): ActionExecution<unknown> => {
		const target = workspaceTarget.current;
		return target === null
			? createProject(selectedPath)
			: createWorkspaceAtDirectory(target.projectId, selectedPath);
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
	const openPathForm = useMemoizedFn(() => {
		setPath('');
		setRefused(null);
		setPathFormOpen(true);
	});
	const open = useMemoizedFn(({ folder }: WorkbenchProjectNewInput) => {
		if (pending) return;
		workspaceTarget.current = null;
		setPathTitle(undefined);
		if (folder === 'new') {
			newFolder.open();
		} else if (acquisition === undefined) {
			openPathForm();
		} else {
			folderAcquisition.requestCreateProject();
		}
	});
	const pending = folderAcquisition.selecting || newFolder.busy || pathFormOpen;
	const newFolderAvailable = acquisition?.newFolder !== undefined;

	React.useEffect(() => {
		const busy = {
			kind: 'disabled',
			reason: workbenchMessages.projectUnavailableCreationPending,
		} as const;
		// Only a host that cannot make a folder lacks New Project in a new folder.
		const noNewFolder = { kind: 'disabled', reason: workbenchMessages.actionUnavailable } as const;
		const registration = createTypedActionRuntime<WorkbenchActionCatalog>(runtime).register(
			scope,
			defineAction({
				category: { id: 'projects', title: workbenchMessages.actionCategoryProjects },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'project-new',
					mode: 'foreground',
				},
				getDiscoveryAvailability: () => (pending ? busy : { kind: 'enabled' }),
				getAvailability: (_context, input) => {
					if (pending) return busy;
					return input.folder === 'new' && !newFolderAvailable ? noNewFolder : { kind: 'enabled' };
				},
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
	const reopenRefused = useMemoizedFn(
		(submission: PathFormSubmission, reason: WorkbenchDirectoryRefusal) => {
			if (latestSubmission.current !== submission) return;
			latestSubmission.current = null;
			if (pending) return;
			workspaceTarget.current = submission.target;
			setPathTitle(submission.title);
			setPath(submission.path);
			setRefused({ path: submission.path, reason });
			setPathFormOpen(true);
			onDirectoryRefusalShown();
		},
	);
	const submitPath = useMemoizedFn((submitted: string) => {
		const submission: PathFormSubmission = {
			path,
			target: workspaceTarget.current,
			title: pathTitle,
		};
		latestSubmission.current = submission;
		const creation = createFromPath(submitted);
		setPathFormOpen(false);
		void creation.completion.then((result) => {
			const reason = readWorkbenchDirectoryRefusal(result);
			if (reason !== null) reopenRefused(submission, reason);
		});
	});
	return {
		requestWorkspaceDirectory: useMemoizedFn((projectId, title) => {
			if (pending) return;
			workspaceTarget.current = { projectId };
			setPathTitle(title);
			if (acquisition === undefined) openPathForm();
			else folderAcquisition.requestCreateProject();
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
			? {
					close: closePathForm,
					path,
					refusal: refused !== null && refused.path === path ? refused.reason : null,
					setPath,
					submit: submitPath,
					title: pathTitle,
				}
			: null,
	};
}
