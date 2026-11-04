import {
	defineAction,
	noActionInput,
	type ActionAvailabilityContext,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionInvocation,
	type ActionMessageDescriptor,
	type ActionRuntime,
	type ActionScope,
	type PreparedActionInvocation,
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
import { resolveActiveTabMovement } from './resolveActiveTabMovement';
import { WORKBENCH_DESCENDANT_ACTION_DISCOVERY_INPUTS } from './workbench-descendant-action-discovery';
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
	| typeof WORKBENCH_ACTION_IDS.closeTabsAfterActive
	| typeof WORKBENCH_ACTION_IDS.createTabAfterActive
	| typeof WORKBENCH_ACTION_IDS.createTabInRecentGroup
	| typeof WORKBENCH_ACTION_IDS.duplicateActiveTabs
	| typeof WORKBENCH_ACTION_IDS.moveActiveTabBackward
	| typeof WORKBENCH_ACTION_IDS.moveActiveTabForward;

/** The lower Action, with its input, that a composite runs for the active Tab as it is now. */
interface ActiveTabCommand {
	readonly actionId: string;
	readonly input: unknown;
}

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
		command: (snapshot: ActiveTabSnapshot) => ActiveTabCommand | null,
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
				WORKBENCH_ACTION_IDS.createTabInRecentGroup,
				workbenchTabMessages.newTabInRecentGroup,
				creationInRecentGroup,
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
				WORKBENCH_ACTION_IDS.closeTabsAfterActive,
				vertical ? workbenchTabMessages.closeTabsBelow : workbenchTabMessages.closeTabsToRight,
				closureAfterActive,
			),
		),
	];
}

function movement(
	snapshot: ActiveTabSnapshot,
	direction: 'backward' | 'forward',
): ActiveTabCommand | null {
	const input = resolveActiveTabMovement({
		activeTabId: snapshot.activeTabId,
		direction,
		tabs: snapshot.tabs.map((tab) => ({ id: tab.id, pinned: tab.pinned })),
	});
	return input === null ? null : { actionId: TAB_STRIP_ACTION_IDS.moveTab, input };
}

/** Joins the active Tab's group; a place inside the pinned Tabs is left to creation to refuse. */
function creationAfterActive(snapshot: ActiveTabSnapshot): ActiveTabCommand | null {
	const index = snapshot.tabs.findIndex((tab) => tab.id === snapshot.activeTabId);
	const active = snapshot.tabs[index];
	if (active === undefined) return null;
	const input: WorkbenchTerminalCreateInput = { groupId: active.group, index: index + 1 };
	return { actionId: WORKBENCH_ACTION_IDS.createTerminal, input };
}

/** The TUI's recent group: the active Tab's, else the one last used while it remains, else the last. */
function creationInRecentGroup(snapshot: ActiveTabSnapshot): ActiveTabCommand | null {
	const remembered = snapshot.groups.find((group) => group.id === snapshot.mostRecentGroupId);
	const groupId =
		snapshot.tabs.find((tab) => tab.id === snapshot.activeTabId)?.group ??
		remembered?.id ??
		snapshot.groups.at(-1)?.id;
	if (groupId === undefined) return null;
	const input: WorkbenchTerminalCreateInput = { groupId };
	return { actionId: WORKBENCH_ACTION_IDS.createTerminal, input };
}

/** The selection when the active Tab belongs to it, otherwise the active Tab alone. */
function duplication(snapshot: ActiveTabSnapshot): ActiveTabCommand | null {
	const tabIds = closeTargets(snapshot)?.selected ?? [];
	return tabIds.length === 0
		? null
		: { actionId: TAB_STRIP_ACTION_IDS.duplicateTabs, input: { tabIds } };
}

/** The Tab menu's own trailing set: pinned and selected Tabs stay, and the selection's last Tab bounds it. */
function closureAfterActive(snapshot: ActiveTabSnapshot): ActiveTabCommand | null {
	const tabIds = closeTargets(snapshot)?.trailing ?? [];
	if (tabIds.length === 0) return null;
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
	readonly command: (snapshot: ActiveTabSnapshot) => ActiveTabCommand | null;
	readonly id: Id;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
	readonly title: ActionMessageDescriptor;
}) {
	const prepare = (context: ActionAvailabilityContext | ActionExecutionContext) => {
		const next = command(projectWorkspaceRuntime.getActiveTerminalContext().tabModel.getSnapshot());
		return next === null ? null : prepareLowerAction(context, runtime, scope, next);
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
		getAvailability: (context) =>
			prepare(context) === null
				? { kind: 'disabled', reason: workbenchMessages.actionUnavailable }
				: { kind: 'enabled' },
		handler: {
			execute: async (context) => {
				const prepared = prepare(context);
				return prepared === null
					? { code: 'active-tab-action-unavailable', kind: 'blocked' }
					: await executePreparedDescendantAction(context, runtime, prepared);
			},
		},
		id,
		input: noActionInput,
		title,
	});
}

function prepareLowerAction(
	context: ActionAvailabilityContext | ActionExecutionContext,
	runtime: ActionRuntime,
	scope: ActionScope,
	{ actionId, input }: ActiveTabCommand,
): PreparedActionInvocation | null {
	const prepared = runtime
		.discover(discoveryInvocation(context, scope), {
			includeDescendants: true,
			inputByActionId: {
				...WORKBENCH_DESCENDANT_ACTION_DISCOVERY_INPUTS,
				[actionId]: input,
			},
		})
		.find(
			(result) =>
				result.kind === 'ready' &&
				result.descriptor.id === actionId &&
				result.availability.kind === 'enabled',
		);
	return prepared?.kind === 'ready' ? prepared.invocation : null;
}

function discoveryInvocation(
	context: ActionAvailabilityContext | ActionExecutionContext,
	scope: ActionScope,
): ActionInvocation {
	return {
		...(context.capabilities === undefined ? {} : { capabilities: context.capabilities }),
		...(context.focus === undefined ? {} : { focus: context.focus }),
		scope,
		source: context.source,
		...(context.target === undefined ? {} : { target: context.target }),
	};
}
