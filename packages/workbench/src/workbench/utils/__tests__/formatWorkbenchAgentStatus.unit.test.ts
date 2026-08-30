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
			glyph: '?',
			provider: 'Codex',
			status: 'Thinking · Status stale',
			tone: 'degraded',
		});
	});

	it.each([
		[{ count: 1, kind: 'needs-attention' }, '!', 'attention'],
		[{ kind: 'failed' }, '×', 'danger'],
		[{ kind: 'canceled' }, '×', 'danger'],
		[{ kind: 'interrupted' }, '×', 'danger'],
		[{ kind: 'succeeded' }, '✓', 'success'],
		[{ activity: null, kind: 'working', phase: 'working' }, '•', 'info'],
		[{ kind: 'idle' }, '•', 'neutral'],
		[{ activityHint: null, kind: 'unknown' }, '?', 'neutral'],
	] as const)('maps %o to a visible semantic glyph and tone', (status, glyph, tone) => {
		expect(
			formatWorkbenchAgentStatus(createOrchbeeIntl('en-US'), {
				activeCount: 0,
				attentionCount: status.kind === 'needs-attention' ? 1 : 0,
				freshness: 'fresh',
				providerIds: [AgentProviderIdSchema.parse('codex')],
				sessionCount: 1,
				status,
			}),
		).toMatchObject({ glyph, tone });
	});
});
