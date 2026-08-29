// @vitest-environment jsdom

import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentRequestIdSchema,
	AgentRuntime,
	AgentSessionIdSchema,
} from '@orchbee/agent';
import { OrchbeeI18nProvider, useOrchbeeIntl } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { act, cleanup, render } from '@testing-library/react';
import { createElement } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../terminal-presentation';
import { PanelWorkspaceStore } from '../../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import type { WorkbenchAgentNotificationObserver } from '../../types/workbench-agent-notification';
import { useWorkbenchAgentNotifications } from '../useWorkbenchAgentNotifications';

afterEach(cleanup);

function Harness({
	observer,
	runtime,
	store,
	tabModel,
}: {
	readonly observer: WorkbenchAgentNotificationObserver;
	readonly runtime: AgentRuntime;
	readonly store: PanelWorkspaceStore;
	readonly tabModel: TabModel;
}) {
	useWorkbenchAgentNotifications({
		intl: useOrchbeeIntl(),
		observer,
		panelWorkspaceStore: store,
		source: runtime,
		tabModel,
	});
	return null;
}

describe('useWorkbenchAgentNotifications', () => {
	it('reports mux-hidden events and activates the exact local destination', () => {
		const tabModel = new TabModel({
			activeTabId: 'tab-a',
			tabs: [
				{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' },
				{ group: null, id: 'tab-b', pinned: false, title: 'Terminal B' },
			],
		});
		let sequence = 1;
		const store = new PanelWorkspaceStore(tabModel, {
			composition: {
				createPresentationId: () => terminalPresentationId(`notification:dynamic:${sequence++}`),
				initialPresentationId: terminalPresentationId('notification:fallback'),
				kind: 'preview',
			},
			initialPresentations: [
				{ cwd: null, presentationId: terminalPresentationId('notification:a') },
				{ cwd: null, presentationId: terminalPresentationId('notification:b') },
			],
		});
		const runtime = new AgentRuntime();
		const observeAgentEvent = vi.fn<WorkbenchAgentNotificationObserver['observeAgentEvent']>();
		render(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(Harness, {
					observer: { observeAgentEvent },
					runtime,
					store,
					tabModel,
				}),
			),
		);
		const sessionId = AgentSessionIdSchema.parse('notification-session');
		act(() => {
			runtime.dispatch({
				capabilities: ['questions'],
				eventId: AgentEventIdSchema.parse('notification-observed'),
				executionLocation: {
					executionLocationId: AgentExecutionLocationIdSchema.parse('notification-execution'),
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
						generation: 'notification-generation',
						navigable: true,
						presentationId: 'notification:b',
						visibility: 'hidden',
					},
				],
				providerId: AgentProviderIdSchema.parse('claude-code'),
				sessionId,
				type: 'session-observed',
			});
			runtime.dispatch({
				eventId: AgentEventIdSchema.parse('notification-attention'),
				generation: 0,
				request: {
					actionPath: 'navigate-only',
					kind: 'question',
					requestId: AgentRequestIdSchema.parse('notification-request'),
					turnId: null,
				},
				sequence: 2,
				sessionId,
				type: 'attention-requested',
			});
		});

		expect(observeAgentEvent).toHaveBeenCalledOnce();
		const observation = observeAgentEvent.mock.calls[0]?.[0];
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

	it('reports an active outer and inner pane as visible', () => {
		const tabModel = new TabModel({
			activeTabId: 'tab-visible',
			tabs: [{ group: null, id: 'tab-visible', pinned: false, title: 'Terminal' }],
		});
		const store = new PanelWorkspaceStore(tabModel, {
			composition: {
				createPresentationId: () => terminalPresentationId('visible:dynamic'),
				initialPresentationId: terminalPresentationId('visible:presentation'),
				kind: 'preview',
			},
		});
		const runtime = new AgentRuntime();
		const observeAgentEvent = vi.fn<WorkbenchAgentNotificationObserver['observeAgentEvent']>();
		render(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(Harness, {
					observer: { observeAgentEvent },
					runtime,
					store,
					tabModel,
				}),
			),
		);
		const sessionId = AgentSessionIdSchema.parse('visible-session');
		act(() => {
			runtime.dispatch({
				capabilities: ['questions'],
				eventId: AgentEventIdSchema.parse('visible-observed'),
				executionLocation: {
					executionLocationId: AgentExecutionLocationIdSchema.parse('visible-execution'),
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
						generation: 'visible-generation',
						navigable: true,
						presentationId: 'visible:presentation',
						visibility: 'visible',
					},
				],
				providerId: AgentProviderIdSchema.parse('codex'),
				sessionId,
				type: 'session-observed',
			});
			runtime.dispatch({
				eventId: AgentEventIdSchema.parse('visible-attention'),
				generation: 0,
				request: {
					actionPath: 'navigate-only',
					kind: 'question',
					requestId: AgentRequestIdSchema.parse('visible-request'),
					turnId: null,
				},
				sequence: 2,
				sessionId,
				type: 'attention-requested',
			});
		});

		expect(observeAgentEvent).toHaveBeenCalledWith(expect.objectContaining({ visible: true }));
	});
});
