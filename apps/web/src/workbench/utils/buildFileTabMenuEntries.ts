import type {
	ActionAvailability,
	ActionExecution,
	ActionScope,
	TypedActionRuntime,
} from '@orchbee/actions';
import { fileOverlayMessages, type MessageDescriptor } from '@orchbee/i18n';

import type { FileTabMenuEntry } from '../../file-overlay';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../actions/workbench-action-ids';

/** The menu's Actions that act on the tab the menu opened on. */
type FileTabMenuTabActionId =
	| typeof WORKBENCH_ACTION_IDS.closeFileOverlayTab
	| typeof WORKBENCH_ACTION_IDS.closeFileOverlayTabsToTheRight
	| typeof WORKBENCH_ACTION_IDS.closeOtherFileOverlayTabs
	| typeof WORKBENCH_ACTION_IDS.keepFileOverlayTab
	| typeof WORKBENCH_ACTION_IDS.pinFileOverlayTab
	| typeof WORKBENCH_ACTION_IDS.unpinFileOverlayTab;

/** The menu's Actions that act on the whole row. */
type FileTabMenuRowActionId =
	| typeof WORKBENCH_ACTION_IDS.closeAllFileOverlayTabs
	| typeof WORKBENCH_ACTION_IDS.closeSavedFileOverlayTabs;

type FileTabMenuItem =
	| {
			readonly actionId: FileTabMenuTabActionId;
			readonly label: MessageDescriptor;
			readonly target: 'tab';
	  }
	| {
			readonly actionId: FileTabMenuRowActionId;
			readonly label: MessageDescriptor;
			readonly target: 'row';
	  };

interface FileTabMenuGroup {
	/** VS Code's name for the group, which orders it. */
	readonly id: string;
	readonly items: readonly FileTabMenuItem[];
}

/**
 * VS Code's tab menu as Orchbee offers it, `MenuId.EditorTitleContext`, group by group in VS Code's
 * order and labelled with VS Code's menu titles. The parity test holds it to VS Code's menu and
 * lists the items left out.
 */
const FILE_TAB_MENU: readonly FileTabMenuGroup[] = [
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

interface BuildFileTabMenuEntriesOptions {
	readonly formatMessage: (descriptor: MessageDescriptor) => string;
	/** Runs an item's Action and says a refusal or failure that no surface showed. */
	readonly report: (start: () => ActionExecution<unknown>) => void;
	/** The Workbench's own scope, where the overlay's Actions resolve. */
	readonly scope: ActionScope;
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	/** The tab the menu opened on. */
	readonly tabId: string;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
	/** The Workspace whose overlay shows the tab. */
	readonly workspaceId: string;
}

/** An item's Action as the menu offers it now. */
interface FileTabMenuOffer {
	readonly availability: ActionAvailability;
	readonly run: () => void;
}

/**
 * One tab's menu as it opens: each item whose Action is offered for that tab, greyed where the
 * Action is disabled, as VS Code shows an item whose precondition fails, with a line between
 * groups. The menu does not show its tab, so its items act on that tab without moving to it.
 */
export function buildFileTabMenuEntries({
	formatMessage,
	report,
	scope,
	shortcutLabelForAction,
	tabId,
	typedRuntime,
	workspaceId,
}: BuildFileTabMenuEntriesOptions): readonly FileTabMenuEntry[] {
	const invocation = { scope, source: 'context-menu' } as const;
	const tabInput = { tabId, workspaceId };
	const offerOf = (item: FileTabMenuItem): FileTabMenuOffer | null => {
		if (item.target === 'tab') {
			const resolved = typedRuntime.resolveAction(item.actionId, tabInput, invocation);
			return resolved === null
				? null
				: {
						availability: resolved.availability,
						run: () => report(() => typedRuntime.execute(item.actionId, tabInput, invocation)),
					};
		}
		const resolved = typedRuntime.resolveAction(item.actionId, undefined, invocation);
		return resolved === null
			? null
			: {
					availability: resolved.availability,
					run: () => report(() => typedRuntime.execute(item.actionId, undefined, invocation)),
				};
	};
	const entries: FileTabMenuEntry[] = [];
	for (const group of FILE_TAB_MENU) {
		const commands = group.items.flatMap((item): FileTabMenuEntry[] => {
			const offer = offerOf(item);
			if (offer === null || offer.availability.kind === 'hidden') return [];
			return [
				{
					disabled: offer.availability.kind === 'disabled',
					id: item.actionId,
					kind: 'command',
					label: formatMessage(item.label),
					run: offer.run,
					shortcut: shortcutLabelForAction(item.actionId),
				},
			];
		});
		if (commands.length === 0) continue;
		if (entries.length > 0) entries.push({ id: group.id, kind: 'separator' });
		entries.push(...commands);
	}
	return entries;
}
