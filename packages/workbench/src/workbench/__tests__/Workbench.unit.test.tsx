import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { Workbench } from '../Workbench';

function renderWorkbench(
	getItem: (key: string) => string | null = () => null,
	locale: OrchbeeLocale = 'en-US',
): string {
	return renderToStaticMarkup(
		createElement(
			OrchbeeI18nProvider,
			{ locale },
			createElement(
				TooltipProvider,
				null,
				createElement(Workbench, {
					layoutStorage: { getItem, setItem: () => undefined },
					shortcutPlatform: 'apple',
				}),
			),
		),
	);
}

describe('Workbench', () => {
	it('renders the sidebar chrome while keeping the tab region empty', () => {
		const markup = renderWorkbench();

		expect(markup).toContain('data-slot="workbench-left-panel"');
		expect(markup).toContain('data-slot="workbench-right-panel"');
		expect(markup).toContain('aria-label="Hide Sidebar"');
		expect(markup).toContain('aria-label="Workbench navigation"');
		expect(markup).toContain('data-slot="workbench-sidebar-new-terminal"');
		expect(markup).toContain('data-icon="terminal"');
		expect(markup).toContain('New Terminal');
		expect(markup).toContain('data-shortcut-action="workbench.terminal.create"');
		expect(markup).toContain('⌘N');
		expect(markup).toContain('data-slot="workbench-sidebar-search"');
		expect(markup).toContain('data-icon="search"');
		expect(markup).toContain('Search');
		expect(markup).toContain('data-shortcut-action="workbench.search.open"');
		expect(markup).toContain('⌘K');
		expect(markup).toContain('Project');
		expect(markup).toContain('data-slot="workbench-sidebar-settings"');
		expect(markup).toContain('data-icon="settings"');
		expect(markup).toContain('data-shortcut-action="workbench.settings.open"');
		expect(markup).toContain('Settings');
		expect(markup).toContain('⌘,');
		expect(markup).not.toContain('role="tablist"');
	});

	it('renders the sidebar chrome in Simplified Chinese', () => {
		const markup = renderWorkbench(() => null, 'zh-CN');

		expect(markup).toContain('aria-label="工作区导航"');
		expect(markup).toContain('新建终端');
		expect(markup).toContain('搜索');
		expect(markup).toContain('项目');
		expect(markup).toContain('设置');
	});

	it('renders the visible sidebar, main panel, and localized toggle', () => {
		const markup = renderWorkbench();

		expect(markup).toContain('data-slot="workbench-left-panel"');
		expect(markup).toContain('data-slot="workbench-right-panel"');
		expect(markup).toContain('data-slot="resizable-panel-group"');
		expect(markup).toContain('data-slot="resizable-handle"');
		expect(markup).toContain('role="separator"');
		expect(markup).toContain('flex-basis:250px');
		expect(markup).toContain('aria-label="Hide Sidebar"');
		expect(markup).toContain('aria-pressed="true"');
	});

	it('keeps the toggle available when stored layout state hides the sidebar', () => {
		const markup = renderWorkbench((key) =>
			key === 'orchbee:workbench:left-panel-visibility:v1' ? '{"visible":false}' : null,
		);

		expect(markup).not.toContain('data-slot="workbench-left-panel"');
		expect(markup).not.toContain('data-slot="resizable-handle"');
		expect(markup).toContain('data-slot="workbench-right-panel"');
		expect(markup).toContain('aria-label="Show Sidebar"');
		expect(markup).toContain('aria-pressed="false"');
		expect(markup.match(/data-slot="workbench-sidebar-toggle"/gu)).toHaveLength(1);
	});
});
