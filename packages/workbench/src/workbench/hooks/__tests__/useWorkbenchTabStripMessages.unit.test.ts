import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { createElement, type ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { useWorkbenchTabStripMessages } from '../useWorkbenchTabStripMessages';

function GroupMessagesProbe({ title }: { readonly title: string }): ReactNode {
	const messages = useWorkbenchTabStripMessages();
	const group = { id: 'group-1', title: '', color: 'blue' as const, collapsed: false };
	const tabs = [{ title }];
	return `${messages.formatGroupTooltip(group, tabs)}|${messages.formatGroupAccessibleName(group, tabs)}`;
}

describe('useWorkbenchTabStripMessages', () => {
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
