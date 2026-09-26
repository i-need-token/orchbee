import { describe, expect, it } from 'vitest';

import { resolveAgentHookAdapter } from '../resolveAgentHookAdapter';

describe('resolveAgentHookAdapter', () => {
	it('grants Mastracode lifecycle and approvals while rejecting unobserved events', () => {
		expect(resolveAgentHookAdapter('mastracode', 'approval-requested')).toMatchObject({
			providerId: 'mastracode',
			capabilities: ['sessionLifecycle', 'turnLifecycle', 'activity', 'approvals', 'subagents'],
		});
		for (const event of [
			'session-started',
			'session-idle',
			'question-requested',
			'turn-interrupted',
			'compaction-started',
		] as const)
			expect(resolveAgentHookAdapter('mastracode', event)).toBeNull();
	});

	it('maps Antigravity CLI session identity to its canonical Agent provider', () => {
		expect(resolveAgentHookAdapter('antigravity-cli', 'session-identified')).toEqual({
			adapterId: 'antigravity-cli',
			capabilities: ['sessionLifecycle'],
			providerId: 'antigravity',
		});
		for (const event of [
			'session-started',
			'session-idle',
			'turn-started',
			'approval-requested',
			'session-ended',
		] as const) {
			expect(resolveAgentHookAdapter('antigravity-cli', event)).toBeNull();
		}
	});
	it('limits Kimi to identified and ended session events', () => {
		for (const event of ['session-identified', 'session-ended'] as const) {
			expect(resolveAgentHookAdapter('kimi', event)).toEqual({
				adapterId: 'kimi',
				capabilities: ['sessionLifecycle'],
				providerId: 'kimi',
			});
		}
		for (const event of [
			'session-started',
			'session-idle',
			'turn-started',
			'turn-stopped',
			'approval-requested',
		] as const)
			expect(resolveAgentHookAdapter('kimi', event)).toBeNull();
	});
	it('limits Droid to session identity without legacy or lifecycle claims', () => {
		expect(resolveAgentHookAdapter('droid', 'session-identified')).toEqual({
			adapterId: 'droid',
			capabilities: ['sessionLifecycle'],
			providerId: 'droid',
		});
		for (const event of [
			'session-started',
			'session-idle',
			'turn-started',
			'approval-requested',
			'session-ended',
		] as const) {
			expect(resolveAgentHookAdapter('droid', event)).toBeNull();
		}
	});
	it('limits Qoder to session identity without legacy or lifecycle claims', () => {
		expect(resolveAgentHookAdapter('qodercli', 'session-identified')).toEqual({
			adapterId: 'qodercli',
			capabilities: ['sessionLifecycle'],
			providerId: 'qodercli',
		});
		for (const event of [
			'session-started',
			'session-idle',
			'turn-started',
			'approval-requested',
			'session-ended',
		] as const) {
			expect(resolveAgentHookAdapter('qodercli', event)).toBeNull();
		}
	});
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
