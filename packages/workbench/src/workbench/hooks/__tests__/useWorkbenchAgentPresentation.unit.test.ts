// @vitest-environment jsdom

import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentRuntime,
	AgentRuntimeSnapshotSchema,
	AgentSessionIdSchema,
	AgentTurnIdSchema,
} from '@orchbee/agent';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { act, render, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { describe, expect, it } from 'vitest';

import { terminalPresentationId } from '../../../terminal-presentation';
import { PanelWorkspaceStore } from '../../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { useWorkbenchAgentPresentation } from '../useWorkbenchAgentPresentation';

describe('useWorkbenchAgentPresentation', () => {
	it('projects provider identity independently from the current tab status', () => {
		const presentationId = terminalPresentationId('agent-presentation-hook');
		const tabModel = new TabModel({
			activeTabId: 'agent-tab-hook',
			tabs: [{ group: null, id: 'agent-tab-hook', pinned: false, title: 'Terminal' }],
		});
		const panelWorkspaceStore = new PanelWorkspaceStore(tabModel, {
			composition: {
				createPresentationId: () => terminalPresentationId('agent-presentation-dynamic'),
				initialPresentationId: presentationId,
				kind: 'preview',
			},
		});
		const runtime = new AgentRuntime();
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal',
			terminalPresentation: {
				createPresentationId: () => terminalPresentationId('agent-project-dynamic'),
				initialPresentationId: terminalPresentationId('agent-project-initial'),
				kind: 'preview',
			},
		});
		const sessionId = AgentSessionIdSchema.parse('agent-session-hook');
		runtime.dispatch({
			capabilities: ['sessionLifecycle', 'turnLifecycle', 'activity'],
			eventId: AgentEventIdSchema.parse('agent-observed-hook'),
			executionLocation: {
				executionLocationId: AgentExecutionLocationIdSchema.parse('agent-execution-hook'),
			},
			observation: {
				fidelity: 'rich',
				freshness: 'fresh',
				generation: 0,
				sequence: 1,
				source: 'hook',
			},
			parentSessionId: null,
			presentationAttachments: [
				{
					generation: 'agent-generation-hook',
					navigable: true,
					presentationId,
					visibility: 'visible',
				},
			],
			providerId: AgentProviderIdSchema.parse('claude-code'),
			sessionId,
			type: 'session-observed',
		});
		runtime.dispatch({
			activity: 'thinking',
			eventId: AgentEventIdSchema.parse('agent-turn-hook'),
			generation: 0,
			phase: 'working',
			sequence: 2,
			sessionId,
			turnId: AgentTurnIdSchema.parse('agent-turn-id-hook'),
			type: 'turn-started',
		});

		const rendered = renderHook(
			() =>
				useWorkbenchAgentPresentation({
					notificationObserver: undefined,
					panelWorkspaceStore,
					projectWorkspaceRuntime,
					source: runtime,
					tabModel,
				}),
			{
				wrapper: ({ children }: { readonly children: ReactNode }) =>
					createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children),
			},
		);

		expect(rendered.result.current.tabStatusAdornments.get('agent-tab-hook')).toMatchObject({
			accessibleLabel: 'Claude Code: Thinking',
			kind: 'active',
		});
		const activeIcon = render(
			createElement('div', null, rendered.result.current.tabIdentityIcons.get('agent-tab-hook')),
		);
		expect(
			activeIcon.container.querySelector('[data-agent-provider-icon="claude-code"]'),
		).toBeTruthy();

		act(() => {
			runtime.dispatch({
				eventId: AgentEventIdSchema.parse('agent-ended-hook'),
				generation: 0,
				sequence: 3,
				sessionId,
				type: 'session-ended',
			});
		});

		expect(rendered.result.current.tabStatusAdornments.has('agent-tab-hook')).toBe(false);
		expect(rendered.result.current.tabIdentityIcons.has('agent-tab-hook')).toBe(false);
	});

	it.each([
		{
			countLabel: '100 live Agents',
			expandLabel: 'Show 100 Agents in Primary',
			locale: 'en-US' as const,
		},
		{
			countLabel: '100 个运行中的 Agent',
			expandLabel: '显示“Primary”中的 100 个 Agent',
			locale: 'zh-CN' as const,
		},
	])('caps only the visual count while preserving the exact count in $locale', (expected) => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal',
			terminalPresentation: {
				createPresentationId: () => terminalPresentationId('agent-count-dynamic'),
				initialPresentationId: terminalPresentationId('agent-count-initial'),
				kind: 'preview',
			},
		});
		projectWorkspaceRuntime.transition({
			directory: {
				available: true,
				git: null,
				id: 'agent-count-directory',
				kind: 'folder',
				name: 'Agent count',
				path: '/agent-count',
				placement: { kind: 'local' },
				source: 'selected',
			},
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: 'Primary',
			projectName: 'Project',
			terminalTitle: 'Agent Tab',
			type: 'project-created',
		});
		const workspace = projectWorkspaceRuntime.getSnapshot().projects[0]?.workspaces[0];
		if (workspace === undefined) throw new Error('Expected the Agent count Workspace.');
		const context = projectWorkspaceRuntime.getTerminalContext(workspace.terminalContextId);
		if (context === null) throw new Error('Expected the Agent count terminal context.');
		const tab = context.panelWorkspaceStore.getSnapshot().tabs[0];
		if (tab?.root.kind !== 'panel') throw new Error('Expected the Agent count Panel.');
		const presentationId = tab.root.primaryPresentation.presentationId;
		const runtime = new AgentRuntime(
			AgentRuntimeSnapshotSchema.parse({
				revision: 1,
				sessions: Array.from({ length: 100 }, (_, index) => ({
					activityHint: null,
					capabilities: [],
					executionLocation: { executionLocationId: `agent-count-execution-${index}` },
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
							generation: `agent-count-generation-${index}`,
							navigable: true,
							presentationId,
							visibility: 'visible',
						},
					],
					providerId: 'codex',
					sessionId: `agent-count-session-${index}`,
					turn: { kind: 'idle' },
				})),
			}),
		);

		const rendered = renderHook(
			() =>
				useWorkbenchAgentPresentation({
					notificationObserver: undefined,
					panelWorkspaceStore: context.panelWorkspaceStore,
					projectWorkspaceRuntime,
					source: runtime,
					tabModel: context.tabModel,
				}),
			{
				wrapper: ({ children }: { readonly children: ReactNode }) =>
					createElement(OrchbeeI18nProvider, { locale: expected.locale }, children),
			},
		);

		expect(rendered.result.current.workspaceAgentOverviews[0]).toMatchObject({
			countLabel: expected.countLabel,
			expandLabel: expected.expandLabel,
			sessionCount: 100,
			visibleCountLabel: '99+',
		});
	});
});
