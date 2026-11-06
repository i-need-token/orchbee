import { workbenchTabMessages, type IntlShape } from '@orchbee/i18n';
import type {
	WorkbenchPublication,
	WorkbenchPublishedTab,
	WorkbenchPublishedTerminalContext,
	WorkbenchTerminalPresentation,
} from '../../workbench';
import type {
	TerminalPresentationLifecycle,
	WorkbenchPanelNode,
	WorkbenchPresentation,
	WorkbenchSnapshot,
	WorkbenchTerminalContext,
} from '@orchbee/workbench-client-protocol';

import type { ServerLeavingView } from './ServerLeaving';
import { observedServerTerminalEnd, type ObservedServerTerminalEnds } from './server-terminal-ends';
import { nameServerTerminalProgram } from './server-terminal-names';

type Navigation = WorkbenchSnapshot['navigation'];
type Directory = Navigation['directories'][number];
type Workspace = Navigation['workspaces'][number];
type Tab = WorkbenchTerminalContext['tabs'][number];
type PanelNode = WorkbenchPublishedTab['root'];

/** A Tab the window shows, with its Server Panel tree after the Panels a close leaves out. */
interface ShownTab {
	readonly position: number;
	readonly root: WorkbenchPanelNode;
	readonly tab: Tab;
}

const NO_OBSERVED_ENDS: ObservedServerTerminalEnds = new Map();
const NONE_LEAVING: ServerLeavingView = {
	panel: () => false,
	tab: () => false,
	workspace: () => false,
};

/** What projecting one Tab's tree reads besides the tree. */
interface PanelProjection {
	readonly intl: IntlShape;
	readonly observedEnds: ObservedServerTerminalEnds;
	/** How a terminal of the Tab is named when it shows no title. */
	readonly tabTitle: string;
}

const EVEN_SPLIT_RATIO = 0.5;

/**
 * Projects what the Server published into the shapes the React Workbench's stores hold. Names
 * follow the TUI's: a terminal that shows no title takes the program the Server launched for it,
 * else its Tab's place, both in `intl`'s language. `observedEnds` holds, by Presentation, what this
 * window saw end. What `leaving` names is left out: a Workspace with its terminal context; a Tab, as
 * the TUI leaves out a Tab whose close is progressing; and a Panel, as the Server removes it, its
 * sibling taking the split's place. A Tab left with no Panel leaves too, and so does a group it
 * leaves empty.
 */
export function projectServerSnapshot(
	snapshot: WorkbenchSnapshot,
	intl: IntlShape,
	observedEnds = NO_OBSERVED_ENDS,
	leaving = NONE_LEAVING,
): WorkbenchPublication {
	const { navigation } = snapshot;
	const isLeaving = (workspace: Workspace): boolean => leaving.workspace(workspace.workspaceId);
	const leavingContextIds = new Set(
		navigation.workspaces.filter(isLeaving).map((workspace) => workspace.terminalContextId),
	);
	const directories = new Map(
		navigation.directories.map((directory) => [directory.directoryId, directory]),
	);
	return {
		projects: inDisplayOrder(
			navigation.projects,
			navigation.projectOrder,
			(project) => project.projectId,
		).map((project) => ({ id: project.projectId, name: project.label })),
		terminalContexts: snapshot.terminalContexts
			.filter((context) => !leavingContextIds.has(context.terminalContextId))
			.map((context) => projectTerminalContext(context, intl, observedEnds, leaving)),
		workspaces: inDisplayOrder(
			navigation.workspaces.filter((workspace) => !isLeaving(workspace)),
			navigation.workspaceOrder,
			(workspace) => workspace.workspaceId,
		).map((workspace) => ({
			directory: projectWorkspaceDirectory(workspace, directories),
			id: workspace.workspaceId,
			name: workspace.label,
			projectId: workspace.projectId,
			terminalContextId: workspace.terminalContextId,
		})),
	};
}

/** An order is either absent or a complete permutation; identity order is the display order then. */
function inDisplayOrder<Item>(
	items: readonly Item[],
	order: readonly string[] | undefined,
	identityOf: (item: Item) => string,
): readonly Item[] {
	if (order === undefined || order.length === 0) return items;
	const positions = new Map(order.map((identity, position) => [identity, position]));
	const positionOf = (item: Item): number => positions.get(identityOf(item)) ?? order.length;
	return [...items].sort((left, right) => positionOf(left) - positionOf(right));
}

function projectWorkspaceDirectory(
	workspace: Workspace,
	directories: ReadonlyMap<string, Directory>,
): WorkbenchPublication['workspaces'][number]['directory'] {
	if (workspace.primaryDirectoryId === null) return null;
	const directory = directories.get(workspace.primaryDirectoryId);
	if (directory === undefined) {
		throw new Error(`Workspace ${workspace.workspaceId} names a missing directory.`);
	}
	return { id: directory.directoryId, path: directory.path };
}

function projectTerminalContext(
	context: WorkbenchTerminalContext,
	intl: IntlShape,
	observedEnds: ObservedServerTerminalEnds,
	leaving: ServerLeavingView,
): WorkbenchPublishedTerminalContext {
	const { terminalContextId } = context;
	// A Tab keeps its place among all the Server holds, so no other Tab is renumbered while it leaves.
	const shownTabs = context.tabs.flatMap((tab, position): ShownTab[] => {
		if (leaving.tab(terminalContextId, tab.tabId)) return [];
		const root = withholdPanels(tab.root, (panelId) =>
			leaving.panel(terminalContextId, tab.tabId, panelId),
		);
		return root === null ? [] : [{ position, root, tab }];
	});
	const shownTabIds = new Set(shownTabs.map(({ tab }) => tab.tabId));
	const withheldTabIds = context.tabs
		.filter((tab) => !shownTabIds.has(tab.tabId))
		.map((tab) => tab.tabId);
	const shownGroups = new Set(shownTabs.map(({ tab }) => tab.groupId));
	const heldGroups = new Set(context.tabs.map((tab) => tab.groupId));
	return {
		// A projected context holds names in one language, the ends this window saw, and the Tabs and
		// Panels it shows, so another language, another end, or a leaving member replaces it.
		fingerprint: JSON.stringify([
			intl.locale,
			context,
			context.tabs.flatMap((tab) =>
				presentationsOf(tab.root).map((presentation) =>
					observedServerTerminalEnd(presentation, observedEnds),
				),
			),
			shownTabs.map(({ root, tab }) => [tab.tabId, panelIdsOf(root)]),
		]),
		groups: context.groups
			.filter((group) => shownGroups.has(group.groupId) || !heldGroups.has(group.groupId))
			.map((group) => ({
				color: group.color,
				id: group.groupId,
				title: group.label,
			})),
		tabs: shownTabs.map(({ position, root, tab }) => {
			const tabTitle = tabPositionTitle(position, intl);
			return {
				customTitle: customTabTitle(tab),
				group: tab.groupId,
				id: tab.tabId,
				pinned: tab.pinned,
				root: projectPanelNode(root, { intl, observedEnds, tabTitle }),
				title: tab.automaticLabel === true ? tabTitle : tab.label,
			};
		}),
		terminalContextId: context.terminalContextId,
		withheldTabIds,
	};
}

/**
 * The Server's Panel tree without the Panels a close leaves out, each removed as the Server removes
 * it: its parent Split gives way to the sibling subtree. Null when every Panel of the tree is left out.
 */
function withholdPanels(
	node: WorkbenchPanelNode,
	isWithheld: (panelId: string) => boolean,
): WorkbenchPanelNode | null {
	if (node.kind === 'panel') return isWithheld(node.panelId) ? null : node;
	const [first, second] = node.children;
	const shownFirst = withholdPanels(first.node, isWithheld);
	const shownSecond = withholdPanels(second.node, isWithheld);
	if (shownFirst === null) return shownSecond;
	if (shownSecond === null) return shownFirst;
	if (shownFirst === first.node && shownSecond === second.node) return node;
	return {
		...node,
		children: [
			{ ...first, node: shownFirst },
			{ ...second, node: shownSecond },
		],
	};
}

/** The Panels a Tab's tree shows, so a context's fingerprint changes as its Panels leave or return. */
function panelIdsOf(node: WorkbenchPanelNode): readonly string[] {
	return node.kind === 'panel'
		? [node.panelId]
		: node.children.flatMap((slot) => panelIdsOf(slot.node));
}

/** Each Client numbers a Tab by its current place, so moving or closing a Tab renumbers others. */
function tabPositionTitle(position: number, intl: IntlShape): string {
	return intl.formatMessage(workbenchTabMessages.positionTitle, { position: `${position + 1}` });
}

/** A Tab without a chosen name follows its focused terminal, so a blank label chooses nothing. */
function customTabTitle(tab: Tab): string | null {
	if (tab.automaticLabel === true) return null;
	const title = tab.label.trim();
	return title.length === 0 ? null : title;
}

function projectPanelNode(node: WorkbenchPanelNode, projection: PanelProjection): PanelNode {
	if (node.kind === 'panel') {
		return {
			activeChildPresentationId: null,
			childPresentations: node.childPresentations.map((presentation) =>
				projectPresentation(presentation, projection),
			),
			id: node.panelId,
			kind: 'panel',
			primaryPresentation: projectPresentation(node.primaryPresentation, projection),
		};
	}
	const [first, second] = node.children;
	const firstWeight = (node.ratio ?? EVEN_SPLIT_RATIO) * 100;
	return {
		children: [
			{ id: first.slotId, node: projectPanelNode(first.node, projection), weight: firstWeight },
			{
				id: second.slotId,
				node: projectPanelNode(second.node, projection),
				weight: 100 - firstWeight,
			},
		],
		id: node.splitId,
		kind: 'split',
		orientation: node.orientation,
	};
}

/**
 * The program comes from the Server's launch of the current binding, else from its failed launch;
 * the Server's raw label is not a name a person chose, so it names only an Agent row.
 */
function projectPresentation(
	presentation: WorkbenchPresentation,
	{ intl, observedEnds, tabTitle }: PanelProjection,
): WorkbenchTerminalPresentation {
	const launched = presentation.launchProgram ?? presentation.launchFailure?.executable ?? null;
	return {
		creationLabel: presentation.label,
		currentCwd: null,
		...(presentation.customLabel === null ? {} : { customTitle: presentation.customLabel }),
		lifecycle: projectTerminalLifecycle(presentation, observedEnds),
		naturalTitle: null,
		presentationId: presentation.presentationId,
		title: (launched === null ? null : nameServerTerminalProgram(launched, intl)) ?? tabTitle,
	};
}

/**
 * A launched terminal runs until this window sees the Host settle that same binding, and then keeps
 * its end and its output, as the Server keeps the Presentation until a person closes it. A failed
 * launch keeps its binding, yet every Client leaves it unattached, and a member with no binding
 * never launches by itself, since the Server binds one only when a command or a restore starts its
 * terminal. Neither is `running` or `failed`, which a surface would try to attach to. Nothing
 * retries either: a person creates another terminal or closes this one.
 */
function projectTerminalLifecycle(
	presentation: WorkbenchPresentation,
	observedEnds: ObservedServerTerminalEnds,
): TerminalPresentationLifecycle {
	if (presentation.launchFailure !== undefined) {
		return { failure: { code: presentation.launchFailure.reason }, kind: 'launch-failed' };
	}
	if (presentation.binding === null) return { kind: 'unbound' };
	return observedServerTerminalEnd(presentation, observedEnds) ?? { kind: 'running' };
}

function presentationsOf(node: WorkbenchPanelNode): readonly WorkbenchPresentation[] {
	return node.kind === 'panel'
		? [node.primaryPresentation, ...node.childPresentations]
		: node.children.flatMap((slot) => presentationsOf(slot.node));
}
