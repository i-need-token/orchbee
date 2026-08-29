import type { AgentAggregatePresentation } from '@orchbee/agent';
import { AgentProviderIdSchema } from '@orchbee/agent';
import { createOrchbeeIntl } from '@orchbee/i18n';
import { describe, expect, it } from 'vitest';

import { formatWorkbenchAgentStatus } from '../formatWorkbenchAgentStatus';

describe('formatWorkbenchAgentStatus', () => {
	it('combines semantic state with observation freshness', () => {
		const presentation: AgentAggregatePresentation = {
			activeCount: 1,
			attentionCount: 0,
			freshness: 'stale',
			providerIds: [AgentProviderIdSchema.parse('codex')],
			sessionCount: 1,
			status: { activity: 'thinking', kind: 'working', phase: 'working' },
		};

		expect(formatWorkbenchAgentStatus(createOrchbeeIntl('en-US'), presentation)).toEqual({
			provider: 'Codex',
			status: 'Thinking · Status stale',
			tone: 'degraded',
		});
	});
});
