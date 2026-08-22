import type { ActionInputValidation, ActionSignature } from '@orchbee/actions';
import type { TabStripOrientation } from '@orchbee/tab-strip';

import type { WorkbenchSearchFilter } from '../types/workbench-search';

export const WORKBENCH_ACTION_IDS = {
	activateStandalone: 'workbench.location.standalone.activate',
	activateWorkspace: 'workbench.workspace.activate',
	addProjectFolder: 'workbench.project.directory.add',
	createProject: 'workbench.project.create',
	createTerminal: 'workbench.terminal.create',
	createWorkspace: 'workbench.workspace.create',
	deleteWorkspace: 'workbench.workspace.delete',
	duplicateWorkspace: 'workbench.workspace.duplicate',
	openSearch: 'workbench.search.open',
	openSettings: 'workbench.settings.open',
	removeProject: 'workbench.project.remove',
	removeProjectDirectory: 'workbench.project.directory.remove',
	renameProject: 'workbench.project.rename',
	reorderProject: 'workbench.project.reorder',
	reorderWorkspace: 'workbench.workspace.reorder',
	closeActiveTerminal: 'workbench.terminal.active.close',
	splitActivePanelBelow: 'workbench.panel.active.split-below',
	splitActivePanelRight: 'workbench.panel.active.split-right',
	setTabStripOrientation: 'workbench.tab-strip.orientation.set',
	setProjectPrimaryDirectory: 'workbench.project.primary-directory.set',
	toggleActivePanelMaximization: 'workbench.panel.active.maximization.toggle',
	toggleProjectExpansion: 'workbench.project.expansion.toggle',
	toggleSidebar: 'workbench.sidebar.toggle',
	updateWorkspace: 'workbench.workspace.update',
} as const;

export interface WorkbenchProjectInput {
	readonly projectId: string;
}

export interface WorkbenchProjectCreateInput {
	readonly path: string;
}

export interface WorkbenchProjectFolderAddInput extends WorkbenchProjectInput {
	readonly path: string;
}

export interface WorkbenchWorkspaceInput extends WorkbenchProjectInput {
	readonly workspaceId: string;
}

export interface WorkbenchWorkspaceCreateInput extends WorkbenchProjectInput {
	readonly directoryId: string;
}

export interface WorkbenchProjectDirectoryInput extends WorkbenchProjectInput {
	readonly directoryId: string;
}

export interface WorkbenchProjectRenameInput extends WorkbenchProjectInput {
	readonly name: string;
}

export interface WorkbenchProjectReorderInput extends WorkbenchProjectInput {
	readonly beforeProjectId: string | null;
}

export interface WorkbenchWorkspaceUpdateInput extends WorkbenchWorkspaceInput {
	readonly name: string;
	readonly primaryDirectoryId: string;
}

export interface WorkbenchWorkspaceReorderInput extends WorkbenchWorkspaceInput {
	readonly beforeWorkspaceId: string | null;
}

export interface WorkbenchTabStripOrientationInput {
	readonly orientation: TabStripOrientation;
}

export interface WorkbenchTerminalCreateInput {
	readonly directoryId?: string | undefined;
	readonly groupId: string | null;
	readonly index?: number | undefined;
}

export interface WorkbenchSearchOpenInput {
	readonly filter: WorkbenchSearchFilter;
}

export interface WorkbenchActionCatalog {
	readonly [WORKBENCH_ACTION_IDS.activateStandalone]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.activateWorkspace]: ActionSignature<WorkbenchWorkspaceInput>;
	readonly [WORKBENCH_ACTION_IDS.addProjectFolder]: ActionSignature<WorkbenchProjectFolderAddInput>;
	readonly [WORKBENCH_ACTION_IDS.createProject]: ActionSignature<WorkbenchProjectCreateInput>;
	readonly [WORKBENCH_ACTION_IDS.createTerminal]: ActionSignature<WorkbenchTerminalCreateInput>;
	readonly [WORKBENCH_ACTION_IDS.createWorkspace]: ActionSignature<WorkbenchWorkspaceCreateInput>;
	readonly [WORKBENCH_ACTION_IDS.deleteWorkspace]: ActionSignature<WorkbenchWorkspaceInput>;
	readonly [WORKBENCH_ACTION_IDS.duplicateWorkspace]: ActionSignature<WorkbenchWorkspaceInput>;
	readonly [WORKBENCH_ACTION_IDS.openSearch]: ActionSignature<WorkbenchSearchOpenInput>;
	readonly [WORKBENCH_ACTION_IDS.openSettings]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.removeProject]: ActionSignature<WorkbenchProjectInput>;
	readonly [WORKBENCH_ACTION_IDS.removeProjectDirectory]: ActionSignature<WorkbenchProjectDirectoryInput>;
	readonly [WORKBENCH_ACTION_IDS.renameProject]: ActionSignature<WorkbenchProjectRenameInput>;
	readonly [WORKBENCH_ACTION_IDS.reorderProject]: ActionSignature<WorkbenchProjectReorderInput>;
	readonly [WORKBENCH_ACTION_IDS.reorderWorkspace]: ActionSignature<WorkbenchWorkspaceReorderInput>;
	readonly [WORKBENCH_ACTION_IDS.closeActiveTerminal]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.splitActivePanelBelow]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.splitActivePanelRight]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.setTabStripOrientation]: ActionSignature<WorkbenchTabStripOrientationInput>;
	readonly [WORKBENCH_ACTION_IDS.setProjectPrimaryDirectory]: ActionSignature<WorkbenchProjectDirectoryInput>;
	readonly [WORKBENCH_ACTION_IDS.toggleActivePanelMaximization]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.toggleProjectExpansion]: ActionSignature<WorkbenchProjectInput>;
	readonly [WORKBENCH_ACTION_IDS.toggleSidebar]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.updateWorkspace]: ActionSignature<WorkbenchWorkspaceUpdateInput>;
}

function isRecord(input: unknown): input is Readonly<Record<string, unknown>> {
	return typeof input === 'object' && input !== null && !Array.isArray(input);
}

function isWorkbenchSearchFilter(input: unknown): input is WorkbenchSearchFilter {
	return input === 'all' || input === 'agents' || input === 'actions' || input === 'settings';
}

export const workbenchTerminalCreateInput: ActionInputValidation<WorkbenchTerminalCreateInput> = {
	name: 'WorkbenchTerminalCreateInput',
	parse: (input) => {
		if (input === undefined) {
			return { success: true, data: { groupId: null } };
		}
		if (
			!isRecord(input) ||
			!(
				input['directoryId'] === undefined ||
				(typeof input['directoryId'] === 'string' && input['directoryId'].length > 0)
			) ||
			!(input['groupId'] === null || typeof input['groupId'] === 'string') ||
			!(
				input['index'] === undefined ||
				(typeof input['index'] === 'number' &&
					Number.isInteger(input['index']) &&
					input['index'] >= 0)
			)
		) {
			return { success: false, code: 'expected-terminal-create-input' };
		}
		return {
			success: true,
			data: {
				...(input['directoryId'] === undefined ? {} : { directoryId: input['directoryId'] }),
				groupId: input['groupId'],
				...(input['index'] === undefined ? {} : { index: input['index'] }),
			},
		};
	},
};

export const workbenchSearchOpenInput: ActionInputValidation<WorkbenchSearchOpenInput> = {
	name: 'WorkbenchSearchOpenInput',
	parse: (input) =>
		isRecord(input) && isWorkbenchSearchFilter(input['filter'])
			? {
					success: true,
					data: { filter: input['filter'] },
				}
			: { success: false, code: 'expected-workbench-search-filter' },
};

export const workbenchProjectInput: ActionInputValidation<WorkbenchProjectInput> = {
	name: 'WorkbenchProjectInput',
	parse: (input) =>
		isRecord(input) && typeof input['projectId'] === 'string'
			? { success: true, data: { projectId: input['projectId'] } }
			: { success: false, code: 'expected-workbench-project-input' },
};

export const workbenchProjectCreateInput: ActionInputValidation<WorkbenchProjectCreateInput> = {
	name: 'WorkbenchProjectCreateInput',
	parse: (input) =>
		isRecord(input) && typeof input['path'] === 'string' && input['path'].trim().length > 0
			? { success: true, data: { path: input['path'].trim() } }
			: { success: false, code: 'expected-workbench-project-create-input' },
};

export const workbenchProjectFolderAddInput: ActionInputValidation<WorkbenchProjectFolderAddInput> =
	{
		name: 'WorkbenchProjectFolderAddInput',
		parse: (input) =>
			isRecord(input) &&
			typeof input['path'] === 'string' &&
			input['path'].trim().length > 0 &&
			typeof input['projectId'] === 'string'
				? {
						success: true,
						data: { path: input['path'].trim(), projectId: input['projectId'] },
					}
				: { success: false, code: 'expected-workbench-project-folder-add-input' },
	};

export const workbenchProjectDirectoryInput: ActionInputValidation<WorkbenchProjectDirectoryInput> =
	{
		name: 'WorkbenchProjectDirectoryInput',
		parse: (input) =>
			isRecord(input) &&
			typeof input['directoryId'] === 'string' &&
			typeof input['projectId'] === 'string'
				? {
						success: true,
						data: {
							directoryId: input['directoryId'],
							projectId: input['projectId'],
						},
					}
				: { success: false, code: 'expected-workbench-project-directory-input' },
	};

export const workbenchProjectRenameInput: ActionInputValidation<WorkbenchProjectRenameInput> = {
	name: 'WorkbenchProjectRenameInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['name'] === 'string' &&
		input['name'].trim().length > 0 &&
		typeof input['projectId'] === 'string'
			? {
					success: true,
					data: {
						name: input['name'].trim(),
						projectId: input['projectId'],
					},
				}
			: { success: false, code: 'expected-workbench-project-rename-input' },
};

export const workbenchProjectReorderInput: ActionInputValidation<WorkbenchProjectReorderInput> = {
	name: 'WorkbenchProjectReorderInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['projectId'] === 'string' &&
		(input['beforeProjectId'] === null || typeof input['beforeProjectId'] === 'string')
			? {
					success: true,
					data: {
						beforeProjectId: input['beforeProjectId'],
						projectId: input['projectId'],
					},
				}
			: { success: false, code: 'expected-workbench-project-reorder-input' },
};

export const workbenchWorkspaceInput: ActionInputValidation<WorkbenchWorkspaceInput> = {
	name: 'WorkbenchWorkspaceInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['projectId'] === 'string' &&
		typeof input['workspaceId'] === 'string'
			? {
					success: true,
					data: {
						projectId: input['projectId'],
						workspaceId: input['workspaceId'],
					},
				}
			: { success: false, code: 'expected-workbench-workspace-input' },
};

export const workbenchWorkspaceReorderInput: ActionInputValidation<WorkbenchWorkspaceReorderInput> =
	{
		name: 'WorkbenchWorkspaceReorderInput',
		parse: (input) =>
			isRecord(input) &&
			(input['beforeWorkspaceId'] === null || typeof input['beforeWorkspaceId'] === 'string') &&
			typeof input['projectId'] === 'string' &&
			typeof input['workspaceId'] === 'string'
				? {
						success: true,
						data: {
							beforeWorkspaceId: input['beforeWorkspaceId'],
							projectId: input['projectId'],
							workspaceId: input['workspaceId'],
						},
					}
				: { success: false, code: 'expected-workbench-workspace-reorder-input' },
	};

export const workbenchWorkspaceCreateInput: ActionInputValidation<WorkbenchWorkspaceCreateInput> = {
	name: 'WorkbenchWorkspaceCreateInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['directoryId'] === 'string' &&
		typeof input['projectId'] === 'string'
			? {
					success: true,
					data: {
						directoryId: input['directoryId'],
						projectId: input['projectId'],
					},
				}
			: { success: false, code: 'expected-workbench-workspace-create-input' },
};

export const workbenchWorkspaceUpdateInput: ActionInputValidation<WorkbenchWorkspaceUpdateInput> = {
	name: 'WorkbenchWorkspaceUpdateInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['name'] === 'string' &&
		input['name'].trim().length > 0 &&
		typeof input['primaryDirectoryId'] === 'string' &&
		typeof input['projectId'] === 'string' &&
		typeof input['workspaceId'] === 'string'
			? {
					success: true,
					data: {
						name: input['name'].trim(),
						primaryDirectoryId: input['primaryDirectoryId'],
						projectId: input['projectId'],
						workspaceId: input['workspaceId'],
					},
				}
			: { success: false, code: 'expected-workbench-workspace-update-input' },
};

export const workbenchTabStripOrientationInput: ActionInputValidation<WorkbenchTabStripOrientationInput> =
	{
		name: 'WorkbenchTabStripOrientationInput',
		parse: (input) =>
			typeof input === 'object' &&
			input !== null &&
			'orientation' in input &&
			(input.orientation === 'horizontal' || input.orientation === 'vertical')
				? {
						success: true,
						data: { orientation: input.orientation },
					}
				: { success: false, code: 'expected-tab-strip-orientation' },
	};
