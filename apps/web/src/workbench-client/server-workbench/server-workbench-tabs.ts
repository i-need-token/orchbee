import type { TabStripArrangementChange } from '@orchbee/tab-strip';
import type { WorkbenchAuthorityChange, WorkbenchAuthorityTabCreation } from '@orchbee/workbench';
import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

import { readServerWorkbenchAction } from './readServerWorkbenchAction';
import type { ServerWorkbenchAction } from './ServerWorkbenchSession';
import { DEFAULT_TERMINAL_GEOMETRY } from './workspace-launch';

type ServerTerminalContext = WorkbenchSnapshot['terminalContexts'][number];

type ServerTabsOperation = Extract<
	Parameters<typeof readServerWorkbenchAction>[0],
	{ readonly kind: 'tabs.configure' }
>['operation'];

function holdsOrder(context: ServerTerminalContext, order: readonly string[]): boolean {
	return (
		context.tabs.length === order.length &&
		context.tabs.every((tab, index) => tab.tabId === order[index])
	);
}

/** Null when the change no longer means what the person asked for. */
function tabsOperation(
	change: TabStripArrangementChange,
	context: ServerTerminalContext,
): ServerTabsOperation | null {
	switch (change.kind) {
		case 'group-moved':
			return {
				direction: change.direction === -1 ? 'previous' : 'next',
				groupId: change.groupId,
				kind: 'move-group',
			};
		case 'group-ungrouped':
			return { groupId: change.groupId, kind: 'ungroup' };
		// The command states the whole group, so what did not change comes from what it will replace.
		case 'group-updated': {
			const group = context.groups.find((candidate) => candidate.groupId === change.groupId);
			return group === undefined
				? null
				: {
						color: change.color ?? group.color,
						groupId: change.groupId,
						kind: 'update-group',
						label: change.title ?? group.label,
					};
		}
		case 'tab-moved':
			return {
				adjacentGroupCollapsed: change.adjacentGroupCollapsed,
				direction: change.direction,
				kind: 'move-tab',
				tabId: change.tabId,
			};
		// A drag's boundary is a place in the order it started from, and says nothing about any other.
		case 'tabs-dragged':
			return holdsOrder(context, change.sourceOrder)
				? {
						groupId: change.groupId,
						insertIndex: change.insertIndex,
						kind: 'move-tabs-by-drag',
						tabIds: [...change.tabIds],
					}
				: null;
		case 'tabs-group-set':
			return { groupId: change.groupId, kind: 'set-tabs-group', tabIds: [...change.tabIds] };
		case 'tabs-grouped':
			return {
				color: 'grey',
				kind: 'create-group-for-tabs',
				label: '',
				tabIds: [...change.tabIds],
			};
		case 'tabs-pinned-set':
			return { kind: 'set-tabs-pinned', pinned: change.pinned, tabIds: [...change.tabIds] };
	}
}

function terminalContextOf(
	snapshot: WorkbenchSnapshot,
	terminalContextId: string,
): ServerTerminalContext | undefined {
	return snapshot.terminalContexts.find(
		(context) => context.terminalContextId === terminalContextId,
	);
}

export function tabsChangeAction(
	{ change, terminalContextId }: Extract<WorkbenchAuthorityChange, { type: 'tabs-changed' }>,
	snapshot: WorkbenchSnapshot,
): ServerWorkbenchAction | null {
	const context = terminalContextOf(snapshot, terminalContextId);
	const operation = context === undefined ? null : tabsOperation(change, context);
	return operation === null
		? null
		: readServerWorkbenchAction({ kind: 'tabs.configure', operation, terminalContextId });
}

/**
 * A new Tab is named by its place until something better shows, as every Client names it. A copy takes
 * its source's name and whether that name follows the terminal.
 */
export function tabCreateAction(
	{ cwd, placement, terminalContextId }: WorkbenchAuthorityTabCreation,
	snapshot: WorkbenchSnapshot,
): ServerWorkbenchAction | null {
	const context = terminalContextOf(snapshot, terminalContextId);
	if (context === undefined) return null;
	const source =
		placement?.kind === 'duplicate'
			? context.tabs.find((tab) => tab.tabId === placement.tabId)
			: undefined;
	if (placement?.kind === 'duplicate' && source === undefined) return null;
	return readServerWorkbenchAction({
		automaticLabel: source === undefined ? true : source.automaticLabel === true,
		geometry: DEFAULT_TERMINAL_GEOMETRY,
		kind: 'tab.create',
		label: source?.label ?? `${context.tabs.length + 1}`,
		...(cwd === undefined ? {} : { options: { cwd } }),
		...(placement === null ? {} : { placement }),
		terminalContextId,
	});
}
