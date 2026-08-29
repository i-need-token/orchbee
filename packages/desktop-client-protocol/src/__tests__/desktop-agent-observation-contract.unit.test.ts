import {
	AgentProviderIdSchema,
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentSessionIdSchema,
	AgentTurnIdSchema,
	createAgentRuntimeSnapshot,
	transitionAgentRuntime,
} from '@orchbee/agent';
import { describe, expect, it } from 'vitest';

import {
	DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
	DesktopAgentObservationMainEventSchema,
	DesktopAgentObservationRendererCommandSchema,
} from '../desktop-agent-observation-contract';

describe('Desktop Agent observation contract', () => {
	it('round-trips one bounded full snapshot and correlated request', () => {
		const request = DesktopAgentObservationRendererCommandSchema.parse({
			requestId: 'agent-observation:request-1',
			type: 'desktop-agent-observation-snapshot-requested',
			version: DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
		});
		const transition = transitionAgentRuntime(createAgentRuntimeSnapshot(), {
			capabilities: ['sessionLifecycle', 'turnLifecycle'],
			eventId: AgentEventIdSchema.parse('event-1'),
			executionLocation: {
				executionLocationId: AgentExecutionLocationIdSchema.parse('execution-1'),
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
					generation: 'binding-1',
					navigable: true,
					presentationId: 'presentation-1',
					visibility: 'visible',
				},
			],
			providerId: AgentProviderIdSchema.parse('claude-code'),
			sessionId: AgentSessionIdSchema.parse('session-1'),
			type: 'session-observed',
		});
		expect(transition.kind).toBe('applied');

		const event = DesktopAgentObservationMainEventSchema.parse({
			requestId: request.requestId,
			snapshot: transition.snapshot,
			type: 'desktop-agent-observation-snapshot-published',
			version: DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
		});

		expect(event.type).toBe('desktop-agent-observation-snapshot-published');
		if (event.type !== 'desktop-agent-observation-snapshot-published') {
			throw new Error('Expected an Agent snapshot publication.');
		}
		expect(event.snapshot.sessions[0]).toMatchObject({
			providerId: 'claude-code',
			sessionId: 'session-1',
		});
	});

	it('rejects unknown fields and prohibited provider content', () => {
		expect(
			DesktopAgentObservationRendererCommandSchema.safeParse({
				prompt: 'secret prompt',
				requestId: 'agent-observation:request-1',
				type: 'desktop-agent-observation-snapshot-requested',
				version: DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
			}).success,
		).toBe(false);
		expect(
			DesktopAgentObservationMainEventSchema.safeParse({
				requestId: null,
				snapshot: {
					revision: 1,
					sessions: [
						{
							activityHint: null,
							capabilities: ['turnLifecycle'],
							executionLocation: { executionLocationId: 'execution-1' },
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
							prompt: 'secret prompt',
							presentationAttachments: [
								{
									generation: 'binding-1',
									navigable: true,
									presentationId: 'presentation-1',
									visibility: 'visible',
								},
							],
							providerId: AgentProviderIdSchema.parse('claude-code'),
							sessionId: AgentSessionIdSchema.parse('session-1'),
							turn: {
								activity: 'thinking',
								kind: 'active',
								phase: 'working',
								turnId: AgentTurnIdSchema.parse('turn-1'),
							},
							pendingRequests: [],
						},
					],
				},
				type: 'desktop-agent-observation-snapshot-published',
				version: DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
			}).success,
		).toBe(false);
	});
});
