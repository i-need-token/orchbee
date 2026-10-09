import type { ActionInputValidation, ActionSignature } from '@orchbee/actions';
import { z } from 'zod';
import { AgentSessionIdSchema, type AgentSessionId } from '@orchbee/agent';
import type { TabStripOrientation } from '@orchbee/tab-strip';

import type { PanelFocusPosition } from '../components/workbench-panel-workspace/model/resolvePanelFocusByPosition';
import type { WorkbenchSearchFilter } from '../types/workbench-search';
import type { WorkbenchProjectCreationReceipt } from '../types/workbench-project-folder-acquisition';

export const WORKBENCH_ACTION_IDS = {
	navigateAgent: 'workbench.agent.navigate',
	activateStandalone: 'workbench.location.standalone.activate',
	activateWorkspace: 'workbench.workspace.activate',
	addProjectFolder: 'workbench.project.directory.add',
	createProject: 'workbench.project.create',
	createTerminal: 'workbench.terminal.create',
	createTerminalInCurrentDirectory: 'workbench.terminal.create-current-directory',
	decreaseTerminalFontSize: 'workbench.terminal.font-size.decrease',
	createWorkspace: 'workbench.workspace.create',
	deleteWorkspace: 'workbench.workspace.delete',
	duplicateWorkspace: 'workbench.workspace.duplicate',
	equalizeActivePanels: 'workbench.panel.active.equalize',
	focusActiveTerminal: 'workbench.focus.active-terminal',
	focusActivePanelDown: 'workbench.panel.active.focus-down',
	focusActivePanelLeft: 'workbench.panel.active.focus-left',
	focusActivePanelByPosition: 'workbench.panel.active.focus-by-position',
	focusActivePanelNext: 'workbench.panel.active.focus-next',
	focusActivePanelPrevious: 'workbench.panel.active.focus-previous',
	focusActivePanelRight: 'workbench.panel.active.focus-right',
	focusActivePanelUp: 'workbench.panel.active.focus-up',
	focusActivePanelVisible: 'workbench.panel.active.focus-visible',
	focusNextRegion: 'workbench.focus.next-region',
	focusPreviousRegion: 'workbench.focus.previous-region',
	focusSidebar: 'workbench.focus.sidebar',
	focusTabStrip: 'workbench.focus.tab-strip',
	increaseTerminalFontSize: 'workbench.terminal.font-size.increase',
	openSearch: 'workbench.search.open',
	openPanelBroadcastInput: 'workbench.panel.active.broadcast-input.open',
	openSettings: 'workbench.settings.open',
	removeProject: 'workbench.project.remove',
	removeProjectDirectory: 'workbench.project.directory.remove',
	resetTerminalFontSize: 'workbench.terminal.font-size.reset',
	resizeActivePanel: 'workbench.panel.active.resize',
	renameProject: 'workbench.project.rename',
	reorderProject: 'workbench.project.reorder',
	reorderWorkspace: 'workbench.workspace.reorder',
	moveActiveTabBackward: 'workbench.tab.active.move-backward',
	moveActiveTabForward: 'workbench.tab.active.move-forward',
	moveActivePanelDown: 'workbench.panel.active.move-down',
	moveActivePanelLeft: 'workbench.panel.active.move-left',
	moveActivePanelRight: 'workbench.panel.active.move-right',
	moveActivePanelUp: 'workbench.panel.active.move-up',
	setTabTitle: 'workbench.tab.title.set',
	selectNextTab: 'workbench.tab.select-next',
	selectPreviousTab: 'workbench.tab.select-previous',
	selectTabByPosition: 'workbench.tab.select-by-position',
	closeActiveTerminal: 'workbench.terminal.active.close',
	splitActivePanelBelow: 'workbench.panel.active.split-below',
	splitActivePanelRight: 'workbench.panel.active.split-right',
	swapActivePanelVisible: 'workbench.panel.active.swap-visible',
	setTabStripOrientation: 'workbench.tab-strip.orientation.set',
	setProjectPrimaryDirectory: 'workbench.project.primary-directory.set',
	toggleActivePanelMaximization: 'workbench.panel.active.maximization.toggle',
	toggleProjectExpansion: 'workbench.project.expansion.toggle',
	toggleSidebar: 'workbench.sidebar.toggle',
	updateWorkspace: 'workbench.workspace.update',
} as const;

export interface WorkbenchAgentNavigateInput {
	readonly projectId: string;
	readonly sessionId: AgentSessionId;
	readonly workspaceId: string;
}

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

export interface WorkbenchWorkspaceActivationInput extends WorkbenchWorkspaceInput {
	readonly creationReceipt?: WorkbenchProjectCreationReceipt | undefined;
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

export type WorkbenchPanelPosition = PanelFocusPosition;

export interface WorkbenchPanelPositionInput {
	readonly position: WorkbenchPanelPosition;
}

export type WorkbenchTabPosition = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9;

export interface WorkbenchTabPositionInput {
	readonly position: WorkbenchTabPosition;
}

export type WorkbenchTabTitleInput =
	| undefined
	| {
			readonly kind: 'open';
			readonly tabId: string;
			readonly terminalContextId: string;
	  }
	| {
			readonly kind: 'commit';
			readonly tabId: string;
			readonly terminalContextId: string;
			readonly title: string | null;
	  };

export interface WorkbenchTerminalCreateInput {
	readonly directoryId?: string | undefined;
	readonly groupId: string | null;
	readonly index?: number | undefined;
}

export interface WorkbenchSearchOpenInput {
	readonly filter: WorkbenchSearchFilter;
}

export interface WorkbenchActionCatalog {
	readonly [WORKBENCH_ACTION_IDS.navigateAgent]: ActionSignature<WorkbenchAgentNavigateInput>;
	readonly [WORKBENCH_ACTION_IDS.activateStandalone]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.activateWorkspace]: ActionSignature<WorkbenchWorkspaceActivationInput>;
	readonly [WORKBENCH_ACTION_IDS.addProjectFolder]: ActionSignature<WorkbenchProjectFolderAddInput>;
	readonly [WORKBENCH_ACTION_IDS.createProject]: ActionSignature<
		WorkbenchProjectCreateInput,
		WorkbenchProjectCreationReceipt
	>;
	readonly [WORKBENCH_ACTION_IDS.createTerminal]: ActionSignature<WorkbenchTerminalCreateInput>;
	readonly [WORKBENCH_ACTION_IDS.createTerminalInCurrentDirectory]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.decreaseTerminalFontSize]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.createWorkspace]: ActionSignature<WorkbenchWorkspaceCreateInput>;
	readonly [WORKBENCH_ACTION_IDS.deleteWorkspace]: ActionSignature<WorkbenchWorkspaceInput>;
	readonly [WORKBENCH_ACTION_IDS.duplicateWorkspace]: ActionSignature<WorkbenchWorkspaceInput>;
	readonly [WORKBENCH_ACTION_IDS.equalizeActivePanels]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusActiveTerminal]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusActivePanelDown]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusActivePanelLeft]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusActivePanelByPosition]: ActionSignature<WorkbenchPanelPositionInput>;
	readonly [WORKBENCH_ACTION_IDS.focusActivePanelNext]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusActivePanelPrevious]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusActivePanelRight]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusActivePanelUp]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusActivePanelVisible]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusNextRegion]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusPreviousRegion]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusSidebar]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusTabStrip]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.increaseTerminalFontSize]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.openSearch]: ActionSignature<WorkbenchSearchOpenInput>;
	readonly [WORKBENCH_ACTION_IDS.openPanelBroadcastInput]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.openSettings]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.removeProject]: ActionSignature<WorkbenchProjectInput>;
	readonly [WORKBENCH_ACTION_IDS.removeProjectDirectory]: ActionSignature<WorkbenchProjectDirectoryInput>;
	readonly [WORKBENCH_ACTION_IDS.resetTerminalFontSize]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.resizeActivePanel]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.renameProject]: ActionSignature<WorkbenchProjectRenameInput>;
	readonly [WORKBENCH_ACTION_IDS.reorderProject]: ActionSignature<WorkbenchProjectReorderInput>;
	readonly [WORKBENCH_ACTION_IDS.reorderWorkspace]: ActionSignature<WorkbenchWorkspaceReorderInput>;
	readonly [WORKBENCH_ACTION_IDS.moveActiveTabBackward]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.moveActiveTabForward]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.moveActivePanelDown]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.moveActivePanelLeft]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.moveActivePanelRight]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.moveActivePanelUp]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.setTabTitle]: ActionSignature<WorkbenchTabTitleInput>;
	readonly [WORKBENCH_ACTION_IDS.selectNextTab]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.selectPreviousTab]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.selectTabByPosition]: ActionSignature<WorkbenchTabPositionInput>;
	readonly [WORKBENCH_ACTION_IDS.closeActiveTerminal]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.splitActivePanelBelow]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.splitActivePanelRight]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.swapActivePanelVisible]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.setTabStripOrientation]: ActionSignature<WorkbenchTabStripOrientationInput>;
	readonly [WORKBENCH_ACTION_IDS.setProjectPrimaryDirectory]: ActionSignature<WorkbenchProjectDirectoryInput>;
	readonly [WORKBENCH_ACTION_IDS.toggleActivePanelMaximization]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.toggleProjectExpansion]: ActionSignature<WorkbenchProjectInput>;
	readonly [WORKBENCH_ACTION_IDS.toggleSidebar]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.updateWorkspace]: ActionSignature<WorkbenchWorkspaceUpdateInput>;
}

export const workbenchAgentNavigateInput: ActionInputValidation<WorkbenchAgentNavigateInput> = {
	name: 'WorkbenchAgentNavigateInput',
	parse: (input) => {
		if (
			!isRecord(input) ||
			typeof input['projectId'] !== 'string' ||
			typeof input['workspaceId'] !== 'string'
		) {
			return { code: 'expected-workbench-agent-navigate-input', success: false };
		}
		const sessionId = AgentSessionIdSchema.safeParse(input['sessionId']);
		return sessionId.success
			? {
					data: {
						projectId: input['projectId'],
						sessionId: sessionId.data,
						workspaceId: input['workspaceId'],
					},
					success: true,
				}
			: { code: 'expected-workbench-agent-navigate-input', success: false };
	},
};

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

export const workbenchPanelPositionInput: ActionInputValidation<WorkbenchPanelPositionInput> = {
	name: 'WorkbenchPanelPositionInput',
	parse: (input) =>
		isRecord(input) && isWorkbenchPanelPosition(input['position'])
			? {
					data: { position: input['position'] },
					success: true,
				}
			: { code: 'expected-workbench-panel-position-input', success: false },
};

export const workbenchTabPositionInput: ActionInputValidation<WorkbenchTabPositionInput> = {
	name: 'WorkbenchTabPositionInput',
	parse: (input) =>
		isRecord(input) && isWorkbenchTabPosition(input['position'])
			? {
					data: { position: input['position'] },
					success: true,
				}
			: { code: 'expected-workbench-tab-position-input', success: false },
};

export const workbenchTabTitleInput: ActionInputValidation<WorkbenchTabTitleInput> = {
	name: 'WorkbenchTabTitleInput',
	parse: (input) => {
		if (input === undefined) return { data: undefined, success: true };
		if (
			!isRecord(input) ||
			(input['kind'] !== 'open' && input['kind'] !== 'commit') ||
			typeof input['tabId'] !== 'string' ||
			input['tabId'].length === 0 ||
			typeof input['terminalContextId'] !== 'string' ||
			input['terminalContextId'].length === 0
		) {
			return { code: 'expected-workbench-tab-title-input', success: false };
		}
		if (input['kind'] === 'open') {
			return {
				data: {
					kind: 'open' as const,
					tabId: input['tabId'],
					terminalContextId: input['terminalContextId'],
				},
				success: true,
			};
		}
		if (!(input['title'] === null || typeof input['title'] === 'string')) {
			return { code: 'expected-workbench-tab-title-input', success: false };
		}
		return {
			data: {
				kind: 'commit' as const,
				tabId: input['tabId'],
				terminalContextId: input['terminalContextId'],
				title: input['title'],
			},
			success: true,
		};
	},
};

function isWorkbenchPanelPosition(input: unknown): input is WorkbenchPanelPosition {
	return (
		input === 1 ||
		input === 2 ||
		input === 3 ||
		input === 4 ||
		input === 5 ||
		input === 6 ||
		input === 7 ||
		input === 8 ||
		input === 9 ||
		input === 10
	);
}

function isWorkbenchTabPosition(input: unknown): input is WorkbenchTabPosition {
	return (
		input === 1 ||
		input === 2 ||
		input === 3 ||
		input === 4 ||
		input === 5 ||
		input === 6 ||
		input === 7 ||
		input === 8 ||
		input === 9
	);
}

export const workbenchProjectInput: ActionInputValidation<WorkbenchProjectInput> = {
	name: 'WorkbenchProjectInput',
	parse: (input) =>
		isRecord(input) && typeof input['projectId'] === 'string'
			? { success: true, data: { projectId: input['projectId'] } }
			: { success: false, code: 'expected-workbench-project-input' },
};

const workspaceActivationSchema = z
	.object({
		projectId: z.string(),
		workspaceId: z.string(),
		creationReceipt: z
			.strictObject({
				directoryId: z.string().min(1),
				path: z.string().min(1),
				presentationId: z.string().min(1),
				projectId: z.string().min(1),
				terminalContextId: z.string().min(1),
				workspaceId: z.string().min(1),
			})
			.optional(),
	})
	.refine(
		(input) =>
			input.creationReceipt === undefined ||
			(input.creationReceipt.projectId === input.projectId &&
				input.creationReceipt.workspaceId === input.workspaceId),
	);

export const workbenchWorkspaceActivationInput: ActionInputValidation<WorkbenchWorkspaceActivationInput> =
	{
		name: 'WorkbenchWorkspaceActivationInput',
		parse: (input) => {
			const result = workspaceActivationSchema.safeParse(input);
			return result.success
				? { data: result.data, success: true }
				: { code: 'expected-workbench-workspace-activation-input', success: false };
		},
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
