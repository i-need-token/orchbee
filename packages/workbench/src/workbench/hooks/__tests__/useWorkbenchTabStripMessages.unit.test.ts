import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { createElement, type ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { useWorkbenchTabStripMessages } from '../useWorkbenchTabStripMessages';

function MessagesProbe({ locale }: { readonly locale: 'en-US' | 'zh-CN' }): ReactNode {
	const messages = useWorkbenchTabStripMessages();
	const tabName = messages.formatTabAccessibleLabel(
		{
			id: 'terminal-1',
			title: locale === 'en-US' ? 'Terminal 1' : '终端 1',
			pinned: false,
			group: null,
		},
		null,
	);
	return `${messages.tabStripLabel}|${messages.newTabToRight}|${messages.openVerticalTabs}|${messages.closeVerticalTabs}|${messages.tabOptions}|${messages.groupColorNames.purple}|${messages.formatClosedTabsAnnouncement(3)}|${tabName}`;
}

function renderMessages(locale: 'en-US' | 'zh-CN'): string {
	return renderToStaticMarkup(
		createElement(OrchbeeI18nProvider, { locale }, createElement(MessagesProbe, { locale })),
	);
}

function GroupMessagesProbe({ title }: { readonly title: string }): ReactNode {
	const messages = useWorkbenchTabStripMessages();
	const group = { id: 'group-1', title: '', color: 'blue' as const, collapsed: false };
	const tabs = [{ title }];
	return `${messages.formatGroupTooltip(group, tabs)}|${messages.formatGroupAccessibleName(group, tabs)}`;
}

describe('useWorkbenchTabStripMessages', () => {
	it('formats the complete English tab interaction surface', () => {
		const output = renderMessages('en-US');

		expect(output).toContain(
			'Terminal tabs|New tab to the right|Open vertical tabs|Close vertical tabs',
		);
		expect(output).toContain('Closed 3 tabs|Terminal 1');
	});

	it('formats menus and accessibility output in Simplified Chinese', () => {
		const output = renderMessages('zh-CN');

		expect(output).toContain('打开垂直标签页|关闭垂直标签页|标签页选项|紫色');
		expect(output).toContain('终端 1');
	});

	describe.each(['en-US', 'zh-CN'] as const)('%s group titles', (locale) => {
		it.each(['👩‍💻', 'e\u0301', '🇨🇳'])(
			'preserves the complete %s grapheme at tooltip and accessibility limits',
			(grapheme) => {
				for (const prefixLength of [28, 45]) {
					const prefix = 'x'.repeat(prefixLength);
					const output = renderToStaticMarkup(
						createElement(
							OrchbeeI18nProvider,
							{ locale },
							createElement(GroupMessagesProbe, { title: `${prefix}${grapheme}tail` }),
						),
					);
					expect(output).toContain(`${prefix}${grapheme}…`);
				}
			},
		);
	});
});
