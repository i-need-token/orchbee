// @vitest-environment jsdom

import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentRuntime,
	AgentSessionIdSchema,
	createAgentRuntimeSnapshot,
} from '@orchbee/agent';
import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { useAgentSnapshot } from '../useAgentSnapshot';

describe('useAgentSnapshot', () => {
	it('subscribes to the source and falls back to the canonical empty snapshot', () => {
		const empty = renderHook(() => useAgentSnapshot(undefined));
		expect(empty.result.current).toEqual(createAgentRuntimeSnapshot());

		const runtime = new AgentRuntime();
		const observed = renderHook(() => useAgentSnapshot(runtime));
		act(() => {
			runtime.dispatch({
				capabilities: [],
				eventId: AgentEventIdSchema.parse('snapshot-observed-event'),
				executionLocation: {
					executionLocationId: AgentExecutionLocationIdSchema.parse('snapshot-execution'),
				},
				observation: {
					fidelity: 'heuristic',
					freshness: 'fresh',
					generation: 0,
					sequence: 1,
					source: 'heuristic',
				},
				parentSessionId: null,
				presentationAttachments: [],
				providerId: AgentProviderIdSchema.parse('generic-agent'),
				sessionId: AgentSessionIdSchema.parse('snapshot-session'),
				type: 'session-observed',
			});
		});

		expect(observed.result.current.revision).toBe(1);
	});
});
