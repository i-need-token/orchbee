import { describe, expect, it } from 'vitest';

import { resolveAgentHookAdapter } from '../resolveAgentHookAdapter';

describe('resolveAgentHookAdapter', () => {
	it('grants capabilities from the closed registry instead of adapter input', () => {
		expect(resolveAgentHookAdapter('codex', 'question-requested')).toMatchObject({
			adapterId: 'codex',
			capabilities: expect.arrayContaining(['questions', 'approvals']),
			providerId: 'codex',
		});
		expect(resolveAgentHookAdapter('grok', 'question-requested')).toBeNull();
		expect(resolveAgentHookAdapter('pi', 'approval-requested')).toBeNull();
		expect(resolveAgentHookAdapter('omp', 'approval-requested')).toMatchObject({
			providerId: 'omp',
		});
	});

	it('keeps provider contracts independent even when normalized events overlap', () => {
		expect(resolveAgentHookAdapter('opencode', 'session-idle')).toMatchObject({
			providerId: 'opencode',
		});
		expect(resolveAgentHookAdapter('claude-code', 'session-idle')).toBeNull();
		expect(resolveAgentHookAdapter('grok', 'turn-canceled')).toMatchObject({
			providerId: 'grok',
		});
	});
});
