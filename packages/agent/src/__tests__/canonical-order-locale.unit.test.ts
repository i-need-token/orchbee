import { expect, it } from 'vitest';

import type { AgentObservationEvent } from '../events';
import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentSessionIdSchema,
} from '../model';
import { createAgentRuntimeSnapshot, transitionAgentRuntime } from '../reducer';

const sessionIds = [
	's:z',
	's:_',
	's:Z',
	's:9',
	's:.',
	's::',
	's:a',
	's:-',
	's:1',
	's:A',
	's:i',
	's:I',
];

const presentationIds = [
	'\u00e9',
	'e\u0301',
	'a\u00adb',
	'ab',
	'a\u200bb',
	'中',
	'文',
	'\ue000',
	'\u{1f600}',
	'z',
	'A',
	'a',
	'0',
	'9',
	'-',
	':',
	'.',
	'_',
	'I',
	'i',
];

export const expectedCanonicalOrder = {
	attachments: [
		'-',
		'.',
		'0',
		'9',
		':',
		'A',
		'I',
		'_',
		'a',
		'ab',
		'a\u00adb',
		'a\u200bb',
		'e\u0301',
		'i',
		'z',
		'\u00e9',
		'\u4e2d',
		'\u6587',
		'\u{1f600}',
		'\ue000',
	],
	sessions: ['s:-', 's:.', 's:1', 's:9', 's::', 's:A', 's:I', 's:Z', 's:_', 's:a', 's:i', 's:z'],
};

export function buildCanonicalOrder(): typeof expectedCanonicalOrder {
	let snapshot = createAgentRuntimeSnapshot();
	let sequence = 0;
	for (const sessionId of sessionIds) {
		sequence += 1;
		const event: AgentObservationEvent = {
			capabilities: [],
			eventId: AgentEventIdSchema.parse(`probe-event-${sequence}`),
			executionLocation: {
				executionLocationId: AgentExecutionLocationIdSchema.parse('probe-exec'),
			},
			observation: {
				fidelity: 'rich',
				freshness: 'fresh',
				generation: 0,
				sequence: 1,
				source: 'hook',
			},
			parentSessionId: null,
			presentationAttachments:
				sessionId === 's:z'
					? presentationIds.map((presentationId, index) => ({
							generation: `gen-${index}`,
							navigable: false,
							presentationId,
							visibility: 'visible' as const,
						}))
					: [],
			providerId: AgentProviderIdSchema.parse('claude-code'),
			sessionId: AgentSessionIdSchema.parse(sessionId),
			type: 'session-observed',
		};
		const transition = transitionAgentRuntime(snapshot, event);
		if (transition.kind !== 'applied') {
			throw new Error(`probe transition failed for ${sessionId}: ${transition.kind}`);
		}
		snapshot = transition.snapshot;
	}
	return {
		attachments: (
			snapshot.sessions.find((session) => session.sessionId === 's:z')?.presentationAttachments ??
			[]
		).map((entry) => entry.presentationId),
		sessions: snapshot.sessions.map((session) => session.sessionId),
	};
}

it('produces the canonical UTF-16 code-unit order under the ambient locale', () => {
	expect(buildCanonicalOrder()).toEqual(expectedCanonicalOrder);
});
