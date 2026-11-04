import { workbenchTabMessages, type IntlShape } from '@orchbee/i18n';
import type { WorkbenchPublication } from '../../workbench';
import { deriveProjectNameFromPath } from '../../project-workspace';
import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type TerminalPresentationLifecycle,
	type WorkbenchPanelNode,
	type WorkbenchPresentation,
	type WorkbenchSharedPanelNode,
	type WorkbenchSharedPresentation,
	type WorkbenchSnapshot,
	type WorkbenchTerminalContext,
} from '@orchbee/workbench-client-protocol';

import { observedServerTerminalEnd, type ObservedServerTerminalEnds } from './server-terminal-ends';
import { nameServerTerminalProgram } from './server-terminal-names';
import { serverResourceScopeId } from './serverResourceScopeId';

type Navigation = WorkbenchSnapshot['navigation'];
type Directory = Navigation['directories'][number];
type Workspace = Navigation['workspaces'][number];
type Tab = WorkbenchTerminalContext['tabs'][number];

/** What projecting the tree reads besides the snapshot, and what it writes besides the document. */
interface PublicationFacts {
	readonly observedEnds: ObservedServerTerminalEnds;
	readonly presentations: Map<
		string,
		{ readonly customTitle: string | null; readonly lifecycle: TerminalPresentationLifecycle }
	>;
	readonly splitWeights: Map<string, readonly [first: number, second: number]>;
}

const NO_OBSERVED_ENDS: ObservedServerTerminalEnds = new Map();

/** How a terminal of one Tab is named when it shows no title. */
interface TerminalFallbackNaming {
	readonly intl: IntlShape;
	readonly tabTitle: string;
}

/** The React Workbench needs one context when no Workspace is open; a Server ID is never this. */
const NO_WORKSPACE_TERMINAL_CONTEXT_ID = 'no-workspace';

/** The Server owns every identity, so a local sequence is never consumed. */
const UNUSED_SEQUENCE = 1;

const EVEN_SPLIT_RATIO = 0.5;

/**
 * Projects what the Server published onto the model the React Workbench reads. Every Workspace gets
 * its own one-directory resource scope, because the Server has no scopes and a Workspace it holds
 * without a directory still needs one here. `defaultDirectory` only names that placeholder, which is
 * unavailable, so the Workbench opens no Workspace in it and says the Workspace has no directory.
 * Names follow the TUI's: a terminal that shows no title takes the program the Server launched for
 * it, else its Tab's place, both in `intl`'s language. `observedEnds` holds, by Presentation, what
 * this window saw end.
 */
export function projectServerSnapshot(
	snapshot: WorkbenchSnapshot,
	defaultDirectory: string,
	intl: IntlShape,
	observedEnds = NO_OBSERVED_ENDS,
): WorkbenchPublication {
	const { navigation } = snapshot;
	const directories = new Map(
		navigation.directories.map((directory) => [directory.directoryId, directory]),
	);
	const workspaces = inDisplayOrder(
		navigation.workspaces,
		navigation.workspaceOrder,
		(workspace) => workspace.workspaceId,
	).map((workspace) => ({
		directory: projectWorkspaceDirectory(workspace, directories, defaultDirectory),
		scopeId: serverResourceScopeId(workspace.workspaceId),
		workspace,
	}));
	const facts: PublicationFacts = {
		observedEnds,
		presentations: new Map(),
		splitWeights: new Map(),
	};
	return {
		// A projected context holds names in one language and the ends this window saw, so another
		// language or another end replaces it.
		contextFingerprints: new Map(
			snapshot.terminalContexts.map((context) => [
				context.terminalContextId,
				JSON.stringify([
					intl.locale,
					context,
					context.tabs.flatMap((tab) =>
						presentationsOf(tab.root).map((presentation) =>
							observedServerTerminalEnd(presentation, observedEnds),
						),
					),
				]),
			]),
		),
		document: createWorkbenchSharedDocument({
			projectWorkspace: {
				nextProjectSequence: UNUSED_SEQUENCE,
				nextResourceScopeSequence: UNUSED_SEQUENCE,
				nextTerminalContextSequence: UNUSED_SEQUENCE,
				nextWorkspaceSequence: UNUSED_SEQUENCE,
				projects: inDisplayOrder(
					navigation.projects,
					navigation.projectOrder,
					(project) => project.projectId,
				).map((project) => ({ id: project.projectId, name: project.label })),
				resourceScopes: workspaces.map(({ directory, scopeId, workspace }) => ({
					directories: [directory],
					id: scopeId,
					launchConfigurations: [],
					name: workspace.label,
					primaryDirectoryId: directory.id,
				})),
				standaloneTerminalContextId: NO_WORKSPACE_TERMINAL_CONTEXT_ID,
				workspaces: workspaces.map(({ directory, scopeId, workspace }) => ({
					id: workspace.workspaceId,
					launchConfigurationId: null,
					name: workspace.label,
					primaryDirectoryId: directory.id,
					projectId: workspace.projectId,
					resourceScopeId: scopeId,
					terminalContextId: workspace.terminalContextId,
				})),
			},
			revision: workbenchSharedDocumentRevision(snapshot.revision),
			terminalContexts: [
				{
					nextTabSequence: UNUSED_SEQUENCE,
					panelWorkspace: {
						nextPanelSequence: UNUSED_SEQUENCE,
						nextPanelSlotSequence: UNUSED_SEQUENCE,
						nextSplitSequence: UNUSED_SEQUENCE,
						nextTerminalSequence: UNUSED_SEQUENCE,
						tabs: [],
					},
					tabCollection: { groups: [], tabs: [] },
					terminalContextId: NO_WORKSPACE_TERMINAL_CONTEXT_ID,
				},
				...snapshot.terminalContexts.map((context) => projectTerminalContext(context, facts, intl)),
			],
		}),
		presentations: facts.presentations,
		splitWeights: facts.splitWeights,
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
	defaultDirectory: string,
) {
	if (workspace.primaryDirectoryId === null) {
		return projectDirectory(
			`no-directory:${workspace.workspaceId}`,
			workspace.terminalDirectory ?? defaultDirectory,
			false,
		);
	}
	const directory = directories.get(workspace.primaryDirectoryId);
	if (directory === undefined) {
		throw new Error(`Workspace ${workspace.workspaceId} names a missing directory.`);
	}
	return projectDirectory(directory.directoryId, directory.path, true);
}

function projectDirectory(id: string, path: string, available: boolean) {
	return {
		available,
		git: null,
		id,
		kind: 'folder' as const,
		name: deriveProjectNameFromPath(path),
		path,
		placement: { kind: 'local' as const },
		source: 'selected' as const,
	};
}

function projectTerminalContext(
	context: WorkbenchTerminalContext,
	facts: PublicationFacts,
	intl: IntlShape,
) {
	return {
		nextTabSequence: UNUSED_SEQUENCE,
		panelWorkspace: {
			nextPanelSequence: UNUSED_SEQUENCE,
			nextPanelSlotSequence: UNUSED_SEQUENCE,
			nextSplitSequence: UNUSED_SEQUENCE,
			nextTerminalSequence: UNUSED_SEQUENCE,
			tabs: context.tabs.map((tab, position) => ({
				root: projectPanelNode(tab.root, facts, {
					intl,
					tabTitle: tabPositionTitle(position, intl),
				}),
				tabId: tab.tabId,
			})),
		},
		tabCollection: {
			groups: context.groups.map((group) => ({
				color: group.color,
				id: group.groupId,
				title: group.label,
			})),
			tabs: context.tabs.map((tab, position) => ({
				customTitle: customTabTitle(tab),
				group: tab.groupId,
				id: tab.tabId,
				pinned: tab.pinned,
				title: tab.automaticLabel === true ? tabPositionTitle(position, intl) : tab.label,
			})),
		},
		terminalContextId: context.terminalContextId,
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

function projectPanelNode(
	node: WorkbenchPanelNode,
	facts: PublicationFacts,
	naming: TerminalFallbackNaming,
): WorkbenchSharedPanelNode {
	if (node.kind === 'panel') {
		return {
			childPresentations: node.childPresentations.map((presentation) =>
				projectPresentation(presentation, facts, naming),
			),
			id: node.panelId,
			kind: 'panel',
			primaryPresentation: projectPresentation(node.primaryPresentation, facts, naming),
		};
	}
	const firstWeight = (node.ratio ?? EVEN_SPLIT_RATIO) * 100;
	facts.splitWeights.set(node.splitId, [firstWeight, 100 - firstWeight]);
	return {
		children: node.children.map((slot) => ({
			id: slot.slotId,
			node: projectPanelNode(slot.node, facts, naming),
		})),
		id: node.splitId,
		kind: 'split',
		orientation: node.orientation,
	};
}

/**
 * The program comes from the Server's launch of the current binding, else from its failed launch;
 * the Server's raw label is not a name a person chose, so it never shows.
 */
function projectPresentation(
	presentation: WorkbenchPresentation,
	facts: PublicationFacts,
	naming: TerminalFallbackNaming,
): WorkbenchSharedPresentation {
	const { intent, lifecycle } = projectTerminalState(presentation, facts.observedEnds);
	facts.presentations.set(presentation.presentationId, {
		customTitle: presentation.customLabel,
		lifecycle,
	});
	const launched = presentation.launchProgram ?? presentation.launchFailure?.executable ?? null;
	return {
		intent,
		presentationId: presentation.presentationId,
		title:
			(launched === null ? null : nameServerTerminalProgram(launched, naming.intl)) ??
			naming.tabTitle,
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
function projectTerminalState(
	presentation: WorkbenchPresentation,
	observedEnds: ObservedServerTerminalEnds,
): {
	readonly intent: WorkbenchSharedPresentation['intent'];
	readonly lifecycle: TerminalPresentationLifecycle;
} {
	if (presentation.launchFailure !== undefined) {
		return {
			intent: { kind: 'provision' },
			lifecycle: { failure: { code: presentation.launchFailure.reason }, kind: 'launch-failed' },
		};
	}
	if (presentation.binding === null) {
		return { intent: { kind: 'provision' }, lifecycle: { kind: 'unbound' } };
	}
	return {
		intent: { kind: 'retain' },
		lifecycle: observedServerTerminalEnd(presentation, observedEnds) ?? { kind: 'running' },
	};
}

function presentationsOf(node: WorkbenchPanelNode): readonly WorkbenchPresentation[] {
	return node.kind === 'panel'
		? [node.primaryPresentation, ...node.childPresentations]
		: node.children.flatMap((slot) => presentationsOf(slot.node));
}
