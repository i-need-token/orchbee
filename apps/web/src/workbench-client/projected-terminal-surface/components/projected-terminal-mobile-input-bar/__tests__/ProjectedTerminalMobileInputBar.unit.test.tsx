import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { ProjectedTerminalMobileInputBar } from '../ProjectedTerminalMobileInputBar';

describe('ProjectedTerminalMobileInputBar', () => {
	it('renders the complete ready key row and reflects actual focus and armed state', () => {
		const markup = renderBar('en-US', {
			alt: false,
			control: true,
			focused: true,
			readiness: { kind: 'ready', revision: 'attachment:4' },
		});

		expect(markup).toContain('role="toolbar"');
		expect(markup).toContain('aria-label="Terminal mobile controls"');
		expect(markup).toContain('aria-label="Hide keyboard"');
		expect(markup).toContain('data-mobile-terminal-modifier="control"');
		expect(markup).toContain('aria-pressed="true"');
		for (const key of ['Escape', 'Tab', 'ArrowLeft', 'ArrowUp', 'ArrowDown', 'ArrowRight']) {
			expect(markup).toContain(`data-mobile-terminal-key="${key}"`);
		}
		expect(markup).not.toContain('disabled=""');
	});

	it('localizes labels and disables input-producing controls while unavailable', () => {
		const markup = renderBar('zh-CN', {
			alt: false,
			control: false,
			focused: false,
			readiness: { kind: 'unavailable' },
		});

		expect(markup).toContain('aria-label="终端移动端控制"');
		expect(markup).toContain('aria-label="显示键盘"');
		expect(markup).toContain('aria-label="向左箭头"');
		expect(markup.match(/\sdisabled=""/g)?.length).toBe(9);
	});
});

function renderBar(
	locale: OrchbeeLocale,
	state: Parameters<typeof ProjectedTerminalMobileInputBar>[0]['state'],
): string {
	return renderToStaticMarkup(
		createElement(
			OrchbeeI18nProvider,
			{ locale },
			createElement(ProjectedTerminalMobileInputBar, {
				onKey: () => undefined,
				onKeyboardToggle: () => undefined,
				onModifierToggle: () => undefined,
				state,
			}),
		),
	);
}
