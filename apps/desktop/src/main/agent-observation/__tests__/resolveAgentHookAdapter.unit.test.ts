import { describe, expect, it } from 'vitest';

import { resolveAgentHookAdapter } from '../resolveAgentHookAdapter';

describe('resolveAgentHookAdapter', () => {
	it('limits Cursor to its evidenced session identity hook', () => {
		expect(resolveAgentHookAdapter('cursor', 'session-identified')).toEqual({
			adapterId: 'cursor',
			capabilities: ['sessionLifecycle'],
			providerId: 'cursor',
		});
		expect(resolveAgentHookAdapter('cursor', 'session-started')).toEqual(
			resolveAgentHookAdapter('cursor', 'session-identified'),
		);
		expect(resolveAgentHookAdapter('codex', 'session-identified')).toBeNull();
		for (const event of [
			'turn-started',
			'turn-stopped',
			'session-idle',
			'approval-requested',
		] as const) {
			expect(resolveAgentHookAdapter('cursor', event)).toBeNull();
		}
	});
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
		expect(resolveAgentHookAdapter('gemini', 'approval-requested')).toMatchObject({
			providerId: 'gemini',
		});
		expect(resolveAgentHookAdapter('github-copilot', 'approval-requested')).toMatchObject({
			providerId: 'github-copilot',
		});
		expect(resolveAgentHookAdapter('kilo', 'question-requested')).toMatchObject({
			providerId: 'kilo',
		});
		expect(resolveAgentHookAdapter('kilo', 'subagent-started')).toBeNull();
		expect(resolveAgentHookAdapter('cline', 'approval-requested')).toBeNull();
		expect(resolveAgentHookAdapter('qwen-code', 'subagent-started')).toMatchObject({
			providerId: 'qwen-code',
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
		expect(resolveAgentHookAdapter('github-copilot', 'subagent-started')).toBeNull();
	});
});
