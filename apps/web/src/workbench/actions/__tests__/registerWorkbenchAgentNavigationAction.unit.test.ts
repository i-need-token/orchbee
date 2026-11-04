import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	createSnapshotRuntime,
	fixturePanelId,
	fixturePresentationId,
	fixtureTabId,
	fixtureWorkspaceId,
	oneWorkspaceSnapshot,
	snapshotPanel,
	snapshotPresentation,
	snapshotSplit,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import {
	findPanel,
	getDisplayedTerminal,
} from '../../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAgentSource } from '../../types/workbench-agent-observation';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../workbench-action-ids';
import { registerWorkbenchAgentNavigationAction } from '../registerWorkbenchAgentNavigationAction';

const WORKSPACE_ID = fixtureWorkspaceId(1);

describe('registerWorkbenchAgentNavigationAction', () => {
	it('re-resolves and opens the exact inactive Workspace Tab before focusing its Panel', async () => {
		const projectWorkspaceRuntime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1), snapshotTab(2)]),
		);
		const context = projectWorkspaceRuntime.getActiveTerminalContext();
		const targetTabId = fixtureTabId(2);
		const targetPresentationId = fixturePresentationId(2);
		context.tabModel.selectTab(fixtureTabId(1));
		projectWorkspaceRuntime.transition({ type: 'standalone-activated' });

		const { execute, registration } = registerNavigation(
			projectWorkspaceRuntime,
			agentSource(targetPresentationId),
		);

		await expect(execute(targetPresentationId)).resolves.toEqual({ kind: 'completed' });
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',
			workspaceId: WORKSPACE_ID,
		});
		expect(context.tabModel.getSnapshot().activeTabId).toBe(targetTabId);
		const selectedTab = context.panelWorkspaceStore
			.getSnapshot()
			.tabs.find((tab) => tab.tabId === targetTabId);
		expect(selectedTab?.focusedPanelId).toBe(fixturePanelId(2));
		expect(
			selectedTab?.root.kind === 'panel'
				? getDisplayedTerminal(selectedTab.root).presentationId
				: null,
		).toBe(targetPresentationId);

		registration.dispose();
	});

	it('blocks a stale row once its terminal no longer holds an Agent', async () => {
		const projectWorkspaceRuntime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1)]),
		);
		const { execute, registration } = registerNavigation(projectWorkspaceRuntime, agentSource());

		await expect(execute(fixturePresentationId(1))).resolves.toMatchObject({
			code: 'action-unavailable',
			kind: 'blocked',
		});

		registration.dispose();
	});

	it('reveals an exact child terminal and removes a conflicting Panel maximization', async () => {
		const targetPanelId = fixturePanelId(1);
		const childPresentationId = fixturePresentationId(3);
		const projectWorkspaceRuntime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [
				snapshotTab(
					1,
					snapshotSplit(
						1,
						'horizontal',
						snapshotPanel(1, snapshotPresentation(1), [snapshotPresentation(3)]),
						snapshotPanel(2),
					),
				),
			]),
		);
		const context = projectWorkspaceRuntime.getActiveTerminalContext();
		const tabId = fixtureTabId(1);
		expect(
			context.panelWorkspaceStore.transition({
				panelId: targetPanelId,
				tabId,
				type: 'primary-terminal-shown',
			}).outcome,
		).toBe('unchanged');
		expect(
			context.panelWorkspaceStore.transition({
				panelId: fixturePanelId(2),
				tabId,
				type: 'panel-maximization-toggled',
			}).outcome,
		).toBe('applied');
		projectWorkspaceRuntime.transition({ type: 'standalone-activated' });

		const { execute, registration } = registerNavigation(
			projectWorkspaceRuntime,
			agentSource(childPresentationId),
		);

		await expect(execute(childPresentationId)).resolves.toEqual({ kind: 'completed' });
		const navigatedTab = context.panelWorkspaceStore.getSnapshot().tabs[0];
		expect(navigatedTab?.focusedPanelId).toBe(targetPanelId);
		expect(navigatedTab?.maximizedPanelId).toBeNull();
		const targetPanel =
			navigatedTab === undefined ? null : findPanel(navigatedTab.root, targetPanelId);
		expect(targetPanel === null ? null : getDisplayedTerminal(targetPanel).presentationId).toBe(
			childPresentationId,
		);

		registration.dispose();
	});
});

function registerNavigation(
	projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime,
	source: WorkbenchAgentSource,
) {
	const actionRuntime = new ActionRuntime();
	const scope = actionRuntime.createScope('workbench');
	const registration = registerWorkbenchAgentNavigationAction({
		projectWorkspaceRuntime,
		scope,
		source,
		typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
	});
	return {
		execute: (presentationId: string) =>
			actionRuntime.execute(
				WORKBENCH_ACTION_IDS.navigateAgent,
				{ presentationId: terminalPresentationId(presentationId), workspaceId: WORKSPACE_ID },
				{ scope, source: 'button' },
			).completion,
		registration,
	};
}

/** The Server observing an Agent in each of these terminals. */
function agentSource(...presentationIds: readonly string[]): WorkbenchAgentSource {
	const snapshot = {
		complete: true,
		presentations: new Map(
			presentationIds.map((presentationId) => [
				presentationId,
				{
					agent: 'codex',
					binding: `terminal:${presentationId}`,
					sequence: 1n,
					state: 'idle' as const,
				},
			]),
		),
	};
	return { getSnapshot: () => snapshot, subscribe: () => () => undefined };
}
