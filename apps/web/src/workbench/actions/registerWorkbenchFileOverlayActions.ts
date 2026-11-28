import {
	defineAction,
	noActionInput,
	type ActionAvailability,
	type ActionDisposable,
	type ActionMessageDescriptor,
	type ActionResult,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { fileOverlayMessages, workbenchMessages } from '@orchbee/i18n';

import type { FileOverlayEvent, FileOverlayState } from '../../file-overlay';
import {
	WORKBENCH_ACTION_IDS,
	workbenchFileOverlayTabInput,
	workbenchFileOverlayTabTargetInput,
	workbenchTabPositionInput,
	type WorkbenchActionCatalog,
	type WorkbenchFileOverlayTabInput,
} from './workbench-action-ids';

interface RegisterWorkbenchFileOverlayActionsOptions {
	/** Applies an event to the shown Workspace's overlay and carries out what it asks of the Workbench. */
	readonly dispatch: (event: FileOverlayEvent) => void;
	/** Gives the shown overlay's active tab the keys once the overlay shows it. */
	readonly focusOverlay: () => void;
	/** The shown Workspace's overlay now, read whenever an Action asks. */
	readonly getState: () => FileOverlayState;
	/**
	 * Lists the tabs, the most recently active first, for the person to pick one; answers its id, or
	 * null once the pick is cancelled or the signal aborts.
	 */
	readonly pickTab: (signal: AbortSignal) => Promise<string | null>;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
	/** The Workspace whose overlay shows; a tab named in another one is not offered. */
	readonly workspaceId: string;
}

/** A tab of the shown overlay that an Action names, and what the row shows of it. */
interface NamedFileTab {
	readonly id: string;
	readonly preview: boolean;
	readonly sticky: boolean;
}

type FileTabs = FileOverlayState['tabs'];

/** The Actions that show a tab they pick from the row by its place. */
type PickedFileTabActionId =
	| typeof WORKBENCH_ACTION_IDS.selectLastFileOverlayTab
	| typeof WORKBENCH_ACTION_IDS.selectNextFileOverlayTab
	| typeof WORKBENCH_ACTION_IDS.selectPreviousFileOverlayTab;

interface PickedFileTabAction {
	readonly id: PickedFileTabActionId;
	/** How many tabs the row needs before there is a tab to show. */
	readonly minimum: number;
	/** The tab to show from the row as it is now. */
	readonly pick: (tabs: FileTabs) => string | undefined;
	readonly title: ActionMessageDescriptor;
}

function activeIndex(tabs: FileTabs): number {
	return tabs.tabs.findIndex((tab) => tab.id === tabs.activeId);
}

/**
 * Open Next Editor and Open Previous Editor go round the row, as VS Code's do within its one editor
 * group; Open Last Editor shows the last tab.
 */
const PICKED_FILE_TAB_ACTIONS: readonly PickedFileTabAction[] = [
	{
		// VS Code parity: workbench.action.nextEditor.
		id: WORKBENCH_ACTION_IDS.selectNextFileOverlayTab,
		minimum: 2,
		pick: (tabs) => tabs.tabs[(activeIndex(tabs) + 1) % tabs.tabs.length]?.id,
		title: fileOverlayMessages.nextEditor,
	},
	{
		// VS Code parity: workbench.action.previousEditor.
		id: WORKBENCH_ACTION_IDS.selectPreviousFileOverlayTab,
		minimum: 2,
		pick: (tabs) => {
			const index = activeIndex(tabs);
			return tabs.tabs[index <= 0 ? tabs.tabs.length - 1 : index - 1]?.id;
		},
		title: fileOverlayMessages.previousEditor,
	},
	{
		// VS Code parity: workbench.action.lastEditorInGroup.
		id: WORKBENCH_ACTION_IDS.selectLastFileOverlayTab,
		minimum: 1,
		pick: (tabs) => tabs.tabs.at(-1)?.id,
		title: fileOverlayMessages.lastEditor,
	},
];

const VIEW_CATEGORY = {
	id: 'view',
	title: workbenchMessages.actionCategoryView,
};

const ENABLED: ActionAvailability = { kind: 'enabled' };
const DISABLED: ActionAvailability = { kind: 'disabled' };
const HIDDEN: ActionAvailability = { kind: 'hidden' };
/** The tab is gone, or the overlay that showed it is no longer shown. */
const SCOPE_CHANGED: ActionResult<void> = { code: 'scope-changed', kind: 'blocked' };

/**
 * The file overlay's Actions. They exist only in a window that offers the File Bar, for the shown
 * Workspace's overlay. A tab Action names its tab with the Workspace that shows it, or without input
 * takes the active tab, as VS Code's editor commands take the active editor. Each is offered only
 * while it has a tab to act on, so that its keys otherwise keep the meaning they have outside the
 * File Bar; one that VS Code shows greyed in a tab's menu is disabled where VS Code's precondition
 * fails, which passes its keys on as well.
 */
export function registerWorkbenchFileOverlayActions({
	dispatch,
	focusOverlay,
	getState,
	pickTab,
	scope,
	typedRuntime,
	workspaceId,
}: RegisterWorkbenchFileOverlayActionsOptions): readonly ActionDisposable[] {
	// Aborts once the Actions go, as when another Workspace shows, so a pick still open ends with them.
	const registered = new AbortController();
	const namedTab = (input: WorkbenchFileOverlayTabInput): NamedFileTab | null => {
		if (input !== undefined && input.workspaceId !== workspaceId) return null;
		const { tabs } = getState();
		const id = input?.tabId ?? tabs.activeId;
		const index = tabs.tabs.findIndex((tab) => tab.id === id);
		const tab = tabs.tabs[index];
		return tab === undefined
			? null
			: { id: tab.id, preview: tabs.previewId === tab.id, sticky: index < tabs.stickyCount };
	};
	/** Applies the event and says whether it changed the overlay. */
	const apply = (event: FileOverlayEvent): ActionResult<void> => {
		const before = getState();
		dispatch(event);
		return getState() === before ? { kind: 'unchanged' } : { kind: 'completed' };
	};
	const tabConcurrencyKey = (input: WorkbenchFileOverlayTabInput): string =>
		input === undefined ? `${workspaceId}:active` : `${input.workspaceId}:${input.tabId}`;
	const tabExecution = {
		concurrency: 'parallel',
		concurrencyKey: tabConcurrencyKey,
		mode: 'foreground',
	} as const;
	const rowExecution = {
		concurrency: 'parallel',
		concurrencyKey: () => `${workspaceId}:row`,
		mode: 'foreground',
	} as const;
	const rowAvailability = (): ActionAvailability =>
		getState().tabs.tabs.length === 0 ? HIDDEN : ENABLED;
	const selectionExecution = {
		concurrency: 'parallel',
		concurrencyKey: () => `${workspaceId}:selection`,
		mode: 'foreground',
	} as const;
	return [
		{ dispose: () => registered.abort() },
		...PICKED_FILE_TAB_ACTIONS.map(({ id, minimum, pick, title }) =>
			typedRuntime.register(
				scope,
				defineAction({
					category: VIEW_CATEGORY,
					execution: selectionExecution,
					getAvailability: () => (getState().tabs.tabs.length < minimum ? HIDDEN : ENABLED),
					handler: {
						execute: (context) => {
							const picked = pick(getState().tabs);
							if (picked === undefined) return SCOPE_CHANGED;
							context.markCommitted();
							return apply({ id: picked, type: 'tab-activated' });
						},
					},
					id,
					input: noActionInput,
					title,
				}),
			),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution: selectionExecution,
				getAvailability: (_context, input) =>
					getState().tabs.tabs[input.position - 1] === undefined ? HIDDEN : ENABLED,
				handler: {
					execute: (context, input) => {
						const tab = getState().tabs.tabs[input.position - 1];
						if (tab === undefined) return SCOPE_CHANGED;
						context.markCommitted();
						return apply({ id: tab.id, type: 'tab-activated' });
					},
				},
				// VS Code parity: workbench.action.openEditorAtIndex1 to 9.
				id: WORKBENCH_ACTION_IDS.selectFileOverlayTabByPosition,
				input: workbenchTabPositionInput,
				title: fileOverlayMessages.positionEditor,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution: selectionExecution,
				getAvailability: rowAvailability,
				handler: {
					execute: async (context) => {
						if (getState().tabs.tabs.length === 0) return SCOPE_CHANGED;
						const picking = new AbortController();
						const stop = (): void => picking.abort();
						context.signal.addEventListener('abort', stop, { once: true });
						registered.signal.addEventListener('abort', stop, { once: true });
						let picked: string | null;
						try {
							picked = await pickTab(picking.signal);
						} finally {
							context.signal.removeEventListener('abort', stop);
							registered.signal.removeEventListener('abort', stop);
						}
						if (picked === null || picking.signal.aborted) return { kind: 'cancelled' };
						if (!getState().tabs.tabs.some((tab) => tab.id === picked)) return SCOPE_CHANGED;
						context.markCommitted();
						const result = apply({ id: picked, type: 'tab-activated' });
						// VS Code's picker opens the editor and focuses it.
						focusOverlay();
						return result;
					},
				},
				// VS Code parity: workbench.action.showEditorsInActiveGroup, and Show Opened Editors in the
				// editor title's menu, workbench.action.showEditorsInGroup.
				id: WORKBENCH_ACTION_IDS.showRecentFileOverlayTabs,
				input: noActionInput,
				title: fileOverlayMessages.showEditorsByMostRecentlyUsed,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => `${workspaceId}:reopen`,
					mode: 'foreground',
				},
				getAvailability: () => (getState().tabs.closed.length === 0 ? HIDDEN : ENABLED),
				handler: {
					execute: (context) => {
						if (getState().tabs.closed.length === 0) return SCOPE_CHANGED;
						context.markCommitted();
						return apply({ type: 'reopen-requested' });
					},
				},
				// VS Code parity: workbench.action.reopenClosedEditor.
				id: WORKBENCH_ACTION_IDS.reopenFileOverlayTab,
				input: noActionInput,
				title: fileOverlayMessages.reopenClosedEditor,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => workspaceId,
					mode: 'foreground',
				},
				getAvailability: () => {
					const { mode } = getState();
					return mode.kind === 'open' ? { checked: mode.tier === 'full', kind: 'enabled' } : HIDDEN;
				},
				handler: {
					execute: (context) => {
						context.markCommitted();
						return apply({ type: 'full-toggled' });
					},
				},
				// VS Code parity: workbench.action.toggleMaximizeEditorGroup.
				id: WORKBENCH_ACTION_IDS.toggleFileOverlayFull,
				input: noActionInput,
				title: fileOverlayMessages.toggleFull,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution: tabExecution,
				getAvailability: (_context, input) => (namedTab(input) === null ? HIDDEN : ENABLED),
				handler: {
					execute: (context, input) => {
						const tab = namedTab(input);
						if (tab === null) return SCOPE_CHANGED;
						context.markCommitted();
						return apply({ id: tab.id, type: 'tab-activated' });
					},
				},
				// No VS Code command names one editor; the nearest is workbench.action.openEditorAtIndex.
				id: WORKBENCH_ACTION_IDS.selectFileOverlayTab,
				input: workbenchFileOverlayTabTargetInput,
				title: fileOverlayMessages.showEditor,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution: tabExecution,
				getAvailability: (_context, input) => (namedTab(input) === null ? HIDDEN : ENABLED),
				handler: {
					execute: (context, input) => {
						const tab = namedTab(input);
						if (tab === null) return SCOPE_CHANGED;
						context.markCommitted();
						// Without a named tab it is Close Editor from a key, which spares a sticky tab.
						return apply({
							target: input === undefined ? { kind: 'active' } : { ids: [tab.id], kind: 'tabs' },
							type: 'close-requested',
						});
					},
				},
				// VS Code parity: workbench.action.closeActiveEditor, or a tab's close button.
				id: WORKBENCH_ACTION_IDS.closeFileOverlayTab,
				input: workbenchFileOverlayTabInput,
				title: fileOverlayMessages.closeEditor,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution: tabExecution,
				// VS Code's precondition: another tab to close.
				getAvailability: (_context, input) => {
					if (namedTab(input) === null) return HIDDEN;
					return getState().tabs.tabs.length === 1 ? DISABLED : ENABLED;
				},
				handler: {
					execute: (context, input) => {
						const tab = namedTab(input);
						if (tab === null) return SCOPE_CHANGED;
						context.markCommitted();
						return apply({
							target: { keepIds: [tab.id], kind: 'others' },
							type: 'close-requested',
						});
					},
				},
				// VS Code parity: workbench.action.closeOtherEditors.
				id: WORKBENCH_ACTION_IDS.closeOtherFileOverlayTabs,
				input: workbenchFileOverlayTabInput,
				title: fileOverlayMessages.closeOtherEditors,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution: tabExecution,
				// VS Code's precondition: a tab to the right.
				getAvailability: (_context, input) => {
					const tab = namedTab(input);
					if (tab === null) return HIDDEN;
					return getState().tabs.tabs.at(-1)?.id === tab.id ? DISABLED : ENABLED;
				},
				handler: {
					execute: (context, input) => {
						const tab = namedTab(input);
						if (tab === null) return SCOPE_CHANGED;
						context.markCommitted();
						return apply({ target: { id: tab.id, kind: 'right' }, type: 'close-requested' });
					},
				},
				// VS Code parity: workbench.action.closeEditorsToTheRight.
				id: WORKBENCH_ACTION_IDS.closeFileOverlayTabsToTheRight,
				input: workbenchFileOverlayTabInput,
				title: fileOverlayMessages.closeEditorsToTheRight,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution: rowExecution,
				getAvailability: rowAvailability,
				handler: {
					execute: (context) => {
						if (getState().tabs.tabs.length === 0) return SCOPE_CHANGED;
						context.markCommitted();
						return apply({ target: { kind: 'saved' }, type: 'close-requested' });
					},
				},
				// VS Code parity: workbench.action.closeUnmodifiedEditors.
				id: WORKBENCH_ACTION_IDS.closeSavedFileOverlayTabs,
				input: noActionInput,
				title: fileOverlayMessages.closeSavedEditors,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution: rowExecution,
				getAvailability: rowAvailability,
				handler: {
					execute: (context) => {
						if (getState().tabs.tabs.length === 0) return SCOPE_CHANGED;
						context.markCommitted();
						// Sticky tabs stay, as VS Code's Close All keeps them.
						return apply({ target: { kind: 'all' }, type: 'close-requested' });
					},
				},
				// VS Code parity: workbench.action.closeEditorsInGroup, and closeAllEditors with its one
				// editor group.
				id: WORKBENCH_ACTION_IDS.closeAllFileOverlayTabs,
				input: noActionInput,
				title: fileOverlayMessages.closeAllEditors,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution: tabExecution,
				// VS Code's Keep Open is greyed for a tab that is not in preview.
				getAvailability: (_context, input) => {
					const tab = namedTab(input);
					if (tab === null) return HIDDEN;
					return tab.preview ? ENABLED : DISABLED;
				},
				handler: {
					execute: (context, input) => {
						const tab = namedTab(input);
						if (tab?.preview !== true) return SCOPE_CHANGED;
						context.markCommitted();
						return apply({ id: tab.id, type: 'tab-kept' });
					},
				},
				// VS Code parity: workbench.action.keepEditor.
				id: WORKBENCH_ACTION_IDS.keepFileOverlayTab,
				input: workbenchFileOverlayTabInput,
				title: fileOverlayMessages.keepEditor,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution: tabExecution,
				getAvailability: (_context, input) => (namedTab(input)?.sticky === true ? ENABLED : HIDDEN),
				handler: {
					execute: (context, input) => {
						const tab = namedTab(input);
						if (tab?.sticky !== true) return SCOPE_CHANGED;
						context.markCommitted();
						return apply({ id: tab.id, type: 'tab-unstuck' });
					},
				},
				// VS Code parity: workbench.action.unpinEditor.
				id: WORKBENCH_ACTION_IDS.unpinFileOverlayTab,
				input: workbenchFileOverlayTabInput,
				title: fileOverlayMessages.unpinEditor,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution: tabExecution,
				// Never offered together with Unpin Editor, so the two share their keys.
				getAvailability: (_context, input) =>
					namedTab(input)?.sticky === false ? ENABLED : HIDDEN,
				handler: {
					execute: (context, input) => {
						const tab = namedTab(input);
						if (tab?.sticky !== false) return SCOPE_CHANGED;
						context.markCommitted();
						return apply({ id: tab.id, type: 'tab-stuck' });
					},
				},
				// VS Code parity: workbench.action.pinEditor.
				id: WORKBENCH_ACTION_IDS.pinFileOverlayTab,
				input: workbenchFileOverlayTabInput,
				title: fileOverlayMessages.pinEditor,
			}),
		),
	];
}
