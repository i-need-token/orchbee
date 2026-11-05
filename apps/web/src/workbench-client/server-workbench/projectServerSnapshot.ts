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

import type { ServerTabLeaving } from './ServerLeavingTabs';
import { observedServerTerminalEnd, type ObservedServerTerminalEnds } from './server-terminal-ends';
import { nameServerTerminalProgram } from './server-terminal-names';

type Navigation = WorkbenchSnapshot['navigation'];
type Directory = Navigation['directories'][number];
type Workspace = Navigation['workspaces'][number];
type Tab = WorkbenchTerminalContext['tabs'][number];
type PanelNode = WorkbenchPublishedTab['root'];

const NO_OBSERVED_ENDS: ObservedServerTerminalEnds = new Map();
const NONE_LEAVING: ServerTabLeaving = () => false;

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
 * window saw end. A Tab `leaving` names is left out, as the TUI leaves out a Tab whose close is
 * progressing, and so is a group it leaves empty.
 */
export function projectServerSnapshot(
	snapshot: WorkbenchSnapshot,
	intl: IntlShape,
	observedEnds = NO_OBSERVED_ENDS,
	leaving = NONE_LEAVING,
): WorkbenchPublication {
	const { navigation } = snapshot;
	const directories = new Map(
		navigation.directories.map((directory) => [directory.directoryId, directory]),
	);
	return {
		projects: inDisplayOrder(
			navigation.projects,
			navigation.projectOrder,
			(project) => project.projectId,
		).map((project) => ({ id: project.projectId, name: project.label })),
		terminalContexts: snapshot.terminalContexts.map((context) =>
			projectTerminalContext(context, intl, observedEnds, leaving),
		),
		workspaces: inDisplayOrder(
			navigation.workspaces,
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
	leaving: ServerTabLeaving,
): WorkbenchPublishedTerminalContext {
	// A Tab keeps its place among all the Server holds, so no other Tab is renumbered while it leaves.
	const placedTabs = context.tabs.map((tab, position) => ({ position, tab }));
	const withheldTabIds = context.tabs
		.filter((tab) => leaving(context.terminalContextId, tab.tabId))
		.map((tab) => tab.tabId);
	const shownTabs = placedTabs.filter(({ tab }) => !withheldTabIds.includes(tab.tabId));
	const shownGroups = new Set(shownTabs.map(({ tab }) => tab.groupId));
	const heldGroups = new Set(context.tabs.map((tab) => tab.groupId));
	return {
		// A projected context holds names in one language, the ends this window saw, and the Tabs it
		// shows, so another language, another end, or a Tab leaving or returning replaces it.
		fingerprint: JSON.stringify([
			intl.locale,
			context,
			context.tabs.flatMap((tab) =>
				presentationsOf(tab.root).map((presentation) =>
					observedServerTerminalEnd(presentation, observedEnds),
				),
			),
			shownTabs.map(({ tab }) => tab.tabId),
		]),
		groups: context.groups
			.filter((group) => shownGroups.has(group.groupId) || !heldGroups.has(group.groupId))
			.map((group) => ({
				color: group.color,
				id: group.groupId,
				title: group.label,
			})),
		tabs: shownTabs.map(({ position, tab }) => {
			const tabTitle = tabPositionTitle(position, intl);
			return {
				customTitle: customTabTitle(tab),
				group: tab.groupId,
				id: tab.tabId,
				pinned: tab.pinned,
				root: projectPanelNode(tab.root, { intl, observedEnds, tabTitle }),
				title: tab.automaticLabel === true ? tabTitle : tab.label,
			};
		}),
		terminalContextId: context.terminalContextId,
		withheldTabIds,
	};
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
