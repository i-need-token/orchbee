import type {
	ActionExecution,
	ActionInvocationSource,
	ActionScope,
	TypedActionRuntime,
} from '@orchbee/actions';
import { fileOverlayMessages, type MessageDescriptor } from '@orchbee/i18n';
import type { ProductIconName } from '@orchbee/ui/components';

import type {
	FileOverlayMenuCommand,
	FileOverlayMenuEntry,
	FileOverlayTitleActions,
	FileOverlayTitleButton,
} from '../../file-overlay';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../actions/workbench-action-ids';

/** The menus' Actions that act on the tab a menu is for. */
type FileOverlayTabMenuActionId =
	| typeof WORKBENCH_ACTION_IDS.closeFileOverlayTab
	| typeof WORKBENCH_ACTION_IDS.closeFileOverlayTabsToTheRight
	| typeof WORKBENCH_ACTION_IDS.closeOtherFileOverlayTabs
	| typeof WORKBENCH_ACTION_IDS.keepFileOverlayTab
	| typeof WORKBENCH_ACTION_IDS.pinFileOverlayTab
	| typeof WORKBENCH_ACTION_IDS.unpinFileOverlayTab;

/** The menus' Actions that act on the whole row. */
type FileOverlayRowMenuActionId =
	| typeof WORKBENCH_ACTION_IDS.closeAllFileOverlayTabs
	| typeof WORKBENCH_ACTION_IDS.closeSavedFileOverlayTabs
	| typeof WORKBENCH_ACTION_IDS.showRecentFileOverlayTabs;

type FileOverlayMenuItem =
	| {
			readonly actionId: FileOverlayTabMenuActionId;
			readonly label: MessageDescriptor;
			readonly target: 'tab';
	  }
	| {
			readonly actionId: FileOverlayRowMenuActionId;
			readonly label: MessageDescriptor;
			readonly target: 'row';
	  };

interface FileOverlayMenuGroup {
	/** VS Code's name for the group, which orders it. */
	readonly id: string;
	readonly items: readonly FileOverlayMenuItem[];
}

/** A title action shown as a button, with the icon VS Code gives it. */
type FileOverlayTitleButtonItem = FileOverlayMenuItem & { readonly icon: ProductIconName };

/**
 * VS Code's tab menu as Orchbee offers it, `MenuId.EditorTitleContext`, group by group in VS Code's
 * order and labelled with VS Code's menu titles.
 */
const FILE_TAB_MENU: readonly FileOverlayMenuGroup[] = [
	{
		id: '1_close',
		items: [
			{
				actionId: WORKBENCH_ACTION_IDS.closeFileOverlayTab,
				label: fileOverlayMessages.tabMenuClose,
				target: 'tab',
			},
			{
				actionId: WORKBENCH_ACTION_IDS.closeOtherFileOverlayTabs,
				label: fileOverlayMessages.tabMenuCloseOthers,
				target: 'tab',
			},
			{
				actionId: WORKBENCH_ACTION_IDS.closeFileOverlayTabsToTheRight,
				label: fileOverlayMessages.tabMenuCloseRight,
				target: 'tab',
			},
			{
				actionId: WORKBENCH_ACTION_IDS.closeSavedFileOverlayTabs,
				label: fileOverlayMessages.tabMenuCloseSaved,
				target: 'row',
			},
			{
				actionId: WORKBENCH_ACTION_IDS.closeAllFileOverlayTabs,
				label: fileOverlayMessages.tabMenuCloseAll,
				target: 'row',
			},
		],
	},
	{
		id: '3_preview',
		items: [
			{
				actionId: WORKBENCH_ACTION_IDS.keepFileOverlayTab,
				label: fileOverlayMessages.tabMenuKeepOpen,
				target: 'tab',
			},
			{
				actionId: WORKBENCH_ACTION_IDS.pinFileOverlayTab,
				label: fileOverlayMessages.tabMenuPin,
				target: 'tab',
			},
			{
				actionId: WORKBENCH_ACTION_IDS.unpinFileOverlayTab,
				label: fileOverlayMessages.tabMenuUnpin,
				target: 'tab',
			},
		],
	},
];

/**
 * VS Code's title actions as Orchbee offers them, `MenuId.EditorTitle`: its `navigation` group by
 * VS Code's order as buttons, and its other groups in VS Code's order as More Actions. A slice that
 * adds a kind of tab adds the items VS Code shows for that kind here, each Action hidden for the
 * tabs it does not act on.
 */
const FILE_OVERLAY_TITLE_MENU: {
	readonly buttons: readonly FileOverlayTitleButtonItem[];
	readonly more: readonly FileOverlayMenuGroup[];
} = {
	buttons: [],
	more: [
		{
			id: '3_open',
			items: [
				{
					actionId: WORKBENCH_ACTION_IDS.showRecentFileOverlayTabs,
					label: fileOverlayMessages.tabMenuShowOpenedEditors,
					target: 'row',
				},
			],
		},
		{
			id: '5_close',
			items: [
				{
					actionId: WORKBENCH_ACTION_IDS.closeAllFileOverlayTabs,
					label: fileOverlayMessages.tabMenuCloseAll,
					target: 'row',
				},
				{
					actionId: WORKBENCH_ACTION_IDS.closeSavedFileOverlayTabs,
					label: fileOverlayMessages.tabMenuCloseSaved,
					target: 'row',
				},
			],
		},
	],
};

interface FileOverlayMenuOptions {
	readonly formatMessage: (descriptor: MessageDescriptor) => string;
	/** Runs an item's Action and says a refusal or failure that no surface showed. */
	readonly report: (source: ActionInvocationSource, start: () => ActionExecution<unknown>) => void;
	/** The Workbench's own scope, where the overlay's Actions resolve. */
	readonly scope: ActionScope;
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	/** The tab the menu is for: the tab right-clicked, or the active tab for the title actions. */
	readonly tabId: string;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
	/** The Workspace whose overlay shows the tab. */
	readonly workspaceId: string;
}

/**
 * An item as its menu offers it now, greyed where its Action is disabled, as VS Code shows an item
 * whose precondition fails; null while its Action is hidden or not registered.
 */
function commandOf(
	item: FileOverlayMenuItem,
	source: ActionInvocationSource,
	{
		formatMessage,
		report,
		scope,
		shortcutLabelForAction,
		tabId,
		typedRuntime,
		workspaceId,
	}: FileOverlayMenuOptions,
): FileOverlayMenuCommand | null {
	const invocation = { scope, source };
	let offer: { readonly disabled: boolean; readonly run: () => void } | null = null;
	if (item.target === 'tab') {
		const input = { tabId, workspaceId };
		const resolved = typedRuntime.resolveAction(item.actionId, input, invocation);
		if (resolved !== null && resolved.availability.kind !== 'hidden') {
			offer = {
				disabled: resolved.availability.kind === 'disabled',
				run: () => report(source, () => typedRuntime.execute(item.actionId, input, invocation)),
			};
		}
	} else {
		const resolved = typedRuntime.resolveAction(item.actionId, undefined, invocation);
		if (resolved !== null && resolved.availability.kind !== 'hidden') {
			offer = {
				disabled: resolved.availability.kind === 'disabled',
				run: () => report(source, () => typedRuntime.execute(item.actionId, undefined, invocation)),
			};
		}
	}
	return offer === null
		? null
		: {
				disabled: offer.disabled,
				id: item.actionId,
				kind: 'command',
				label: formatMessage(item.label),
				run: offer.run,
				shortcut: shortcutLabelForAction(item.actionId),
			};
}

/** A menu's offered items, group by group, with a line between groups. */
function menuEntries(
	groups: readonly FileOverlayMenuGroup[],
	source: ActionInvocationSource,
	options: FileOverlayMenuOptions,
): readonly FileOverlayMenuEntry[] {
	const entries: FileOverlayMenuEntry[] = [];
	for (const group of groups) {
		const commands = group.items.flatMap((item) => {
			const command = commandOf(item, source, options);
			return command === null ? [] : [command];
		});
		if (commands.length === 0) continue;
		if (entries.length > 0) entries.push({ id: group.id, kind: 'separator' });
		entries.push(...commands);
	}
	return entries;
}

/**
 * One tab's menu as it opens: each item VS Code's tab menu shows for that tab. The menu does not show
 * its tab, so its items act on that tab without moving to it.
 */
export function buildFileTabMenuEntries(
	options: FileOverlayMenuOptions,
): readonly FileOverlayMenuEntry[] {
	return menuEntries(FILE_TAB_MENU, 'context-menu', options);
}

/** The title actions VS Code shows for the active tab. */
export function buildFileOverlayTitleActions(
	options: FileOverlayMenuOptions,
): FileOverlayTitleActions {
	return {
		buttons: FILE_OVERLAY_TITLE_MENU.buttons.flatMap((item): FileOverlayTitleButton[] => {
			const command = commandOf(item, 'button', options);
			return command === null ? [] : [{ ...command, icon: item.icon }];
		}),
		more: menuEntries(FILE_OVERLAY_TITLE_MENU.more, 'menu', options),
	};
}
