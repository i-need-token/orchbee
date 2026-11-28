import {
	defineAction,
	noActionInput,
	type ActionAvailability,
	type ActionDisposable,
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
	type WorkbenchActionCatalog,
	type WorkbenchFileOverlayTabInput,
} from './workbench-action-ids';

interface RegisterWorkbenchFileOverlayActionsOptions {
	/** Applies an event to the shown Workspace's overlay and carries out what it asks of the Workbench. */
	readonly dispatch: (event: FileOverlayEvent) => void;
	/** The shown Workspace's overlay now, read whenever an Action asks. */
	readonly getState: () => FileOverlayState;
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

const VIEW_CATEGORY = {
	id: 'view',
	title: workbenchMessages.actionCategoryView,
};

const ENABLED: ActionAvailability = { kind: 'enabled' };
const HIDDEN: ActionAvailability = { kind: 'hidden' };
/** The tab is gone, or the overlay that showed it is no longer shown. */
const SCOPE_CHANGED: ActionResult<void> = { code: 'scope-changed', kind: 'blocked' };

/**
 * The file overlay's Actions. They exist only in a window that offers the File Bar, for the shown
 * Workspace's overlay. A tab Action names its tab with the Workspace that shows it, or without input
 * takes the active tab, as VS Code's editor commands take the active editor.
 */
export function registerWorkbenchFileOverlayActions({
	dispatch,
	getState,
	scope,
	typedRuntime,
	workspaceId,
}: RegisterWorkbenchFileOverlayActionsOptions): readonly ActionDisposable[] {
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
	return [
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
				execution: {
					concurrency: 'parallel',
					concurrencyKey: tabConcurrencyKey,
					mode: 'foreground',
				},
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
				execution: {
					concurrency: 'parallel',
					concurrencyKey: tabConcurrencyKey,
					mode: 'foreground',
				},
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
				execution: {
					concurrency: 'parallel',
					concurrencyKey: tabConcurrencyKey,
					mode: 'foreground',
				},
				getAvailability: (_context, input) =>
					namedTab(input)?.preview === true ? ENABLED : HIDDEN,
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
				execution: {
					concurrency: 'parallel',
					concurrencyKey: tabConcurrencyKey,
					mode: 'foreground',
				},
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
	];
}
