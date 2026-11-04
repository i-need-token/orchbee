import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import { collectPanelLeaves } from '../../components/workbench-panel-workspace/model/panel-workspace-state';
import type {
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
	WorkbenchAgentState,
} from '../../types/workbench-agent-observation';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../WorkbenchProjectWorkspaceRuntime';
import {
	findWorkbenchAgentDestination,
	projectWorkbenchAgentOverview,
	resolveWorkbenchAgentNavigationTarget,
} from '../project-workbench-agent-overview';

describe('Workbench Agent overview', () => {
	it('lists each Workspace’s Agents in topology order, and by urgency for a fold, under its most urgent state', () => {
		const fixture = createOverviewFixture();
		const overview = projectWorkbenchAgentOverview(
			agents([
				[fixture.secondTabPresentation, observation('codex', 'working')],
				[fixture.firstTabPresentation, observation('claude', 'idle')],
				[fixture.secondWorkspacePresentation, observation('codex', 'blocked')],
				// A terminal this Workbench does not hold has no row anywhere.
				['elsewhere', observation('gemini', 'blocked')],
			]),
			fixture.runtime,
		);

		expect(
			overview.map((workspace) => ({
				agents: workspace.agents,
				byUrgency: workspace.rowsByUrgency.map((row) => row.destination.presentationId),
				presentations: workspace.rows.map((row) => row.destination.presentationId),
				state: workspace.state,
				workspaceId: workspace.workspaceId,
			})),
		).toEqual([
			{
				agents: ['claude', 'codex'],
				byUrgency: [fixture.secondTabPresentation, fixture.firstTabPresentation],
				presentations: [fixture.firstTabPresentation, fixture.secondTabPresentation],
				state: 'working',
				workspaceId: fixture.firstWorkspaceId,
			},
			{
				agents: ['codex'],
				byUrgency: [fixture.secondWorkspacePresentation],
				presentations: [fixture.secondWorkspacePresentation],
				state: 'blocked',
				workspaceId: fixture.secondWorkspaceId,
			},
		]);
		expect(overview[0]?.rows[1]?.destination).toMatchObject({
			panelPosition: 1,
			tabTitle: fixture.secondTabTitle,
			terminalPosition: { kind: 'primary' },
		});
	});

	it('opens a row only while its terminal holds an Agent in the named Workspace', () => {
		const fixture = createOverviewFixture();
		const observed = agents([[fixture.secondTabPresentation, observation('codex', 'done')]]);
		const input = {
			presentationId: terminalPresentationId(fixture.secondTabPresentation),
			workspaceId: fixture.firstWorkspaceId,
		};

		expect(resolveWorkbenchAgentNavigationTarget(input, observed, fixture.runtime)).toMatchObject({
			presentationId: fixture.secondTabPresentation,
			tabTitle: fixture.secondTabTitle,
		});
		expect(
			resolveWorkbenchAgentNavigationTarget(
				{ ...input, workspaceId: fixture.secondWorkspaceId },
				observed,
				fixture.runtime,
			),
		).toBeNull();
		expect(resolveWorkbenchAgentNavigationTarget(input, agents([]), fixture.runtime)).toBeNull();
		expect(
			findWorkbenchAgentDestination(fixture.runtime, fixture.secondWorkspacePresentation),
		).toMatchObject({ workspaceId: fixture.secondWorkspaceId });
		expect(findWorkbenchAgentDestination(fixture.runtime, 'elsewhere')).toBeNull();
	});
});

function observation(agent: string, state: WorkbenchAgentState): WorkbenchAgentObservation {
	return { agent, binding: `terminal:${agent}`, sequence: 1n, state };
}

function agents(
	entries: readonly (readonly [string, WorkbenchAgentObservation])[],
): WorkbenchAgentSnapshot {
	return { complete: true, presentations: new Map(entries) };
}

/** One Project with two Workspaces: the first holds two Tabs, the second one Tab. */
function createOverviewFixture() {
	let presentationSequence = 0;
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Standalone',
		terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
			terminalPresentationId('overview-standalone'),
			() => terminalPresentationId(`overview-presentation-${(presentationSequence += 1)}`),
		),
	});
	const created = runtime.transition({
		directory: {
			available: true,
			git: null,
			id: 'overview-directory',
			kind: 'folder',
			name: 'overview',
			path: '/overview',
			placement: { kind: 'local' },
			source: 'selected',
		},
		launchConfigurations: [],
		primaryWorkspaceLaunchConfigurationId: null,
		primaryWorkspaceName: 'First Workspace',
		projectName: 'Overview',
		terminalTitle: 'First Tab',
		type: 'project-created',
	});
	const project = runtime.getSnapshot().projects[0];
	const firstWorkspace = runtime.getSnapshot().workspaces[0];
	if (created.outcome !== 'applied' || project === undefined || firstWorkspace === undefined) {
		throw new Error('Expected the overview Project.');
	}
	const firstTabId = tabIds(runtime, firstWorkspace.terminalContextId)[0];
	const secondTab = runtime.createTerminal({
		groupId: null,
		source: 'programmatic',
		title: 'Second Tab',
	});
	if (firstTabId === undefined || secondTab.kind !== 'created') {
		throw new Error('Expected the second Tab.');
	}
	const workspaceCreated = runtime.transition({
		directoryId: firstWorkspace.primaryDirectoryId,
		launchConfigurationId: null,
		name: 'Second Workspace',
		resourceScopeId: project.id.replace(/^project-/u, 'resource-scope-'),
		projectId: project.id,
		terminalTitle: 'Second Workspace Terminal',
		type: 'workspace-created',
	});
	const secondWorkspace = runtime
		.getSnapshot()
		.workspaces.find((workspace) => workspace.id !== firstWorkspace.id);
	if (workspaceCreated.outcome !== 'applied' || secondWorkspace === undefined) {
		throw new Error('Expected the second Workspace.');
	}
	const firstContext = runtime.getTerminalContext(firstWorkspace.terminalContextId);
	const firstTabPresentation = tabPresentation(
		runtime,
		firstWorkspace.terminalContextId,
		firstTabId,
	);
	const secondTabPresentation = tabPresentation(
		runtime,
		firstWorkspace.terminalContextId,
		secondTab.tabId,
	);
	const secondWorkspacePresentation = tabPresentation(
		runtime,
		secondWorkspace.terminalContextId,
		tabIds(runtime, secondWorkspace.terminalContextId)[0],
	);
	const secondTabTitle = firstContext?.tabModel
		.getSnapshot()
		.tabs.find((tab) => tab.id === secondTab.tabId)?.title;
	if (
		firstTabPresentation === undefined ||
		secondTabPresentation === undefined ||
		secondWorkspacePresentation === undefined ||
		secondTabTitle === undefined
	) {
		throw new Error('Expected a terminal in every Tab.');
	}
	return {
		firstTabPresentation,
		firstWorkspaceId: firstWorkspace.id,
		runtime,
		secondTabPresentation,
		secondTabTitle,
		secondWorkspaceId: secondWorkspace.id,
		secondWorkspacePresentation,
	};
}

function tabIds(runtime: WorkbenchProjectWorkspaceRuntime, terminalContextId: string): string[] {
	const context = runtime.getTerminalContext(terminalContextId);
	return context === null ? [] : context.tabModel.getSnapshot().tabs.map((tab) => tab.id);
}

function tabPresentation(
	runtime: WorkbenchProjectWorkspaceRuntime,
	terminalContextId: string,
	tabId: string | undefined,
): string | undefined {
	const panelTab = runtime
		.getTerminalContext(terminalContextId)
		?.panelWorkspaceStore.getSnapshot()
		.tabs.find((tab) => tab.tabId === tabId);
	return panelTab === undefined
		? undefined
		: collectPanelLeaves(panelTab.root)[0]?.primaryPresentation.presentationId;
}
