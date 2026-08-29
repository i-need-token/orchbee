import { AgentProviderIdSchema } from '@orchbee/agent';
import { createOrchbeeIntl } from '@orchbee/i18n';
import { describe, expect, it } from 'vitest';

import { formatWorkbenchAgentProvider } from '../formatWorkbenchAgentProvider';

describe('formatWorkbenchAgentProvider', () => {
	it('uses curated provider labels without exposing adapter identifiers', () => {
		const intl = createOrchbeeIntl('en-US');

		expect(formatWorkbenchAgentProvider(intl, [AgentProviderIdSchema.parse('claude-code')])).toBe(
			'Claude Code',
		);
		expect(formatWorkbenchAgentProvider(intl, [AgentProviderIdSchema.parse('codex')])).toBe(
			'Codex',
		);
		expect(formatWorkbenchAgentProvider(intl, [AgentProviderIdSchema.parse('grok')])).toBe('Grok');
		expect(formatWorkbenchAgentProvider(intl, [AgentProviderIdSchema.parse('opencode')])).toBe(
			'OpenCode',
		);
		expect(formatWorkbenchAgentProvider(intl, [AgentProviderIdSchema.parse('pi')])).toBe('Pi');
		expect(formatWorkbenchAgentProvider(intl, [AgentProviderIdSchema.parse('omp')])).toBe('OMP');
		expect(
			formatWorkbenchAgentProvider(intl, [AgentProviderIdSchema.parse('private-adapter')]),
		).toBe('Agent');
		expect(
			formatWorkbenchAgentProvider(intl, [
				AgentProviderIdSchema.parse('claude-code'),
				AgentProviderIdSchema.parse('codex'),
			]),
		).toBe('Agents');
	});
});
