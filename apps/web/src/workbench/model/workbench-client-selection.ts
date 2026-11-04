import {
	collectPanelLeaves,
	findPanel,
	getDisplayedTerminal,
	type PanelWorkspaceEvent,
	type TabPanelWorkspace,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import { findWorkbenchAgentDestination } from './project-workbench-agent-overview';
import type {
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchTerminalContext,
} from './WorkbenchProjectWorkspaceRuntime';

/** What a window shows, in the terms every Client uses to report its view to the Server. */
export interface WorkbenchClientSelection {
	/** How many Panels the shown Tab holds: zero exactly when the Workspace has no Tab. */
	readonly panelCount: number;
	/** The Panel the Tab draws in front: the one filling the Tab, else the focused one. */
	readonly panelId: string | null;
	readonly tabId: string | null;
	readonly terminalContextId: string;
	/** Whether that Panel fills its Tab. As in the TUI, a Tab of one Panel never counts as zoomed. */
	readonly zoomed: boolean;
}

/** Where a Client is asked to look. A Panel names its Tab, and both name their Workspace's context. */
export type WorkbenchClientTarget =
	| { readonly kind: 'context'; readonly terminalContextId: string }
	| { readonly kind: 'tab'; readonly tabId: string; readonly terminalContextId: string }
	| {
			readonly kind: 'panel';
			readonly panelId: string;
			readonly tabId: string;
			readonly terminalContextId: string;
			/** Whether the Panel fills its Tab afterwards; absent, a zoomed Tab stays zoomed on it. */
			readonly zoomed?: boolean | undefined;
	  };

type WorkbenchClientNavigation =
	/** Nothing changed: the Workspace, Tab or Panel the target names is not there. */
	{ readonly kind: 'unavailable' } | { readonly changed: boolean; readonly kind: 'shown' };

/** A terminal that a jump may show, as the Server reports where a session is attached. */
interface WorkbenchPresentationCandidate {
	readonly presentationId: string;
	readonly visible: boolean;
}

const UNAVAILABLE: WorkbenchClientNavigation = { kind: 'unavailable' };

/**
 * Reads what the window shows: null while it shows no Workspace, and undefined while what it shows
 * is not settled, such as a Tab whose Panels are not there yet.
 */
export function readWorkbenchClientSelection(
	runtime: WorkbenchProjectWorkspaceRuntime,
): WorkbenchClientSelection | null | undefined {
	const { activeLocation, workspaces } = runtime.getSnapshot();
	if (activeLocation.kind !== 'workspace') return null;
	const workspace = workspaces.find(({ id }) => id === activeLocation.workspaceId);
	const context =
		workspace === undefined ? null : runtime.getTerminalContext(workspace.terminalContextId);
	if (context === null) return undefined;
	const { terminalContextId } = context;
	const tabId = context.tabModel.getSnapshot().activeTabId;
	if (tabId === null) {
		return { panelCount: 0, panelId: null, tabId: null, terminalContextId, zoomed: false };
	}
	const tab = panelTab(context, tabId);
	if (tab === null) return undefined;
	const panelCount = collectPanelLeaves(tab.root).length;
	const zoomedPanelId = panelCount > 1 ? tab.maximizedPanelId : null;
	return {
		panelCount,
		panelId: zoomedPanelId ?? tab.focusedPanelId,
		tabId,
		terminalContextId,
		zoomed: zoomedPanelId !== null,
	};
}

/** The terminal a Panel shows: its active child terminal, else its own; null once the Panel is gone. */
export function readWorkbenchShownPresentationId(
	runtime: WorkbenchProjectWorkspaceRuntime,
	{ panelId, tabId, terminalContextId }: Extract<WorkbenchClientTarget, { readonly kind: 'panel' }>,
): string | null {
	const context = runtime.getTerminalContext(terminalContextId);
	const tab = context === null ? null : panelTab(context, tabId);
	const panel = tab === null ? null : findPanel(tab.root, panelId);
	return panel === null ? null : getDisplayedTerminal(panel).presentationId;
}

/**
 * Shows a target as the TUI does. A Workspace returns to the Tab and Panel it showed last unless the
 * target names them, a zoomed Tab stays zoomed onto the Panel that takes its focus, and only a Tab of
 * several Panels zooms. Nothing changes when a part the target names is missing. The Workspace is
 * shown last, so the window's route moves once, straight to the Tab the target leaves shown.
 */
export function showWorkbenchClientTarget(
	runtime: WorkbenchProjectWorkspaceRuntime,
	target: WorkbenchClientTarget,
): WorkbenchClientNavigation {
	const workspace = runtime
		.getSnapshot()
		.workspaces.find(({ terminalContextId }) => terminalContextId === target.terminalContextId);
	const context =
		workspace === undefined ? null : runtime.getTerminalContext(workspace.terminalContextId);
	if (workspace === undefined || context === null) return UNAVAILABLE;
	let changed = false;
	if (target.kind !== 'context') {
		const tabs = context.tabModel.getSnapshot();
		const tab = panelTab(context, target.tabId);
		if (
			tab === null ||
			!tabs.tabs.some(({ id }) => id === target.tabId) ||
			(target.kind === 'panel' && findPanel(tab.root, target.panelId) === null)
		) {
			return UNAVAILABLE;
		}
		if (tabs.activeTabId !== target.tabId) {
			context.tabModel.selectTab(target.tabId);
			changed = true;
		}
		if (target.kind === 'panel') {
			const event = panelFocusEvent(tab, target);
			if (event !== null) {
				changed = context.panelWorkspaceStore.transition(event).outcome === 'applied' || changed;
			}
		}
	}
	const { activeLocation } = runtime.getSnapshot();
	if (activeLocation.kind !== 'workspace' || activeLocation.workspaceId !== workspace.id) {
		changed =
			runtime.transition({ type: 'workspace-activated', workspaceId: workspace.id }).outcome ===
				'applied' || changed;
	}
	return { changed, kind: 'shown' };
}

/**
 * Shows the first candidate this window holds, with its terminal in front of its Panel, in the order
 * the TUI's Agent board tries them: one in the shown Tab first, then a visible one, then by
 * identity. Nothing changes when the window holds none of them.
 */
export function showWorkbenchPresentation(
	runtime: WorkbenchProjectWorkspaceRuntime,
	candidates: readonly WorkbenchPresentationCandidate[],
): WorkbenchClientNavigation {
	const shown = readWorkbenchClientSelection(runtime);
	const placed = candidates.flatMap(({ presentationId, visible }) => {
		const destination = findWorkbenchAgentDestination(runtime, presentationId);
		if (destination === null) return [];
		const inShownTab =
			shown?.terminalContextId === destination.terminalContextId &&
			shown.tabId === destination.tabId;
		return [{ destination, rank: (inShownTab ? 2 : 0) + (visible ? 1 : 0) }];
	});
	placed.sort(
		(left, right) =>
			right.rank - left.rank ||
			compareIdentities(left.destination.presentationId, right.destination.presentationId),
	);
	for (const { destination } of placed) {
		const context = runtime.getTerminalContext(destination.terminalContextId);
		if (context === null) continue;
		const { panelId, presentationId, tabId, terminalContextId } = destination;
		const fronted = context.panelWorkspaceStore.transition(
			destination.terminalPosition.kind === 'primary'
				? { panelId, tabId, type: 'primary-terminal-shown' }
				: { childPresentationId: presentationId, panelId, tabId, type: 'child-terminal-activated' },
		);
		if (fronted.outcome === 'rejected') continue;
		const navigation = showWorkbenchClientTarget(runtime, {
			kind: 'panel',
			panelId,
			tabId,
			terminalContextId,
		});
		if (navigation.kind === 'shown') {
			return { changed: navigation.changed || fronted.outcome === 'applied', kind: 'shown' };
		}
	}
	return UNAVAILABLE;
}

function compareIdentities(left: string, right: string): number {
	if (left === right) return 0;
	return left < right ? -1 : 1;
}

/** The one change that focuses the Panel and leaves its Tab zoomed as the target asks, if any. */
function panelFocusEvent(
	tab: TabPanelWorkspace,
	{ panelId, tabId, zoomed }: Extract<WorkbenchClientTarget, { readonly kind: 'panel' }>,
): PanelWorkspaceEvent | null {
	const several = collectPanelLeaves(tab.root).length > 1;
	const zoom = several && (zoomed ?? tab.maximizedPanelId !== null);
	if (zoom) {
		// Maximizing another Panel moves the zoom onto it and focuses it in one step.
		if (tab.maximizedPanelId !== panelId)
			return { panelId, tabId, type: 'panel-maximization-toggled' };
	} else if (tab.maximizedPanelId !== null) {
		return { panelId, tabId, type: 'panel-revealed' };
	}
	return tab.focusedPanelId === panelId ? null : { panelId, tabId, type: 'panel-focused' };
}

function panelTab(context: WorkbenchTerminalContext, tabId: string): TabPanelWorkspace | null {
	return (
		context.panelWorkspaceStore.getSnapshot().tabs.find((candidate) => candidate.tabId === tabId) ??
		null
	);
}
