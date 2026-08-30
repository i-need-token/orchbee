import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { AgentRuntime, AgentRuntimeSnapshotSchema } from '@orchbee/agent';
import { describe, expect, it } from 'vitest';

import { terminalPresentationId } from '../../../terminal-presentation';
import {
	collectPanelLeaves,
	findPanel,
	getDisplayedTerminal,
} from '../../components/workbench-panel-workspace/model/panel-workspace-state';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../../model/createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../workbench-action-ids';
import { registerWorkbenchAgentNavigationAction } from '../registerWorkbenchAgentNavigationAction';

describe('registerWorkbenchAgentNavigationAction', () => {
	it('re-resolves and opens the exact inactive Workspace Tab before focusing its Panel', async () => {
		const projectWorkspaceRuntime = projectRuntime();
		const project = projectWorkspaceRuntime.getSnapshot().projects[0];
		const workspace = project?.workspaces[0];
		expect(project).toBeDefined();
		expect(workspace).toBeDefined();
		if (project === undefined || workspace === undefined) return;
		const context = projectWorkspaceRuntime.getTerminalContext(workspace.terminalContextId);
		expect(context).not.toBeNull();
		if (context === null) return;
		const creation = projectWorkspaceRuntime.createTerminal({
			groupId: null,
			source: 'programmatic',
			title: 'Agent Tab',
		});
		expect(creation.kind).toBe('created');
		if (creation.kind !== 'created') return;
		const targetTab = context.panelWorkspaceStore
			.getSnapshot()
			.tabs.find((tab) => tab.tabId === creation.tabId);
		expect(targetTab).toBeDefined();
		if (targetTab === undefined) return;
		const targetPanel = targetTab.root.kind === 'panel' ? targetTab.root : null;
		expect(targetPanel).not.toBeNull();
		if (targetPanel === null) return;
		const targetPresentationId = targetPanel.primaryPresentation.presentationId;
		const firstTabId = context.tabModel.getSnapshot().tabs[0]?.id;
		expect(firstTabId).toBeDefined();
		if (firstTabId === undefined) return;
		context.tabModel.selectTab(firstTabId);
		projectWorkspaceRuntime.transition({ type: 'standalone-activated' });

		const source = new AgentRuntime(
			AgentRuntimeSnapshotSchema.parse({
				revision: 1,
				sessions: [session('session-agent-navigation', targetPresentationId)],
			}),
		);
		const actionRuntime = new ActionRuntime();
		const scope = actionRuntime.createScope('workbench');
		const registration = registerWorkbenchAgentNavigationAction({
			projectWorkspaceRuntime,
			scope,
			source,
			typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
		});
		const sessionId = source.getSnapshot().sessions[0]?.sessionId;
		if (sessionId === undefined) return;

		await expect(
			actionRuntime.execute(
				WORKBENCH_ACTION_IDS.navigateAgent,
				{
					projectId: project.id,
					sessionId,
					workspaceId: workspace.id,
				},
				{ scope, source: 'button' },
			).completion,
		).resolves.toEqual({ kind: 'completed' });
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',
			projectId: project.id,
			workspaceId: workspace.id,
		});
		expect(context.tabModel.getSnapshot().activeTabId).toBe(creation.tabId);
		const selectedTab = context.panelWorkspaceStore
			.getSnapshot()
			.tabs.find((tab) => tab.tabId === creation.tabId);
		expect(selectedTab?.focusedPanelId).toBe(targetPanel.id);
		expect(
			selectedTab?.root.kind === 'panel'
				? getDisplayedTerminal(selectedTab.root).presentationId
				: null,
		).toBe(targetPresentationId);

		registration.dispose();
	});

	it('blocks a stale row after the Agent Session has ended', async () => {
		const projectWorkspaceRuntime = projectRuntime();
		const project = projectWorkspaceRuntime.getSnapshot().projects[0];
		const workspace = project?.workspaces[0];
		if (project === undefined || workspace === undefined) return;
		const context = projectWorkspaceRuntime.getTerminalContext(workspace.terminalContextId);
		if (context === null) return;
		const tab = context.panelWorkspaceStore.getSnapshot().tabs[0];
		if (tab?.root.kind !== 'panel') return;
		const source = new AgentRuntime(
			AgentRuntimeSnapshotSchema.parse({
				revision: 2,
				sessions: [
					{
						...session('session-ended-navigation', tab.root.primaryPresentation.presentationId),
						lifecycle: 'ended',
					},
				],
			}),
		);
		const actionRuntime = new ActionRuntime();
		const scope = actionRuntime.createScope('workbench');
		const registration = registerWorkbenchAgentNavigationAction({
			projectWorkspaceRuntime,
			scope,
			source,
			typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
		});
		const sessionId = source.getSnapshot().sessions[0]?.sessionId;
		if (sessionId === undefined) return;

		await expect(
			actionRuntime.execute(
				WORKBENCH_ACTION_IDS.navigateAgent,
				{
					projectId: project.id,
					sessionId,
					workspaceId: workspace.id,
				},
				{ scope, source: 'button' },
			).completion,
		).resolves.toMatchObject({ code: 'action-unavailable', kind: 'blocked' });

		registration.dispose();
	});

	it('reveals an exact child terminal and removes a conflicting Panel maximization', async () => {
		const projectWorkspaceRuntime = projectRuntime();
		const project = projectWorkspaceRuntime.getSnapshot().projects[0];
		const workspace = project?.workspaces[0];
		if (project === undefined || workspace === undefined) return;
		const context = projectWorkspaceRuntime.getTerminalContext(workspace.terminalContextId);
		if (context === null) return;
		const initialTab = context.panelWorkspaceStore.getSnapshot().tabs[0];
		if (initialTab?.root.kind !== 'panel') return;
		const tabId = initialTab.tabId;
		const targetPanelId = initialTab.root.id;
		const childPresentation = context.panelWorkspaceStore.createTerminalPresentation();
		expect(
			context.panelWorkspaceStore.transition({
				newTerminalTitle: 'Agent child',
				panelId: targetPanelId,
				presentation: childPresentation,
				tabId,
				type: 'child-terminal-created',
			}).outcome,
		).toBe('applied');
		const childPresentationId = childPresentation.presentationId;
		expect(
			context.panelWorkspaceStore.transition({
				panelId: targetPanelId,
				tabId,
				type: 'primary-terminal-shown',
			}).outcome,
		).toBe('applied');
		expect(
			context.panelWorkspaceStore.transition({
				direction: 'right',
				newTerminalTitle: 'Other Panel',
				panelId: targetPanelId,
				presentation: context.panelWorkspaceStore.createTerminalPresentation(),
				tabId,
				type: 'panel-split',
			}).outcome,
		).toBe('applied');
		const splitTab = context.panelWorkspaceStore.getSnapshot().tabs[0];
		if (splitTab === undefined) return;
		const otherPanel = collectPanelLeaves(splitTab.root).find(
			(panel) => panel.id !== targetPanelId,
		);
		if (otherPanel === undefined) return;
		expect(
			context.panelWorkspaceStore.transition({
				panelId: otherPanel.id,
				tabId,
				type: 'panel-maximization-toggled',
			}).outcome,
		).toBe('applied');
		projectWorkspaceRuntime.transition({ type: 'standalone-activated' });

		const source = new AgentRuntime(
			AgentRuntimeSnapshotSchema.parse({
				revision: 3,
				sessions: [session('session-child-navigation', childPresentationId)],
			}),
		);
		const actionRuntime = new ActionRuntime();
		const scope = actionRuntime.createScope('workbench');
		const registration = registerWorkbenchAgentNavigationAction({
			projectWorkspaceRuntime,
			scope,
			source,
			typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
		});
		const sessionId = source.getSnapshot().sessions[0]?.sessionId;
		if (sessionId === undefined) return;

		await expect(
			actionRuntime.execute(
				WORKBENCH_ACTION_IDS.navigateAgent,
				{ projectId: project.id, sessionId, workspaceId: workspace.id },
				{ scope, source: 'button' },
			).completion,
		).resolves.toEqual({ kind: 'completed' });
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

function projectRuntime(): WorkbenchProjectWorkspaceRuntime {
	let sequence = 0;
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Standalone',
		terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
			terminalPresentationId('agent-navigation-initial'),
			() => terminalPresentationId(`agent-navigation-${(sequence += 1)}`),
		),
	});
	const transition = runtime.transition({
		directory: {
			available: true,
			git: null,
			id: 'directory-agent-navigation',
			kind: 'folder',
			name: 'Agent Navigation',
			path: '/agent-navigation',
			placement: { kind: 'local' },
			source: 'selected',
		},
		launchConfigurations: [],
		primaryWorkspaceLaunchConfigurationId: null,
		primaryWorkspaceName: 'Workspace',
		projectName: 'Project',
		terminalTitle: 'First Tab',
		type: 'project-created',
	});
	if (transition.outcome !== 'applied') throw new Error('Expected Project creation.');
	return runtime;
}

function session(sessionId: string, presentationId: string) {
	return {
		activityHint: null,
		capabilities: ['turnLifecycle'],
		executionLocation: { executionLocationId: `execution-${sessionId}` },
		lastOutcome: null,
		lifecycle: 'live',
		observation: {
			fidelity: 'rich',
			freshness: 'fresh',
			generation: 0,
			sequence: 1,
			source: 'hook',
		},
		parentSessionId: null,
		pendingRequests: [],
		presentationAttachments: [
			{
				generation: `generation-${sessionId}`,
				navigable: true,
				presentationId,
				visibility: 'visible',
			},
		],
		providerId: 'codex',
		sessionId,
		turn: { kind: 'idle' },
	};
}
