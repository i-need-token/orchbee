// @vitest-environment jsdom

import { AgentProviderIdSchema } from '@orchbee/agent';
import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { WorkbenchAgentProviderIcon } from '../WorkbenchAgentProviderIcon';

const curatedProviderIds = [
	'claude-code',
	'cline',
	'codex',
	'gemini',
	'github-copilot',
	'grok',
	'omp',
	'opencode',
	'pi',
	'qwen-code',
] as const;

describe('WorkbenchAgentProviderIcon', () => {
	it.each(curatedProviderIds)('renders the curated %s product mark', (providerId) => {
		const { container } = render(
			<WorkbenchAgentProviderIcon
				gradientPrefix={`test-${providerId}`}
				providerIds={[AgentProviderIdSchema.parse(providerId)]}
			/>,
		);

		expect(container.querySelector(`[data-agent-provider-icon="${providerId}"]`)).toBeTruthy();
	});

	it('uses the provider-neutral Agent icon for unknown or mixed providers', () => {
		const unknown = render(
			<WorkbenchAgentProviderIcon
				gradientPrefix='test-unknown'
				providerIds={[AgentProviderIdSchema.parse('private-adapter')]}
			/>,
		);
		const mixed = render(
			<WorkbenchAgentProviderIcon
				gradientPrefix='test-mixed'
				providerIds={[
					AgentProviderIdSchema.parse('claude-code'),
					AgentProviderIdSchema.parse('codex'),
				]}
			/>,
		);

		expect(unknown.container.querySelector('[data-icon="agent"]')).toBeTruthy();
		expect(mixed.container.querySelector('[data-icon="agent"]')).toBeTruthy();
	});
});
