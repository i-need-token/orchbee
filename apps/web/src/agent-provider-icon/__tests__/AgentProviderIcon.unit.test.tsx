// @vitest-environment jsdom

import { WorkbenchAgentStartProviderSchema } from '@orchbee/workbench-client-protocol';
import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import panelAgentSchema from '../../../../../schemas/workbench/v1/workbench-panel-agent.schema.json';
import { AgentProviderIcon } from '../AgentProviderIcon';
import { hasAgentProductMark } from '../workbench-agent-providers';

/** Every label the Server reports for a provider it recognizes, as the Panel Agent schema lists them. */
const reportedLabels = z
	.object({
		$defs: z.object({
			screenExplanation: z.object({
				properties: z.object({ agent: z.object({ enum: z.array(z.string()).min(1) }) }),
			}),
		}),
	})
	.parse(panelAgentSchema).$defs.screenExplanation.properties.agent.enum;

describe('AgentProviderIcon', () => {
	it.each([...reportedLabels, ...WorkbenchAgentStartProviderSchema.options])(
		'draws the %s product mark',
		(agent) => {
			const { container } = render(
				<AgentProviderIcon agents={[agent]} gradientPrefix={`test-${agent}`} />,
			);

			expect(hasAgentProductMark(agent)).toBe(true);
			expect(container.querySelector(`[data-agent-provider-icon="${agent}"]`)).toBeTruthy();
			expect(container.querySelector('[data-icon="agent"]')).toBeNull();
		},
	);

	it('uses the provider-neutral Agent icon for a custom Agent', () => {
		const unknown = render(
			<AgentProviderIcon agents={['private-adapter']} gradientPrefix='test-unknown' />,
		);

		expect(unknown.container.querySelector('[data-icon="agent"]')).toBeTruthy();
	});

	it('overlaps two complete provider marks in registry order', () => {
		const mixed = render(
			<AgentProviderIcon agents={['codex', 'claude']} gradientPrefix='test-mixed' />,
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
			<AgentProviderIcon agents={['codex', 'claude', 'gemini']} gradientPrefix='test-multi' />,
		);

		expect(rendered.container.querySelector('[data-icon="agent"]')).toBeTruthy();
		expect(rendered.container.querySelector('[data-agent-provider-composition]')).toBeNull();
	});
});
