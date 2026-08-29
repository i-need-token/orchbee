// @vitest-environment jsdom

import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentRuntime,
	AgentSessionIdSchema,
	AgentTurnIdSchema,
} from '@orchbee/agent';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { describe, expect, it } from 'vitest';

import { terminalPresentationId } from '../../../terminal-presentation';
import { PanelWorkspaceStore } from '../../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import { useWorkbenchAgentPresentation } from '../useWorkbenchAgentPresentation';

describe('useWorkbenchAgentPresentation', () => {
	it('prepares a provider-neutral location and tab adornment from one snapshot', () => {
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
		const sessionId = AgentSessionIdSchema.parse('agent-session-hook');
		runtime.dispatch({
			capabilities: ['turnLifecycle', 'activity'],
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
					source: runtime,
					tabModel,
				}),
			{
				wrapper: ({ children }: { readonly children: ReactNode }) =>
					createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children),
			},
		);

		expect(rendered.result.current.locations.get(presentationId)?.status).toEqual({
			activity: 'thinking',
			kind: 'working',
			phase: 'working',
		});
		expect(rendered.result.current.tabStatusAdornments.get('agent-tab-hook')).toMatchObject({
			accessibleLabel: 'Claude Code: Thinking',
			kind: 'active',
		});
	});
});
