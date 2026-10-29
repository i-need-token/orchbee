import { AgentRuntimeSnapshotSchema } from '@orchbee/agent';
import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import { collectPanelLeaves } from '../../components/workbench-panel-workspace/model/panel-workspace-state';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../WorkbenchProjectWorkspaceRuntime';
import { projectWorkbenchAgentOverview } from '../project-workbench-agent-overview';

describe('projectWorkbenchAgentOverview', () => {
	it('projects live Sessions per Workspace with attachment deduplication and stable topology order', () => {
		const runtime = createOverviewRuntime();
		const project = runtime.getSnapshot().projects[0];
		const firstWorkspace = runtime.getSnapshot().workspaces[0];
		expect(firstWorkspace).toBeDefined();
		if (project === undefined || firstWorkspace === undefined) return;
		const firstContext = runtime.getTerminalContext(firstWorkspace.terminalContextId);
		expect(firstContext).not.toBeNull();
		if (firstContext === null) return;
		const firstPresentation = presentationIds(firstContext)[0];
		expect(firstPresentation).toBeDefined();
		if (firstPresentation === undefined) return;
		const secondTab = runtime.createTerminal({
			groupId: null,
			source: 'programmatic',
			title: 'Second Tab',
		});
		expect(secondTab.kind).toBe('created');
		const secondPresentation = presentationIds(firstContext)[1];
		expect(secondPresentation).toBeDefined();
		if (secondPresentation === undefined) return;
		const secondTabTitle = firstContext.tabModel.getSnapshot().tabs[1]?.title;
		expect(secondTabTitle).toBeDefined();
		if (secondTabTitle === undefined) return;

		const secondWorkspaceTransition = runtime.transition({
			directoryId: firstWorkspace.primaryDirectoryId,
			launchConfigurationId: null,
			name: 'Second Workspace',
			resourceScopeId: project.id.replace(/^project-/u, 'resource-scope-'),
			projectId: project.id,
			terminalTitle: 'Second Workspace Terminal',
			type: 'workspace-created',
		});
		expect(secondWorkspaceTransition.outcome).toBe('applied');
		const secondWorkspace = runtime
			.getSnapshot()
			.workspaces.filter(
				(workspace) => workspace.projectId === runtime.getSnapshot().projects[0]?.id,
			)[1];
		expect(secondWorkspace).toBeDefined();
		if (secondWorkspace === undefined) return;
		const secondContext = runtime.getTerminalContext(secondWorkspace.terminalContextId);
		expect(secondContext).not.toBeNull();
		if (secondContext === null) return;
		const workspaceTwoPresentation = presentationIds(secondContext)[0];
		expect(workspaceTwoPresentation).toBeDefined();
		if (workspaceTwoPresentation === undefined) return;

		const snapshot = AgentRuntimeSnapshotSchema.parse({
			revision: 8,
			sessions: [
				session({
					attachments: [
						{ navigable: false, presentationId: firstPresentation },
						{ navigable: true, presentationId: secondPresentation },
						{ navigable: true, presentationId: workspaceTwoPresentation },
					],
					providerId: 'codex',
					sessionId: 'session-shared',
					status: 'working',
				}),
				session({
					attachments: [{ navigable: true, presentationId: firstPresentation }],
					providerId: 'claude-code',
					sessionId: 'session-first-tab',
					status: 'idle',
				}),
				session({
					attachments: [{ navigable: true, presentationId: firstPresentation }],
					lifecycle: 'ended',
					providerId: 'gemini',
					sessionId: 'session-ended',
					status: 'idle',
				}),
			],
		});

		const overview = projectWorkbenchAgentOverview(snapshot, runtime);
		expect(overview.workspaces).toHaveLength(2);
		expect(overview.workspaces[0]).toMatchObject({
			providerIds: ['claude-code', 'codex'],
			sessionCount: 2,
			workspaceId: firstWorkspace.id,
		});
		expect(overview.workspaces[0]?.sessions.map((item) => item.sessionId)).toEqual([
			'session-first-tab',
			'session-shared',
		]);
		expect(overview.workspaces[0]?.sessions[1]?.destination).toMatchObject({
			navigable: true,
			presentationId: secondPresentation,
			tabTitle: secondTabTitle,
		});
		expect(overview.workspaces[1]).toMatchObject({
			providerIds: ['codex'],
			sessionCount: 1,
			workspaceId: secondWorkspace.id,
		});
		expect(overview.workspaces[1]?.sessions[0]?.sessionId).toBe('session-shared');
	});

	it('handles the bounded 512-Session snapshot without changing topology order for status', () => {
		const runtime = createOverviewRuntime();
		const workspace = runtime
			.getSnapshot()
			.workspaces.find((entry) => entry.projectId === runtime.getSnapshot().projects[0]?.id);
		expect(workspace).toBeDefined();
		if (workspace === undefined) return;
		const context = runtime.getTerminalContext(workspace.terminalContextId);
		expect(context).not.toBeNull();
		if (context === null) return;
		const presentationId = presentationIds(context)[0];
		expect(presentationId).toBeDefined();
		if (presentationId === undefined) return;
		const sessions = Array.from({ length: 512 }, (_, index) =>
			session({
				attachments: [{ navigable: index % 2 === 0, presentationId }],
				providerId: 'codex',
				sessionId: `session-${String(index).padStart(3, '0')}`,
				status: index === 0 ? 'working' : 'idle',
			}),
		);
		const first = projectWorkbenchAgentOverview(
			AgentRuntimeSnapshotSchema.parse({ revision: 1, sessions }),
			runtime,
		).workspaces[0];
		const statusChanged = sessions.map((value, index) =>
			index === 511
				? { ...value, capabilities: ['questions'], pendingRequests: [attentionRequest()] }
				: value,
		);
		const second = projectWorkbenchAgentOverview(
			AgentRuntimeSnapshotSchema.parse({ revision: 2, sessions: statusChanged }),
			runtime,
		).workspaces[0];

		expect(first?.sessionCount).toBe(512);
		expect(second?.status).toEqual({ count: 1, kind: 'needs-attention' });
		expect(second?.sessions.map((item) => item.sessionId)).toEqual(
			first?.sessions.map((item) => item.sessionId),
		);
	});
});

function createOverviewRuntime(): WorkbenchProjectWorkspaceRuntime {
	let presentationSequence = 0;
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Standalone',
		terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
			terminalPresentationId('overview-standalone'),
			() => terminalPresentationId(`overview-presentation-${(presentationSequence += 1)}`),
		),
	});
	const transition = runtime.transition({
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
	if (transition.outcome !== 'applied') throw new Error('Expected overview Project creation.');
	return runtime;
}

function presentationIds(
	context: NonNullable<ReturnType<WorkbenchProjectWorkspaceRuntime['getTerminalContext']>>,
): readonly string[] {
	return context.panelWorkspaceStore
		.getSnapshot()
		.tabs.flatMap((tab) =>
			collectPanelLeaves(tab.root).flatMap((panel) => [
				panel.primaryPresentation.presentationId,
				...panel.childPresentations.map((presentation) => presentation.presentationId),
			]),
		);
}

function session(input: {
	readonly attachments: readonly { readonly navigable: boolean; readonly presentationId: string }[];
	readonly lifecycle?: 'ended' | 'live';
	readonly providerId: string;
	readonly sessionId: string;
	readonly status: 'idle' | 'working';
}) {
	return {
		activityHint: null,
		capabilities: input.status === 'working' ? ['turnLifecycle'] : [],
		executionLocation: { executionLocationId: `execution-${input.sessionId}` },
		lastOutcome: null,
		lifecycle: input.lifecycle ?? 'live',
		observation: {
			fidelity: 'rich',
			freshness: 'fresh',
			generation: 0,
			sequence: 1,
			source: 'hook',
		},
		parentSessionId: null,
		pendingRequests: [],
		presentationAttachments: input.attachments.map((attachment, index) => ({
			generation: `generation-${input.sessionId}-${index}`,
			navigable: attachment.navigable,
			presentationId: attachment.presentationId,
			visibility: 'visible',
		})),
		providerId: input.providerId,
		sessionId: input.sessionId,
		turn:
			input.status === 'working'
				? {
						activity: 'thinking',
						kind: 'active',
						phase: 'working',
						turnId: `turn-${input.sessionId}`,
					}
				: { kind: 'idle' },
	};
}

function attentionRequest() {
	return {
		actionPath: 'navigate-only',
		kind: 'question',
		requestId: 'request-session-511',
		turnId: null,
	};
}
