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

	it('uses the provider-neutral Agent icon for an unknown provider', () => {
		const unknown = render(
			<WorkbenchAgentProviderIcon
				gradientPrefix='test-unknown'
				providerIds={[AgentProviderIdSchema.parse('private-adapter')]}
			/>,
		);

		expect(unknown.container.querySelector('[data-icon="agent"]')).toBeTruthy();
	});

	it('overlaps two complete provider marks in registry order', () => {
		const mixed = render(
			<WorkbenchAgentProviderIcon
				gradientPrefix='test-mixed'
				providerIds={[
					AgentProviderIdSchema.parse('codex'),
					AgentProviderIdSchema.parse('claude-code'),
				]}
			/>,
		);

		expect(mixed.container.querySelector('[data-icon="agent"]')).toBeNull();
		expect(
			[...mixed.container.querySelectorAll('[data-agent-provider]')].map((item) =>
				item.getAttribute('data-agent-provider'),
			),
		).toEqual(['claude-code', 'codex']);
	});

	it('uses the provider-neutral multi-Agent mark for three providers', () => {
		const rendered = render(
			<WorkbenchAgentProviderIcon
				gradientPrefix='test-multi'
				providerIds={[
					AgentProviderIdSchema.parse('codex'),
					AgentProviderIdSchema.parse('claude-code'),
					AgentProviderIdSchema.parse('gemini'),
				]}
			/>,
		);

		expect(rendered.container.querySelector('[data-icon="agent"]')).toBeTruthy();
		expect(rendered.container.querySelector('[data-agent-provider-composition]')).toBeNull();
	});
});
