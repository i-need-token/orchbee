import type { ActionInputValidation, ActionSignature } from '@orchbee/actions';
import { z } from 'zod';
import {
	TerminalPresentationIdSchema,
	UserLaunchProfileSchema,
	WorkbenchAgentStartProviderSchema,
	WorkbenchLabelSchema,
	type TerminalPresentationId,
	type UserLaunchProfile,
	type WorkbenchAgentStartProvider,
} from '@orchbee/workbench-client-protocol';

import type { PanelFocusPosition } from '../components/workbench-panel-workspace/model/resolvePanelFocusByPosition';
import { isWorkbenchWorktreeFailureCause } from '../model/workbench-worktree-failure';
import {
	WORKBENCH_AGENT_START_PLACES,
	type WorkbenchAgentStartInput,
} from '../types/workbench-agent-start';
import type { WorkbenchSearchFilter } from '../types/workbench-search';
import type { WorkbenchProjectCreationReceipt } from '../types/workbench-project-folder-acquisition';
import type { WorkbenchWorktreeFailureCause } from '../types/workbench-worktrees';

export const WORKBENCH_ACTION_IDS = {
	navigateAgent: 'workbench.agent.navigate',
	activateWorkspace: 'workbench.workspace.activate',
	createProject: 'workbench.project.create',
	newProject: 'workbench.project.new',
	createTerminal: 'workbench.terminal.create',
	createTerminalInCurrentDirectory: 'workbench.terminal.create-current-directory',
	decreaseTerminalFontSize: 'workbench.terminal.font-size.decrease',
	createWorkspace: 'workbench.workspace.create',
	setWorkspaceProject: 'workbench.workspace.set-project',
	createWorkspaceProject: 'workbench.workspace.create-project',
	closeProjectWorkspaces: 'workbench.project.close-all',
	closeWorkspaceConfirmed: 'workbench.workspace.close-confirmed',
	closeTabConfirmed: 'workbench.tab.close-confirmed',
	closePanelTarget: 'workbench.panel.target.close',
	closePanelTargetConfirmed: 'workbench.panel.target.close-confirmed',
	deleteWorkspace: 'workbench.workspace.delete',
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
	focusLastPanel: 'workbench.panel.focus-last',
	focusNextRegion: 'workbench.focus.next-region',
	focusPreviousRegion: 'workbench.focus.previous-region',
	focusSidebar: 'workbench.focus.sidebar',
	focusTabStrip: 'workbench.focus.tab-strip',
	reopenEarlierLayout: 'workbench.layout.reopen-earlier',
	increaseTerminalFontSize: 'workbench.terminal.font-size.increase',
	openAgentBoard: 'workbench.agent-board.open',
	openAgentStart: 'workbench.agent.start',
	startAgent: 'workbench.agent.start-provider',
	openSearch: 'workbench.search.open',
	openSettings: 'workbench.settings.open',
	openShortcuts: 'workbench.shortcuts.open',
	removeProject: 'workbench.project.dissolve',
	resetTerminalFontSize: 'workbench.terminal.font-size.reset',
	resizeActivePanelDown: 'workbench.panel.active.resize-down',
	resizeActivePanelLeft: 'workbench.panel.active.resize-left',
	resizeActivePanelRight: 'workbench.panel.active.resize-right',
	resizeActivePanelUp: 'workbench.panel.active.resize-up',
	renameProject: 'workbench.project.rename',
	reorderProject: 'workbench.project.reorder',
	reorderWorkspace: 'workbench.workspace.reorder',
	moveActiveTabBackward: 'workbench.tab.active.move-backward',
	moveActiveTabForward: 'workbench.tab.active.move-forward',
	closeActiveTab: 'workbench.tab.active.close',
	closeTabsAfterActive: 'workbench.tab.active.close-after',
	createTabAfterActive: 'workbench.tab.active.create-after',
	duplicateActiveTabs: 'workbench.tab.active.duplicate',
	moveActivePanelDown: 'workbench.panel.active.move-down',
	moveActivePanelLeft: 'workbench.panel.active.move-left',
	moveActivePanelRight: 'workbench.panel.active.move-right',
	moveActivePanelUp: 'workbench.panel.active.move-up',
	resumeTabClose: 'workbench.tab.close.resume',
	setTabTitle: 'workbench.tab.title.set',
	renamePanel: 'workbench.panel.rename',
	selectNextTab: 'workbench.tab.select-next',
	selectPreviousTab: 'workbench.tab.select-previous',
	selectTabByPosition: 'workbench.tab.select-by-position',
	closeActiveTerminal: 'workbench.terminal.active.close',
	splitActivePanelAbove: 'workbench.panel.active.split-above',
	splitActivePanelBelow: 'workbench.panel.active.split-below',
	splitActivePanelLeft: 'workbench.panel.active.split-left',
	splitActivePanelRight: 'workbench.panel.active.split-right',
	swapActivePanelVisible: 'workbench.panel.active.swap-visible',
	toggleActivePanelMaximization: 'workbench.panel.active.maximization.toggle',
	toggleProjectExpansion: 'workbench.project.expansion.toggle',
	toggleRecentsExpansion: 'workbench.recents.expansion.toggle',
	toggleSidebar: 'workbench.sidebar.toggle',
	updateWorkspace: 'workbench.workspace.update',
	createWorktree: 'workbench.worktree.create',
	createFocusedPanelWorktree: 'workbench.worktree.active.create',
	openWorktree: 'workbench.worktree.open',
	openFocusedPanelWorktree: 'workbench.worktree.active.open',
	removeWorktree: 'workbench.worktree.remove',
	resumeWorktreeRemoval: 'workbench.worktree.resume-removal',
} as const;

export interface WorkbenchAgentNavigateInput {
	readonly presentationId: TerminalPresentationId;
	readonly workspaceId: string;
}

export interface WorkbenchProjectInput {
	readonly projectId: string;
}

interface WorkbenchProjectCreateInput {
	readonly path: string;
}

/** Where a new Project's folder comes from: one that exists, or one created for it. */
export interface WorkbenchProjectNewInput {
	readonly folder: 'existing' | 'new';
}

/** A checkout the Workspace's menu offered to open. */
export interface WorkbenchWorktreeCheckoutInput {
	readonly path: string;
	readonly workspaceId: string;
}

/** A checkout Git listed where the focused Panel's terminal stands, read for that Panel. */
export interface WorkbenchFocusedPanelWorktreeCheckoutInput extends WorkbenchWorktreeCheckoutInput {
	readonly panelId: string;
	readonly presentationId: string;
}

/** The Workspace's own checkout as its menu showed it, which the confirmation repeats. */
export interface WorkbenchWorktreeRemovalInput {
	readonly branch: string | null;
	readonly path: string;
	readonly terminalCount: number;
	readonly workspaceId: string;
}

/** A removal the authority reported as stopped where it may continue. */
export interface WorkbenchWorktreeResumeInput {
	readonly cause: WorkbenchWorktreeFailureCause;
	readonly operationId: string;
}

/** The Workspace a Worktree command opened. */
export interface WorkbenchWorktreeOpening {
	readonly workspaceId: string;
}

export interface WorkbenchWorkspaceInput {
	readonly workspaceId: string;
}

interface WorkbenchWorkspaceActivationInput extends WorkbenchWorkspaceInput {
	readonly creationReceipt?: WorkbenchProjectCreationReceipt | undefined;
}

/** Nothing for a default Workspace, a Project for a default one in it, or a recorded directory. */
type WorkbenchWorkspaceCreateInput =
	| void
	| { readonly projectId: string }
	| { readonly projectId: string; readonly path: string }
	| {
			readonly projectId: string | null;
			readonly directoryId: string;
	  };

interface WorkbenchWorkspaceProjectInput extends WorkbenchWorkspaceInput {
	readonly projectId: string | null;
}
interface WorkbenchWorkspaceProjectCreateInput extends WorkbenchWorkspaceInput {
	readonly name: string;
}

interface WorkbenchProjectRenameInput extends WorkbenchProjectInput {
	readonly name: string;
}

interface WorkbenchProjectReorderInput extends WorkbenchProjectInput {
	readonly beforeProjectId: string | null;
}

interface WorkbenchWorkspaceUpdateInput extends WorkbenchWorkspaceInput {
	readonly name: string;
}

/** A place in navigation: before a Workspace of `projectId`'s list, or at its end when null. */
interface WorkbenchWorkspaceReorderInput extends WorkbenchWorkspaceInput {
	readonly beforeWorkspaceId: string | null;
	/** The destination Project, or null for the Workspaces outside any Project. */
	readonly projectId: string | null;
}

export type WorkbenchPanelPosition = PanelFocusPosition;

/** One earlier layout, named by its save time in Unix milliseconds. */
export interface WorkbenchEarlierLayoutInput {
	readonly savedAt: number;
}

export interface WorkbenchPanelPositionInput {
	readonly position: WorkbenchPanelPosition;
}

export type WorkbenchTabPosition = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9;

export interface WorkbenchTabPositionInput {
	readonly position: WorkbenchTabPosition;
}

/** A Tab named exactly, or the active Tab when absent. */
export type WorkbenchTabTargetInput =
	| undefined
	| { readonly tabId: string; readonly terminalContextId: string };

/** One or more distinct Tabs of one terminal context, named exactly. */
export type WorkbenchTabSetTargetInput =
	| undefined
	| { readonly tabIds: readonly string[]; readonly terminalContextId: string };

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

/** A Panel named by its location, whichever Workspace shows it. */
export type WorkbenchPanelTargetInput = {
	readonly panelId: string;
	readonly tabId: string;
	readonly terminalContextId: string;
};

export type WorkbenchPanelNameInput =
	| undefined
	| {
			readonly kind: 'open';
			readonly panelId: string;
			readonly tabId: string;
			readonly terminalContextId: string;
	  }
	| {
			readonly kind: 'commit';
			readonly panelId: string;
			readonly tabId: string;
			readonly terminalContextId: string;
			readonly presentationId: string;
			readonly title: string;
	  };

export interface WorkbenchTerminalCreateInput {
	readonly directoryId?: string | undefined;
	readonly groupId: string | null;
	readonly index?: number | undefined;
	/** A saved launch profile to start with once, as the authority's Tab creation describes. */
	readonly profile?: UserLaunchProfile | undefined;
}

/** An Agent a created terminal runs first; without it the terminal starts its shell. */
export type WorkbenchAgentStartOption = void | { readonly agentStart: WorkbenchAgentStartProvider };

export interface WorkbenchSearchOpenInput {
	readonly filter: WorkbenchSearchFilter;
}

export interface WorkbenchActionCatalog {
	readonly [WORKBENCH_ACTION_IDS.setWorkspaceProject]: ActionSignature<WorkbenchWorkspaceProjectInput>;
	readonly [WORKBENCH_ACTION_IDS.createWorkspaceProject]: ActionSignature<WorkbenchWorkspaceProjectCreateInput>;
	readonly [WORKBENCH_ACTION_IDS.closeProjectWorkspaces]: ActionSignature<WorkbenchProjectInput>;
	readonly [WORKBENCH_ACTION_IDS.closeWorkspaceConfirmed]: ActionSignature<WorkbenchWorkspaceInput>;
	readonly [WORKBENCH_ACTION_IDS.closeTabConfirmed]: ActionSignature<WorkbenchTabSetTargetInput>;
	readonly [WORKBENCH_ACTION_IDS.closePanelTarget]: ActionSignature<WorkbenchPanelTargetInput>;
	readonly [WORKBENCH_ACTION_IDS.closePanelTargetConfirmed]: ActionSignature<WorkbenchPanelTargetInput>;
	readonly [WORKBENCH_ACTION_IDS.navigateAgent]: ActionSignature<WorkbenchAgentNavigateInput>;
	readonly [WORKBENCH_ACTION_IDS.activateWorkspace]: ActionSignature<WorkbenchWorkspaceActivationInput>;
	readonly [WORKBENCH_ACTION_IDS.createProject]: ActionSignature<
		WorkbenchProjectCreateInput,
		WorkbenchProjectCreationReceipt
	>;
	readonly [WORKBENCH_ACTION_IDS.newProject]: ActionSignature<WorkbenchProjectNewInput>;
	readonly [WORKBENCH_ACTION_IDS.createTerminal]: ActionSignature<WorkbenchTerminalCreateInput>;
	readonly [WORKBENCH_ACTION_IDS.createTerminalInCurrentDirectory]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.decreaseTerminalFontSize]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.createWorkspace]: ActionSignature<WorkbenchWorkspaceCreateInput>;
	readonly [WORKBENCH_ACTION_IDS.deleteWorkspace]: ActionSignature<WorkbenchWorkspaceInput>;
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
	readonly [WORKBENCH_ACTION_IDS.focusLastPanel]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusNextRegion]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusPreviousRegion]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusSidebar]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.focusTabStrip]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.reopenEarlierLayout]: ActionSignature<WorkbenchEarlierLayoutInput>;
	readonly [WORKBENCH_ACTION_IDS.increaseTerminalFontSize]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.openAgentBoard]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.openAgentStart]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.startAgent]: ActionSignature<WorkbenchAgentStartInput>;
	readonly [WORKBENCH_ACTION_IDS.openSearch]: ActionSignature<WorkbenchSearchOpenInput>;
	readonly [WORKBENCH_ACTION_IDS.openSettings]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.openShortcuts]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.removeProject]: ActionSignature<WorkbenchProjectInput>;
	readonly [WORKBENCH_ACTION_IDS.resetTerminalFontSize]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.resizeActivePanelDown]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.resizeActivePanelLeft]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.resizeActivePanelRight]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.resizeActivePanelUp]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.renameProject]: ActionSignature<WorkbenchProjectRenameInput>;
	readonly [WORKBENCH_ACTION_IDS.reorderProject]: ActionSignature<WorkbenchProjectReorderInput>;
	readonly [WORKBENCH_ACTION_IDS.reorderWorkspace]: ActionSignature<WorkbenchWorkspaceReorderInput>;
	readonly [WORKBENCH_ACTION_IDS.moveActiveTabBackward]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.moveActiveTabForward]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.closeActiveTab]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.closeTabsAfterActive]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.createTabAfterActive]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.duplicateActiveTabs]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.moveActivePanelDown]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.moveActivePanelLeft]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.moveActivePanelRight]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.moveActivePanelUp]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.resumeTabClose]: ActionSignature<WorkbenchTabTargetInput>;
	readonly [WORKBENCH_ACTION_IDS.setTabTitle]: ActionSignature<WorkbenchTabTitleInput>;
	readonly [WORKBENCH_ACTION_IDS.renamePanel]: ActionSignature<WorkbenchPanelNameInput>;
	readonly [WORKBENCH_ACTION_IDS.selectNextTab]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.selectPreviousTab]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.selectTabByPosition]: ActionSignature<WorkbenchTabPositionInput>;
	readonly [WORKBENCH_ACTION_IDS.closeActiveTerminal]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.splitActivePanelAbove]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.splitActivePanelBelow]: ActionSignature<WorkbenchAgentStartOption>;
	readonly [WORKBENCH_ACTION_IDS.splitActivePanelLeft]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.splitActivePanelRight]: ActionSignature<WorkbenchAgentStartOption>;
	readonly [WORKBENCH_ACTION_IDS.swapActivePanelVisible]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.toggleActivePanelMaximization]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.toggleProjectExpansion]: ActionSignature<WorkbenchProjectInput>;
	readonly [WORKBENCH_ACTION_IDS.toggleRecentsExpansion]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.toggleSidebar]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.updateWorkspace]: ActionSignature<WorkbenchWorkspaceUpdateInput>;
	readonly [WORKBENCH_ACTION_IDS.createWorktree]: ActionSignature<
		WorkbenchWorkspaceInput,
		WorkbenchWorktreeOpening
	>;
	readonly [WORKBENCH_ACTION_IDS.createFocusedPanelWorktree]: ActionSignature<
		WorkbenchAgentStartOption,
		WorkbenchWorktreeOpening
	>;
	readonly [WORKBENCH_ACTION_IDS.openWorktree]: ActionSignature<
		WorkbenchWorktreeCheckoutInput,
		WorkbenchWorktreeOpening
	>;
	readonly [WORKBENCH_ACTION_IDS.openFocusedPanelWorktree]: ActionSignature<
		WorkbenchFocusedPanelWorktreeCheckoutInput,
		WorkbenchWorktreeOpening
	>;
	readonly [WORKBENCH_ACTION_IDS.removeWorktree]: ActionSignature<WorkbenchWorktreeRemovalInput>;
	readonly [WORKBENCH_ACTION_IDS.resumeWorktreeRemoval]: ActionSignature<WorkbenchWorktreeResumeInput>;
}

export const workbenchAgentNavigateInput: ActionInputValidation<WorkbenchAgentNavigateInput> = {
	name: 'WorkbenchAgentNavigateInput',
	parse: (input) => {
		if (!isRecord(input) || typeof input['workspaceId'] !== 'string') {
			return { code: 'expected-workbench-agent-navigate-input', success: false };
		}
		const presentationId = TerminalPresentationIdSchema.safeParse(input['presentationId']);
		return presentationId.success
			? {
					data: {
						presentationId: presentationId.data,
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
		const profile =
			input['profile'] === undefined
				? undefined
				: UserLaunchProfileSchema.safeParse(input['profile']);
		if (profile?.success === false) {
			return { success: false, code: 'expected-terminal-create-input' };
		}
		return {
			success: true,
			data: {
				...(input['directoryId'] === undefined ? {} : { directoryId: input['directoryId'] }),
				groupId: input['groupId'],
				...(input['index'] === undefined ? {} : { index: input['index'] }),
				...(profile === undefined ? {} : { profile: profile.data }),
			},
		};
	},
};

export const workbenchAgentStartOption: ActionInputValidation<WorkbenchAgentStartOption> = {
	name: 'WorkbenchAgentStartOption',
	parse: (input) => {
		if (input === undefined) return { success: true, data: undefined };
		const agentStart = isRecord(input)
			? WorkbenchAgentStartProviderSchema.safeParse(input['agentStart'])
			: null;
		return agentStart?.success === true
			? { success: true, data: { agentStart: agentStart.data } }
			: { success: false, code: 'expected-workbench-agent-start-option' };
	},
};

export const workbenchAgentStartInput: ActionInputValidation<WorkbenchAgentStartInput> = {
	name: 'WorkbenchAgentStartInput',
	parse: (input) => {
		if (!isRecord(input)) return { success: false, code: 'expected-workbench-agent-start' };
		const providerId = WorkbenchAgentStartProviderSchema.safeParse(input['providerId']);
		const place = WORKBENCH_AGENT_START_PLACES.find((candidate) => candidate === input['place']);
		return providerId.success && place !== undefined
			? { success: true, data: { place, providerId: providerId.data } }
			: { success: false, code: 'expected-workbench-agent-start' };
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

export const workbenchTabTargetInput: ActionInputValidation<WorkbenchTabTargetInput> = {
	name: 'WorkbenchTabTargetInput',
	parse: (input) => {
		if (input === undefined) return { data: undefined, success: true };
		return isRecord(input) &&
			typeof input['tabId'] === 'string' &&
			input['tabId'].length > 0 &&
			typeof input['terminalContextId'] === 'string' &&
			input['terminalContextId'].length > 0
			? {
					data: { tabId: input['tabId'], terminalContextId: input['terminalContextId'] },
					success: true,
				}
			: { code: 'expected-workbench-tab-target-input', success: false };
	},
};

export const workbenchTabSetTargetInput: ActionInputValidation<WorkbenchTabSetTargetInput> = {
	name: 'WorkbenchTabSetTargetInput',
	parse: (input) => {
		if (input === undefined) return { data: undefined, success: true };
		if (!isRecord(input))
			return { code: 'expected-workbench-tab-set-target-input', success: false };
		const { tabIds, terminalContextId } = input;
		return Array.isArray(tabIds) &&
			tabIds.length > 0 &&
			tabIds.every((tabId): tabId is string => typeof tabId === 'string' && tabId.length > 0) &&
			new Set(tabIds).size === tabIds.length &&
			typeof terminalContextId === 'string' &&
			terminalContextId.length > 0
			? { data: { tabIds: [...tabIds], terminalContextId }, success: true }
			: { code: 'expected-workbench-tab-set-target-input', success: false };
	},
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

export const workbenchPanelTargetInput: ActionInputValidation<WorkbenchPanelTargetInput> = {
	name: 'WorkbenchPanelTargetInput',
	parse: (input) => {
		if (
			!isRecord(input) ||
			typeof input['panelId'] !== 'string' ||
			typeof input['tabId'] !== 'string' ||
			typeof input['terminalContextId'] !== 'string'
		)
			return { success: false, code: 'expected-panel-target-input' };
		return {
			success: true,
			data: {
				panelId: input['panelId'],
				tabId: input['tabId'],
				terminalContextId: input['terminalContextId'],
			},
		};
	},
};

export const workbenchPanelNameInput: ActionInputValidation<WorkbenchPanelNameInput> = {
	name: 'WorkbenchPanelNameInput',
	parse: (input) => {
		if (input === undefined) return { success: true, data: undefined };
		if (
			!isRecord(input) ||
			typeof input['panelId'] !== 'string' ||
			typeof input['tabId'] !== 'string' ||
			typeof input['terminalContextId'] !== 'string'
		)
			return { success: false, code: 'expected-panel-name-input' };
		const target = {
			panelId: input['panelId'],
			tabId: input['tabId'],
			terminalContextId: input['terminalContextId'],
		};
		if (input['kind'] === 'open') return { success: true, data: { ...target, kind: 'open' } };
		if (
			input['kind'] === 'commit' &&
			typeof input['presentationId'] === 'string' &&
			typeof input['title'] === 'string'
		)
			return {
				success: true,
				data: {
					...target,
					kind: 'commit',
					presentationId: input['presentationId'],
					title: input['title'],
				},
			};
		return { success: false, code: 'expected-panel-name-input' };
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
		workspaceId: z.string(),
		creationReceipt: z
			.strictObject({
				directoryId: z.string().min(1),
				path: z.string().min(1),
				presentationId: z.string().min(1),
				projectId: z.string().min(1).nullable(),
				terminalContextId: z.string().min(1),
				workspaceId: z.string().min(1),
			})
			.optional(),
	})
	.refine(
		(input) =>
			input.creationReceipt === undefined ||
			input.creationReceipt.workspaceId === input.workspaceId,
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

export const workbenchProjectNewInput: ActionInputValidation<WorkbenchProjectNewInput> = {
	name: 'WorkbenchProjectNewInput',
	parse: (input) =>
		isRecord(input) && (input['folder'] === 'existing' || input['folder'] === 'new')
			? { success: true, data: { folder: input['folder'] } }
			: { success: false, code: 'expected-workbench-project-new-input' },
};

export const workbenchProjectRenameInput: ActionInputValidation<WorkbenchProjectRenameInput> = {
	name: 'WorkbenchProjectRenameInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['name'] === 'string' &&
		WorkbenchLabelSchema.safeParse(input['name']).success &&
		typeof input['projectId'] === 'string'
			? {
					success: true,
					data: {
						name: input['name'],
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

export const workbenchWorktreeCheckoutInput: ActionInputValidation<WorkbenchWorktreeCheckoutInput> =
	{
		name: 'WorkbenchWorktreeCheckoutInput',
		parse: (input) =>
			isRecord(input) &&
			typeof input['path'] === 'string' &&
			typeof input['workspaceId'] === 'string'
				? { success: true, data: { path: input['path'], workspaceId: input['workspaceId'] } }
				: { success: false, code: 'expected-workbench-worktree-checkout-input' },
	};

export const workbenchFocusedPanelWorktreeCheckoutInput: ActionInputValidation<WorkbenchFocusedPanelWorktreeCheckoutInput> =
	{
		name: 'WorkbenchFocusedPanelWorktreeCheckoutInput',
		parse: (input) =>
			isRecord(input) &&
			typeof input['panelId'] === 'string' &&
			typeof input['path'] === 'string' &&
			typeof input['presentationId'] === 'string' &&
			typeof input['workspaceId'] === 'string'
				? {
						success: true,
						data: {
							panelId: input['panelId'],
							path: input['path'],
							presentationId: input['presentationId'],
							workspaceId: input['workspaceId'],
						},
					}
				: { success: false, code: 'expected-workbench-focused-panel-worktree-checkout-input' },
	};

export const workbenchWorktreeRemovalInput: ActionInputValidation<WorkbenchWorktreeRemovalInput> = {
	name: 'WorkbenchWorktreeRemovalInput',
	parse: (input) =>
		isRecord(input) &&
		(input['branch'] === null || typeof input['branch'] === 'string') &&
		typeof input['path'] === 'string' &&
		typeof input['terminalCount'] === 'number' &&
		Number.isSafeInteger(input['terminalCount']) &&
		input['terminalCount'] >= 0 &&
		typeof input['workspaceId'] === 'string'
			? {
					success: true,
					data: {
						branch: input['branch'],
						path: input['path'],
						terminalCount: input['terminalCount'],
						workspaceId: input['workspaceId'],
					},
				}
			: { success: false, code: 'expected-workbench-worktree-removal-input' },
};

export const workbenchWorktreeResumeInput: ActionInputValidation<WorkbenchWorktreeResumeInput> = {
	name: 'WorkbenchWorktreeResumeInput',
	parse: (input) =>
		isRecord(input) &&
		isWorkbenchWorktreeFailureCause(input['cause']) &&
		typeof input['operationId'] === 'string'
			? { success: true, data: { cause: input['cause'], operationId: input['operationId'] } }
			: { success: false, code: 'expected-workbench-worktree-resume-input' },
};

export const workbenchWorkspaceInput: ActionInputValidation<WorkbenchWorkspaceInput> = {
	name: 'WorkbenchWorkspaceInput',
	parse: (input) =>
		isRecord(input) && typeof input['workspaceId'] === 'string'
			? {
					success: true,
					data: {
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
			(input['projectId'] === null || typeof input['projectId'] === 'string') &&
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

const workspaceCreateSchema = z.union([
	z.strictObject({ projectId: z.string().min(1), path: z.string().trim().min(1) }),
	z.strictObject({
		projectId: z.string().min(1).nullable(),
		directoryId: z.string().min(1),
	}),
	z.strictObject({ projectId: z.string().min(1) }),
]);
export const workbenchWorkspaceCreateInput: ActionInputValidation<WorkbenchWorkspaceCreateInput> = {
	name: 'WorkbenchWorkspaceCreateInput',
	parse: (input) => {
		if (input === undefined) return { success: true, data: undefined };
		const result = workspaceCreateSchema.safeParse(input);
		return result.success
			? { success: true, data: result.data }
			: { success: false, code: 'expected-workbench-workspace-create-input' };
	},
};
const workspaceProjectSchema = z.strictObject({
	workspaceId: z.string().min(1),
	projectId: z.string().min(1).nullable(),
});
export const workbenchWorkspaceProjectInput: ActionInputValidation<WorkbenchWorkspaceProjectInput> =
	{
		name: 'WorkbenchWorkspaceProjectInput',
		parse: (input) => {
			const result = workspaceProjectSchema.safeParse(input);
			return result.success
				? { success: true, data: result.data }
				: { success: false, code: 'expected-workbench-workspace-project-input' };
		},
	};
const workspaceProjectCreateSchema = z.strictObject({
	workspaceId: z.string().min(1),
	name: WorkbenchLabelSchema,
});
export const workbenchWorkspaceProjectCreateInput: ActionInputValidation<WorkbenchWorkspaceProjectCreateInput> =
	{
		name: 'WorkbenchWorkspaceProjectCreateInput',
		parse: (input) => {
			const result = workspaceProjectCreateSchema.safeParse(input);
			return result.success
				? { success: true, data: result.data }
				: { success: false, code: 'expected-workbench-workspace-project-create-input' };
		},
	};

export const workbenchWorkspaceUpdateInput: ActionInputValidation<WorkbenchWorkspaceUpdateInput> = {
	name: 'WorkbenchWorkspaceUpdateInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['name'] === 'string' &&
		WorkbenchLabelSchema.safeParse(input['name']).success &&
		typeof input['workspaceId'] === 'string'
			? {
					success: true,
					data: {
						name: input['name'],
						workspaceId: input['workspaceId'],
					},
				}
			: { success: false, code: 'expected-workbench-workspace-update-input' },
};

export const workbenchEarlierLayoutInput: ActionInputValidation<WorkbenchEarlierLayoutInput> = {
	name: 'WorkbenchEarlierLayoutInput',
	parse: (input) =>
		typeof input === 'object' &&
		input !== null &&
		'savedAt' in input &&
		Number.isSafeInteger(input.savedAt) &&
		typeof input.savedAt === 'number' &&
		input.savedAt > 0
			? { success: true, data: { savedAt: input.savedAt } }
			: { success: false, code: 'expected-workbench-earlier-layout-input' },
};
