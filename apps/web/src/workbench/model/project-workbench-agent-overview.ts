import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { WorkbenchWorkspace } from '../../project-workspace';
import { collectPanelLeaves } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type {
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
	WorkbenchAgentState,
} from '../types/workbench-agent-observation';
import type { WorkbenchAgentTitleSource } from './resolveWorkbenchAgentTitle';
import type { WorkbenchProjectWorkspaceRuntime } from './WorkbenchProjectWorkspaceRuntime';

type WorkbenchAgentTerminalPosition =
	| { readonly kind: 'primary' }
	| { readonly kind: 'child'; readonly position: number };

/** Where a terminal Presentation sits in its Workspace. */
export interface WorkbenchAgentDestination {
	readonly panelId: string;
	readonly panelPosition: number;
	readonly presentationId: TerminalPresentationId;
	readonly projectId: string | null;
	readonly tabId: string;
	readonly tabTitle: string;
	readonly terminalContextId: string;
	/** The terminal's names that an Agent row's title comes from. */
	readonly terminalNames: WorkbenchAgentTitleSource;
	readonly terminalPosition: WorkbenchAgentTerminalPosition;
	readonly workspaceId: string;
	readonly workspaceName: string;
}

interface WorkbenchAgentRowOverview {
	readonly destination: WorkbenchAgentDestination;
	readonly observation: WorkbenchAgentObservation;
}

interface WorkbenchWorkspaceAgentOverview {
	/** Distinct Agent labels in row order. */
	readonly agents: readonly string[];
	readonly projectId: string | null;
	readonly rows: readonly WorkbenchAgentRowOverview[];
	/** The rows most urgent first and otherwise in row order, as a folded Workspace lists their states. */
	readonly rowsByUrgency: readonly WorkbenchAgentRowOverview[];
	readonly state: WorkbenchAgentState;
	readonly workspaceId: string;
	readonly workspaceName: string;
}

export interface WorkbenchAgentNavigationInput {
	/**
	 * The terminal incarnation a notification announced. A Presentation rebound to another terminal
	 * is then no longer the destination, and nothing else replaces it.
	 */
	readonly binding?: string;
	readonly presentationId: TerminalPresentationId;
	readonly workspaceId: string;
}

/** The TUI's order: what waits on the person first, then outcomes, then activity. */
const STATE_URGENCY: Readonly<Record<WorkbenchAgentState, number>> = {
	blocked: 5,
	error: 4,
	done: 3,
	working: 2,
	idle: 1,
	unknown: 0,
};

export function mostUrgentWorkbenchAgentState(
	states: Iterable<WorkbenchAgentState>,
): WorkbenchAgentState | null {
	let urgent: WorkbenchAgentState | null = null;
	for (const state of states) {
		if (urgent === null || STATE_URGENCY[state] > STATE_URGENCY[urgent]) urgent = state;
	}
	return urgent;
}

/** Each Workspace's Agents in Tab, Panel and terminal order, as the TUI sidebar lists them. */
export function projectWorkbenchAgentOverview(
	snapshot: WorkbenchAgentSnapshot,
	runtime: WorkbenchProjectWorkspaceRuntime,
): readonly WorkbenchWorkspaceAgentOverview[] {
	const workspaces: WorkbenchWorkspaceAgentOverview[] = [];
	for (const workspace of runtime.getSnapshot().workspaces) {
		const rows = collectWorkspaceDestinations(runtime, workspace).flatMap((destination) => {
			const observation = snapshot.presentations.get(destination.presentationId);
			return observation === undefined ? [] : [{ destination, observation }];
		});
		const state = mostUrgentWorkbenchAgentState(rows.map((row) => row.observation.state));
		if (state === null) continue;
		workspaces.push({
			agents: [...new Set(rows.map((row) => row.observation.agent))],
			projectId: workspace.projectId,
			rows,
			// `sort` is stable, so equally urgent rows keep their order.
			rowsByUrgency: [...rows].sort(
				(left, right) =>
					STATE_URGENCY[right.observation.state] - STATE_URGENCY[left.observation.state],
			),
			state,
			workspaceId: workspace.id,
			workspaceName: workspace.name,
		});
	}
	return workspaces;
}

/** The terminal an Agent row opens, while it still holds that Agent. */
export function resolveWorkbenchAgentNavigationTarget(
	input: WorkbenchAgentNavigationInput,
	snapshot: WorkbenchAgentSnapshot,
	runtime: WorkbenchProjectWorkspaceRuntime,
): WorkbenchAgentDestination | null {
	const observation = snapshot.presentations.get(input.presentationId);
	if (observation === undefined) return null;
	if (input.binding !== undefined && observation.binding !== input.binding) return null;
	const workspace = runtime
		.getSnapshot()
		.workspaces.find((candidate) => candidate.id === input.workspaceId);
	if (workspace === undefined) return null;
	return (
		collectWorkspaceDestinations(runtime, workspace).find(
			(destination) => destination.presentationId === input.presentationId,
		) ?? null
	);
}

/** Where a Presentation sits, in whichever Workspace holds it. */
export function findWorkbenchAgentDestination(
	runtime: WorkbenchProjectWorkspaceRuntime,
	presentationId: string,
): WorkbenchAgentDestination | null {
	for (const workspace of runtime.getSnapshot().workspaces) {
		const destination = collectWorkspaceDestinations(runtime, workspace).find(
			(candidate) => candidate.presentationId === presentationId,
		);
		if (destination !== undefined) return destination;
	}
	return null;
}

function collectWorkspaceDestinations(
	runtime: WorkbenchProjectWorkspaceRuntime,
	workspace: WorkbenchWorkspace,
): WorkbenchAgentDestination[] {
	const context = runtime.getTerminalContext(workspace.terminalContextId);
	if (context === null) return [];
	const panelTabs = new Map(
		context.panelWorkspaceStore.getSnapshot().tabs.map((tab) => [tab.tabId, tab] as const),
	);
	const destinations: WorkbenchAgentDestination[] = [];
	for (const tab of context.tabModel.getSnapshot().tabs) {
		const panelTab = panelTabs.get(tab.id);
		if (panelTab === undefined) continue;
		for (const [panelIndex, panel] of collectPanelLeaves(panelTab.root).entries()) {
			const location = {
				panelId: panel.id,
				panelPosition: panelIndex + 1,
				projectId: workspace.projectId,
				tabId: tab.id,
				tabTitle: tab.title,
				terminalContextId: workspace.terminalContextId,
				workspaceId: workspace.id,
				workspaceName: workspace.name,
			};
			destinations.push({
				...location,
				presentationId: panel.primaryPresentation.presentationId,
				terminalNames: panel.primaryPresentation,
				terminalPosition: { kind: 'primary' },
			});
			for (const [childIndex, child] of panel.childPresentations.entries()) {
				destinations.push({
					...location,
					presentationId: child.presentationId,
					terminalNames: child,
					terminalPosition: { kind: 'child', position: childIndex + 1 },
				});
			}
		}
	}
	return destinations;
}
