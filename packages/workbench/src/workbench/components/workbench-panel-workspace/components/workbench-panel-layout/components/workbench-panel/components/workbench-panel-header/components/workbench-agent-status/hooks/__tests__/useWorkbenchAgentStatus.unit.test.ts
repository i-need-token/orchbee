import { AgentProviderIdSchema, type AgentLocationPresentation } from '@orchbee/agent';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { useWorkbenchAgentStatus } from '../useWorkbenchAgentStatus';

function AgentStatusProbe({
	agentPresentation,
}: {
	readonly agentPresentation: AgentLocationPresentation;
}) {
	const view = useWorkbenchAgentStatus(agentPresentation);
	return `${view.provider}|${view.status}|${view.tone}`;
}

function renderStatus(
	locale: 'en-US' | 'zh-CN',
	agentPresentation: AgentLocationPresentation,
): string {
	return renderToStaticMarkup(
		createElement(
			OrchbeeI18nProvider,
			{ locale },
			createElement(AgentStatusProbe, { agentPresentation }),
		),
	);
}

function createPresentation(
	status: AgentLocationPresentation['status'],
	freshness: AgentLocationPresentation['freshness'] = 'fresh',
): AgentLocationPresentation {
	return {
		activeCount: status.kind === 'working' ? 1 : 0,
		attentionCount: status.kind === 'needs-attention' ? status.count : 0,
		freshness,
		location: {
			generation: 'binding-status',
			navigable: true,
			presentationId: 'presentation-status',
			visibility: 'visible',
		},
		providerIds: [AgentProviderIdSchema.parse('claude-code')],
		sessionCount: 1,
		status,
	};
}

describe('useWorkbenchAgentStatus', () => {
	it('prepares curated English provider, activity, and tone values', () => {
		const output = renderStatus(
			'en-US',
			createPresentation({ activity: 'thinking', kind: 'working', phase: 'working' }),
		);

		expect(output).toContain('Claude Code|Thinking|active');
	});

	it('combines semantic attention and degraded health in Simplified Chinese', () => {
		const output = renderStatus(
			'zh-CN',
			createPresentation({ count: 2, kind: 'needs-attention' }, 'stale'),
		);

		expect(output).toContain('Claude Code|需要处理（2 项） · 状态已过期|degraded');
	});

	it('labels an untrusted activity hint as limited in both locales', () => {
		const presentation = createPresentation({ activityHint: 'command', kind: 'unknown' });

		expect(renderStatus('en-US', presentation)).toContain(
			'Claude Code|Activity detected · Limited status|neutral',
		);
		expect(renderStatus('zh-CN', presentation)).toContain(
			'Claude Code|检测到活动 · 状态受限|neutral',
		);
	});
});
