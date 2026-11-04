import type { TabStripArrangementChange } from '@orchbee/tab-strip';
import type { WorkbenchAuthorityChange, WorkbenchAuthorityTabCreation } from '../../workbench';
import {
	WorkbenchLaunchSelectionSchema,
	type UserLaunchProfile,
	type UserLaunchProfileCatalog,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';
import type { z } from 'zod';

import { readServerWorkbenchAction } from './readServerWorkbenchAction';
import type { ServerWorkbenchAction } from './ServerWorkbenchSession';
import { DEFAULT_TERMINAL_GEOMETRY } from './workspace-launch';

type ServerLaunchSelection = z.output<typeof WorkbenchLaunchSelectionSchema>;

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
				color: change.color,
				kind: 'create-group-for-tabs',
				label: change.title,
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

function sameLaunchProfile(left: UserLaunchProfile, right: UserLaunchProfile): boolean {
	if (left.id !== right.id || left.name !== right.name) return false;
	const [leftCommand, rightCommand] = [left.command, right.command];
	if (leftCommand.kind === 'system-shell') {
		return rightCommand.kind === 'system-shell' && leftCommand.login === rightCommand.login;
	}
	return (
		rightCommand.kind === 'custom-executable' &&
		leftCommand.executable === rightCommand.executable &&
		leftCommand.arguments.length === rightCommand.arguments.length &&
		leftCommand.arguments.every((argument, index) => argument === rightCommand.arguments[index])
	);
}

/**
 * How a Tab starts with a saved launch profile from this window: a custom program as its literal
 * executable and arguments. The window has no system-shell resolver of its own, so a system shell
 * starts only as the default, which the Worker resolves where it runs; a default changed in the same
 * moment is the one that starts. Null starts nothing in place of a profile that is no longer saved as
 * shown, or that no command can carry.
 */
export function serverProfileLaunch(
	profile: UserLaunchProfile,
	catalog: Pick<UserLaunchProfileCatalog, 'defaultProfileId' | 'profiles'> | undefined,
): { readonly launch: ServerLaunchSelection | undefined } | null {
	const saved = catalog?.profiles.find((candidate) => candidate.id === profile.id);
	if (catalog === undefined || saved === undefined || !sameLaunchProfile(saved, profile)) {
		return null;
	}
	const { command } = profile;
	if (command.kind === 'system-shell') {
		return catalog.defaultProfileId === profile.id ? { launch: undefined } : null;
	}
	const launch = WorkbenchLaunchSelectionSchema.safeParse({
		args: command.arguments,
		exec: command.executable,
	});
	return launch.success ? { launch: launch.data } : null;
}

/**
 * A new Tab is named by its place until something better shows, as every Client names it. A copy takes
 * its source's name and whether that name follows the terminal.
 */
export function tabCreateAction(
	{ cwd, placement, terminalContextId }: WorkbenchAuthorityTabCreation,
	snapshot: WorkbenchSnapshot,
	launch?: ServerLaunchSelection,
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
		...(launch === undefined ? {} : { launch }),
		...(cwd === undefined ? {} : { options: { cwd } }),
		...(placement === null ? {} : { placement }),
		terminalContextId,
	});
}
