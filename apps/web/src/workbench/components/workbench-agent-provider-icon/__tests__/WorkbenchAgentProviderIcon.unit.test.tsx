// @vitest-environment jsdom

import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { WorkbenchAgentProviderIcon } from '../WorkbenchAgentProviderIcon';

/** The labels the Server reports for these providers, not their canonical provider IDs. */
const curatedAgents = [
	'claude',
	'cline',
	'codex',
	'gemini',
	'copilot',
	'grok',
	'omp',
	'opencode',
	'pi',
	'qwen',
] as const;

describe('WorkbenchAgentProviderIcon', () => {
	it.each(curatedAgents)('renders the curated %s product mark', (agent) => {
		const { container } = render(
			<WorkbenchAgentProviderIcon agents={[agent]} gradientPrefix={`test-${agent}`} />,
		);

		expect(container.querySelector(`[data-agent-provider-icon="${agent}"]`)).toBeTruthy();
	});

	it('uses the provider-neutral Agent icon for a custom Agent', () => {
		const unknown = render(
			<WorkbenchAgentProviderIcon agents={['private-adapter']} gradientPrefix='test-unknown' />,
		);

		expect(unknown.container.querySelector('[data-icon="agent"]')).toBeTruthy();
	});

	it('overlaps two complete provider marks in registry order', () => {
		const mixed = render(
			<WorkbenchAgentProviderIcon agents={['codex', 'claude']} gradientPrefix='test-mixed' />,
		);

		expect(mixed.container.querySelector('[data-icon="agent"]')).toBeNull();
		expect(
			[...mixed.container.querySelectorAll('[data-agent-provider]')].map((item) =>
				item.getAttribute('data-agent-provider'),
			),
		).toEqual(['claude', 'codex']);
	});

	it('uses the provider-neutral multi-Agent mark for three providers', () => {
		const rendered = render(
			<WorkbenchAgentProviderIcon
				agents={['codex', 'claude', 'gemini']}
				gradientPrefix='test-multi'
			/>,
		);

		expect(rendered.container.querySelector('[data-icon="agent"]')).toBeTruthy();
		expect(rendered.container.querySelector('[data-agent-provider-composition]')).toBeNull();
	});
});
