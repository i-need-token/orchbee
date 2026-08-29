import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentRuntime,
	AgentSessionIdSchema,
} from '@orchbee/agent';
import { describe, expect, it } from 'vitest';

import { projectAgentLocationMap } from '../projectAgentLocationMap';

describe('projectAgentLocationMap', () => {
	it('projects one current aggregate per attached presentation', () => {
		const runtime = new AgentRuntime();
		runtime.dispatch({
			capabilities: [],
			eventId: AgentEventIdSchema.parse('location-map-event'),
			executionLocation: {
				executionLocationId: AgentExecutionLocationIdSchema.parse('location-map-execution'),
			},
			observation: {
				fidelity: 'heuristic',
				freshness: 'fresh',
				generation: 0,
				sequence: 1,
				source: 'heuristic',
			},
			parentSessionId: null,
			presentationAttachments: [
				{
					generation: 'location-map-generation',
					navigable: true,
					presentationId: 'location-map-presentation',
					visibility: 'visible',
				},
			],
			providerId: AgentProviderIdSchema.parse('generic-agent'),
			sessionId: AgentSessionIdSchema.parse('location-map-session'),
			type: 'session-observed',
		});

		expect(
			projectAgentLocationMap(runtime.getSnapshot()).get('location-map-presentation'),
		).toMatchObject({
			freshness: 'fresh',
			status: { kind: 'unknown' },
		});
	});
});
