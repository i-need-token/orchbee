import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixtureDirectoryId,
	fixturePresentationId,
	fixtureProjectId,
	fixtureTabId,
	fixtureWorkspaceId,
	snapshotContext,
	snapshotDirectory,
	snapshotProject,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../../__tests__/server-workbench-snapshot';
import type {
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
	WorkbenchAgentState,
} from '../../types/workbench-agent-observation';
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
	const projectId = fixtureProjectId(1);
	const runtime = createSnapshotRuntime(
		workbenchSnapshot(
			1,
			{
				directories: [snapshotDirectory(1)],
				projects: [snapshotProject(1)],
				workspaces: [
					snapshotWorkspace(1, { projectId }),
					snapshotWorkspace(2, { primaryDirectoryId: fixtureDirectoryId(1), projectId }),
				],
			},
			[snapshotContext(1, [snapshotTab(1), snapshotTab(2)]), snapshotContext(2, [snapshotTab(3)])],
		),
	);
	const secondTabTitle = activeTerminalContext(runtime)
		.tabModel.getSnapshot()
		.tabs.find((tab) => tab.id === fixtureTabId(2))?.title;
	if (secondTabTitle === undefined) throw new Error('Expected the second Tab.');
	return {
		firstTabPresentation: fixturePresentationId(1),
		firstWorkspaceId: fixtureWorkspaceId(1),
		runtime,
		secondTabPresentation: fixturePresentationId(2),
		secondTabTitle,
		secondWorkspaceId: fixtureWorkspaceId(2),
		secondWorkspacePresentation: fixturePresentationId(3),
	};
}
