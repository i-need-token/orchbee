import {
	defineAction,
	noActionInput,
	type ActionAvailabilityContext,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionMessageDescriptor,
	type ActionRuntime,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';
import {
	getTabCloseTargets,
	TAB_STRIP_ACTION_IDS,
	TabStripOrientation,
	type TabCloseInput,
	type TabModel,
} from '@orchbee/tab-strip';

import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import { executePreparedDescendantAction } from './executePreparedDescendantAction';
import { prepareDescendantAction } from './prepareDescendantAction';
import { resolveActiveTabMovement } from './resolveActiveTabMovement';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchTerminalCreateInput,
} from './workbench-action-ids';

interface RegisterWorkbenchActiveTabActionsOptions {
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
	/** Names the place after the active Tab as the strip shows it: to its right, or below it. */
	readonly tabStripOrientation: TabStripOrientation;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

type ActiveTabActionId =
	| typeof WORKBENCH_ACTION_IDS.closeActiveTab
	| typeof WORKBENCH_ACTION_IDS.closeTabsAfterActive
	| typeof WORKBENCH_ACTION_IDS.createTabAfterActive
	| typeof WORKBENCH_ACTION_IDS.duplicateActiveTabs
	| typeof WORKBENCH_ACTION_IDS.moveActiveTabBackward
	| typeof WORKBENCH_ACTION_IDS.moveActiveTabForward;

/**
 * The lower Action, with its input, that a composite runs for the active Tab as it is now, or why
 * the active Tab offers none.
 */
type ActiveTabCommand =
	| { readonly actionId: string; readonly input: unknown }
	| { readonly unavailable: ActionMessageDescriptor };

type ActiveTabSnapshot = ReturnType<TabModel['getSnapshot']>;

/**
 * The commands the TUI's global menu offers for the current Tab. Each one reads the active Tab when it
 * is offered and again when it runs, then runs the Tab Strip or Workbench Action that already owns the
 * change, so availability, confirmation and the authority stay theirs.
 */
export function registerWorkbenchActiveTabActions({
	projectWorkspaceRuntime,
	runtime,
	scope,
	tabStripOrientation,
	typedRuntime,
}: RegisterWorkbenchActiveTabActionsOptions): readonly ActionDisposable[] {
	const vertical = tabStripOrientation === TabStripOrientation.Vertical;
	const definition = (
		id: ActiveTabActionId,
		title: ActionMessageDescriptor,
		command: (snapshot: ActiveTabSnapshot) => ActiveTabCommand,
	) => activeTabActionDefinition({ command, id, projectWorkspaceRuntime, runtime, scope, title });
	return [
		typedRuntime.register(
			scope,
			definition(
				WORKBENCH_ACTION_IDS.moveActiveTabBackward,
				workbenchTabMessages.moveActiveTabBackward,
				(snapshot) => movement(snapshot, 'backward'),
			),
		),
		typedRuntime.register(
			scope,
			definition(
				WORKBENCH_ACTION_IDS.moveActiveTabForward,
				workbenchTabMessages.moveActiveTabForward,
				(snapshot) => movement(snapshot, 'forward'),
			),
		),
		typedRuntime.register(
			scope,
			definition(
				WORKBENCH_ACTION_IDS.createTabAfterActive,
				vertical ? workbenchTabMessages.newTabBelow : workbenchTabMessages.newTabToRight,
				creationAfterActive,
			),
		),
		typedRuntime.register(
			scope,
			definition(
				WORKBENCH_ACTION_IDS.duplicateActiveTabs,
				workbenchTabMessages.duplicateActiveTab,
				duplication,
			),
		),
		typedRuntime.register(
			scope,
			definition(
				WORKBENCH_ACTION_IDS.closeActiveTab,
				workbenchTabMessages.closeActiveTab,
				closureOfActive,
			),
		),
		typedRuntime.register(
			scope,
			definition(
				WORKBENCH_ACTION_IDS.closeTabsAfterActive,
				vertical ? workbenchTabMessages.closeTabsBelow : workbenchTabMessages.closeTabsToRight,
				closureAfterActive,
			),
		),
	];
}

const NO_ACTIVE_TAB = { unavailable: workbenchTabMessages.unavailableNoneSelected } as const;

function movement(
	snapshot: ActiveTabSnapshot,
	direction: 'backward' | 'forward',
): ActiveTabCommand {
	const input = resolveActiveTabMovement({
		activeTabId: snapshot.activeTabId,
		direction,
		tabs: snapshot.tabs.map((tab) => ({ id: tab.id, pinned: tab.pinned })),
	});
	if (input !== null) return { actionId: TAB_STRIP_ACTION_IDS.moveTab, input };
	// A Tab moves only among Tabs pinned as it is, so a lone one has nothing to pass.
	const active = snapshot.tabs.find((tab) => tab.id === snapshot.activeTabId);
	if (active === undefined) return NO_ACTIVE_TAB;
	return {
		unavailable: active.pinned
			? workbenchTabMessages.unavailableOnlyPinned
			: workbenchTabMessages.unavailableOnlyUnpinned,
	};
}

/** Joins the active Tab's group; a place inside the pinned Tabs is left to creation to refuse. */
function creationAfterActive(snapshot: ActiveTabSnapshot): ActiveTabCommand {
	const index = snapshot.tabs.findIndex((tab) => tab.id === snapshot.activeTabId);
	const active = snapshot.tabs[index];
	if (active === undefined) return NO_ACTIVE_TAB;
	const input: WorkbenchTerminalCreateInput = { groupId: active.group, index: index + 1 };
	return { actionId: WORKBENCH_ACTION_IDS.createTerminal, input };
}

/** The selection when the active Tab belongs to it, otherwise the active Tab alone. */
function duplication(snapshot: ActiveTabSnapshot): ActiveTabCommand {
	const tabIds = closeTargets(snapshot)?.selected ?? [];
	return tabIds.length === 0
		? NO_ACTIVE_TAB
		: { actionId: TAB_STRIP_ACTION_IDS.duplicateTabs, input: { tabIds } };
}

/** The TUI's Close Tab…: the active Tab alone, whatever else is selected. */
function closureOfActive(snapshot: ActiveTabSnapshot): ActiveTabCommand {
	if (snapshot.activeTabId === null) return NO_ACTIVE_TAB;
	const input: TabCloseInput = { closeSource: 'non-ui', tabIds: [snapshot.activeTabId] };
	return { actionId: TAB_STRIP_ACTION_IDS.closeTabs, input };
}

/** The Tab menu's own trailing set: pinned and selected Tabs stay, and the selection's last Tab bounds it. */
function closureAfterActive(snapshot: ActiveTabSnapshot): ActiveTabCommand {
	const targets = closeTargets(snapshot);
	if (targets === null) return NO_ACTIVE_TAB;
	const tabIds = targets.trailing;
	if (tabIds.length === 0) return { unavailable: workbenchTabMessages.unavailableNoMatchingTabs };
	const input: TabCloseInput = { closeSource: 'non-ui', tabIds };
	return { actionId: TAB_STRIP_ACTION_IDS.closeTabs, input };
}

function closeTargets(snapshot: ActiveTabSnapshot) {
	return snapshot.activeTabId === null
		? null
		: getTabCloseTargets(snapshot.tabs, snapshot.selectedTabIds, snapshot.activeTabId);
}

function activeTabActionDefinition<Id extends ActiveTabActionId>({
	command,
	id,
	projectWorkspaceRuntime,
	runtime,
	scope,
	title,
}: {
	readonly command: (snapshot: ActiveTabSnapshot) => ActiveTabCommand;
	readonly id: Id;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
	readonly title: ActionMessageDescriptor;
}) {
	const prepare = (context: ActionAvailabilityContext | ActionExecutionContext) => {
		const tabModel = projectWorkspaceRuntime.getActiveTerminalContext()?.tabModel;
		if (tabModel === undefined) {
			return {
				kind: 'unavailable' as const,
				reason: workbenchMessages.workspaceUnavailableRequired,
			};
		}
		const next = command(tabModel.getSnapshot());
		return 'unavailable' in next
			? { kind: 'unavailable' as const, reason: next.unavailable }
			: prepareDescendantAction({ ...next, context, runtime, scope });
	};
	return defineAction({
		category: {
			id: 'tabs',
			title: workbenchTabMessages.actionCategoryTabs,
		},
		execution: {
			concurrency: 'parallel',
			concurrencyKey: () => id,
			mode: 'foreground',
		},
		getAvailability: (context) => {
			const prepared = prepare(context);
			switch (prepared.kind) {
				case 'ready':
					return { kind: 'enabled' };
				case 'unavailable':
					return { kind: 'disabled', reason: prepared.reason };
				case 'hidden':
					return prepared;
			}
		},
		handler: {
			execute: async (context) => {
				const prepared = prepare(context);
				return prepared.kind === 'ready'
					? await executePreparedDescendantAction(context, runtime, prepared.invocation)
					: { code: 'active-tab-action-unavailable', kind: 'blocked' };
			},
		},
		id,
		input: noActionInput,
		title,
	});
}
