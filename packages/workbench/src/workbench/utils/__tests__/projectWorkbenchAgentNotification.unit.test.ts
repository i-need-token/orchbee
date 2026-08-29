import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentRequestIdSchema,
	AgentRuntime,
	AgentSessionIdSchema,
} from '@orchbee/agent';
import { createOrchbeeIntl } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { describe, expect, it } from 'vitest';

import { terminalPresentationId } from '../../../terminal-presentation';
import { PanelWorkspaceStore } from '../../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import { projectWorkbenchAgentNotification } from '../projectWorkbenchAgentNotification';

describe('projectWorkbenchAgentNotification', () => {
	it('projects hidden attention and navigation to the exact attached tab', () => {
		const tabModel = new TabModel({
			activeTabId: 'tab-a',
			tabs: [
				{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' },
				{ group: null, id: 'tab-b', pinned: false, title: 'Terminal B' },
			],
		});
		let sequence = 1;
		const panelWorkspaceStore = new PanelWorkspaceStore(tabModel, {
			composition: {
				createPresentationId: () => terminalPresentationId(`projection:dynamic:${sequence++}`),
				initialPresentationId: terminalPresentationId('projection:fallback'),
				kind: 'preview',
			},
			initialPresentations: [
				{ cwd: null, presentationId: terminalPresentationId('projection:a') },
				{ cwd: null, presentationId: terminalPresentationId('projection:b') },
			],
		});
		const runtime = new AgentRuntime();
		const sessionId = AgentSessionIdSchema.parse('projection-session');
		runtime.dispatch({
			capabilities: ['questions'],
			eventId: AgentEventIdSchema.parse('projection-observed'),
			executionLocation: {
				executionLocationId: AgentExecutionLocationIdSchema.parse('projection-execution'),
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
					generation: 'projection-generation',
					navigable: true,
					presentationId: 'projection:b',
					visibility: 'hidden',
				},
			],
			providerId: AgentProviderIdSchema.parse('claude-code'),
			sessionId,
			type: 'session-observed',
		});
		const transition = runtime.dispatch({
			eventId: AgentEventIdSchema.parse('projection-attention'),
			generation: 0,
			request: {
				actionPath: 'navigate-only',
				kind: 'question',
				requestId: AgentRequestIdSchema.parse('projection-request'),
				turnId: null,
			},
			sequence: 2,
			sessionId,
			type: 'attention-requested',
		});
		const event = transition.semanticEvents[0];
		const session = transition.snapshot.sessions[0];
		expect(event).toBeDefined();
		expect(session).toBeDefined();
		if (event === undefined || session === undefined) return;

		const observation = projectWorkbenchAgentNotification({
			event,
			intl: createOrchbeeIntl('en-US'),
			panelWorkspaceStore,
			session,
			tabModel,
		});

		expect(observation).toMatchObject({
			notification: {
				body: 'Claude Code is waiting for your response.',
				title: 'Agent needs attention',
			},
			visible: false,
		});
		observation?.notification.onActivate();
		expect(tabModel.getSnapshot().activeTabId).toBe('tab-b');
	});
});
