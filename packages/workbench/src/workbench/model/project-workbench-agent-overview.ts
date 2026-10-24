import {
	projectAgentSession,
	projectAgentSessionGroup,
	type AgentAggregatePresentation,
	type AgentProviderId,
	type AgentRuntimeSnapshot,
	type AgentSemanticStatus,
	type AgentSessionId,
	type AgentSessionSnapshot,
} from '@orchbee/agent';

import { collectPanelLeaves } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchProjectWorkspaceRuntime } from './WorkbenchProjectWorkspaceRuntime';
import type { TerminalPresentationId } from '../../terminal-presentation';

type WorkbenchAgentTerminalPosition =
	| { readonly kind: 'primary' }
	| { readonly kind: 'child'; readonly position: number };

export interface WorkbenchAgentNavigationTarget {
	readonly navigable: boolean;
	readonly panelId: string;
	readonly panelPosition: number;
	readonly presentationId: TerminalPresentationId;
	readonly projectId: string | null;
	readonly tabId: string;
	readonly tabTitle: string;
	readonly terminalContextId: string;
	readonly terminalPosition: WorkbenchAgentTerminalPosition;
	readonly workspaceId: string;
}

interface WorkbenchAgentSessionOverview {
	readonly destination: WorkbenchAgentNavigationTarget;
	readonly freshness: AgentSessionSnapshot['observation']['freshness'];
	readonly providerId: AgentProviderId;
	readonly sessionId: AgentSessionId;
	readonly status: AgentSemanticStatus;
}

interface WorkbenchWorkspaceAgentOverview extends AgentAggregatePresentation {
	readonly projectId: string | null;
	readonly sessions: readonly WorkbenchAgentSessionOverview[];
	readonly workspaceId: string;
}

interface WorkbenchAgentOverview {
	readonly workspaces: readonly WorkbenchWorkspaceAgentOverview[];
}

export interface WorkbenchAgentNavigationInput {
	readonly sessionId: AgentSessionId;
	readonly workspaceId: string;
}

interface IndexedWorkbenchAgentNavigationTarget extends WorkbenchAgentNavigationTarget {
	readonly topologyIndex: number;
}

interface SessionCandidate {
	readonly destination: IndexedWorkbenchAgentNavigationTarget;
	readonly generation: string;
}

export function projectWorkbenchAgentOverview(
	snapshot: AgentRuntimeSnapshot,
	runtime: WorkbenchProjectWorkspaceRuntime,
): WorkbenchAgentOverview {
	const workspaces: WorkbenchWorkspaceAgentOverview[] = [];
	for (const workspace of runtime.getSnapshot().workspaces) {
		const context = runtime.getTerminalContext(workspace.terminalContextId);
		if (context === null) continue;
		const panelState = context.panelWorkspaceStore.getSnapshot();
		const panelsByTabId = new Map(panelState.tabs.map((tab) => [tab.tabId, tab] as const));
		const destinationsByPresentationId = new Map<string, IndexedWorkbenchAgentNavigationTarget>();
		let topologyIndex = 0;
		for (const tab of context.tabModel.getSnapshot().tabs) {
			const panelTab = panelsByTabId.get(tab.id);
			if (panelTab === undefined) continue;
			for (const [panelIndex, panel] of collectPanelLeaves(panelTab.root).entries()) {
				const terminalPresentations = [
					{
						presentationId: panel.primaryPresentation.presentationId,
						terminalPosition: { kind: 'primary' as const },
					},
					...panel.childPresentations.map((presentation, childIndex) => ({
						presentationId: presentation.presentationId,
						terminalPosition: {
							kind: 'child' as const,
							position: childIndex + 1,
						},
					})),
				];
				for (const terminal of terminalPresentations) {
					destinationsByPresentationId.set(terminal.presentationId, {
						navigable: false,
						panelId: panel.id,
						panelPosition: panelIndex + 1,
						presentationId: terminal.presentationId,
						projectId: workspace.projectId,
						tabId: tab.id,
						tabTitle: tab.title,
						terminalContextId: workspace.terminalContextId,
						terminalPosition: terminal.terminalPosition,
						topologyIndex,
						workspaceId: workspace.id,
					});
					topologyIndex += 1;
				}
			}
		}

		const selectedSessions: AgentSessionSnapshot[] = [];
		const sessionOverviews: Array<
			WorkbenchAgentSessionOverview & { readonly topologyIndex: number }
		> = [];
		for (const session of snapshot.sessions) {
			if (session.lifecycle !== 'live') continue;
			const candidates: SessionCandidate[] = [];
			for (const attachment of session.presentationAttachments) {
				const destination = destinationsByPresentationId.get(attachment.presentationId);
				if (destination === undefined) continue;
				candidates.push({
					destination: { ...destination, navigable: attachment.navigable },
					generation: attachment.generation,
				});
			}
			candidates.sort(compareSessionCandidate);
			const candidate = candidates[0];
			if (candidate === undefined) continue;
			const presentation = projectAgentSession(session);
			selectedSessions.push(session);
			sessionOverviews.push({
				destination: withoutTopologyIndex(candidate.destination),
				freshness: presentation.observation.freshness,
				providerId: presentation.providerId,
				sessionId: presentation.sessionId,
				status: presentation.status,
				topologyIndex: candidate.destination.topologyIndex,
			});
		}
		const aggregate = projectAgentSessionGroup(selectedSessions);
		if (aggregate === null) continue;
		sessionOverviews.sort(compareSessionOverview);
		workspaces.push({
			...aggregate,
			projectId: workspace.projectId,
			sessions: sessionOverviews.map(removeSessionTopologyIndex),
			workspaceId: workspace.id,
		});
	}
	return { workspaces };
}

export function resolveWorkbenchAgentNavigationTarget(
	input: WorkbenchAgentNavigationInput,
	snapshot: AgentRuntimeSnapshot,
	runtime: WorkbenchProjectWorkspaceRuntime,
): WorkbenchAgentNavigationTarget | null {
	const workspace = projectWorkbenchAgentOverview(snapshot, runtime).workspaces.find(
		(candidate) => candidate.workspaceId === input.workspaceId,
	);
	const session = workspace?.sessions.find((candidate) => candidate.sessionId === input.sessionId);
	return session?.destination.navigable === true ? session.destination : null;
}

function compareSessionCandidate(left: SessionCandidate, right: SessionCandidate): number {
	const navigableOrder = Number(right.destination.navigable) - Number(left.destination.navigable);
	if (navigableOrder !== 0) return navigableOrder;
	const topologyOrder = left.destination.topologyIndex - right.destination.topologyIndex;
	return topologyOrder === 0 ? right.generation.localeCompare(left.generation) : topologyOrder;
}

function compareSessionOverview(
	left: WorkbenchAgentSessionOverview & { readonly topologyIndex: number },
	right: WorkbenchAgentSessionOverview & { readonly topologyIndex: number },
): number {
	const navigableOrder = Number(right.destination.navigable) - Number(left.destination.navigable);
	if (navigableOrder !== 0) return navigableOrder;
	const topologyOrder = left.topologyIndex - right.topologyIndex;
	return topologyOrder === 0 ? left.sessionId.localeCompare(right.sessionId) : topologyOrder;
}

function withoutTopologyIndex(
	destination: IndexedWorkbenchAgentNavigationTarget,
): WorkbenchAgentNavigationTarget {
	return {
		navigable: destination.navigable,
		panelId: destination.panelId,
		panelPosition: destination.panelPosition,
		presentationId: destination.presentationId,
		projectId: destination.projectId,
		tabId: destination.tabId,
		tabTitle: destination.tabTitle,
		terminalContextId: destination.terminalContextId,
		terminalPosition: destination.terminalPosition,
		workspaceId: destination.workspaceId,
	};
}

function removeSessionTopologyIndex(
	session: WorkbenchAgentSessionOverview & { readonly topologyIndex: number },
): WorkbenchAgentSessionOverview {
	return {
		destination: session.destination,
		freshness: session.freshness,
		providerId: session.providerId,
		sessionId: session.sessionId,
		status: session.status,
	};
}
