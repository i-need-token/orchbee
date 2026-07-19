import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { Workbench } from '../Workbench';

function renderWorkbench(
	getItem: (key: string) => string | null = () => null,
	tabs: readonly { readonly id: string; readonly label: string }[] = [],
	activeTabId?: string,
	isTabManagementEnabled = false,
): string {
	return renderToStaticMarkup(
		createElement(
			OrchbeeI18nProvider,
			{ locale: 'en-US' },
			createElement(
				TooltipProvider,
				null,
				createElement(Workbench, {
					activeTabId,
					layoutStorage: { getItem, setItem: () => undefined },
					onTabClose: isTabManagementEnabled ? () => undefined : undefined,
					onTabCreate: isTabManagementEnabled ? () => undefined : undefined,
					onTabMove: isTabManagementEnabled ? () => undefined : undefined,
					shortcutPlatform: 'apple',
					tabs,
				}),
			),
		),
	);
}

describe('Workbench', () => {
	it('keeps both panels empty when no prepared tabs are provided', () => {
		const markup = renderWorkbench();

		expect(markup).toContain('data-slot="workbench-left-panel"');
		expect(markup).toContain('data-slot="workbench-right-panel"');
		expect(markup).toContain('aria-label="Hide Sidebar"');
		expect(markup).not.toContain('Orchbee');
		expect(markup).not.toContain('terminal');
		expect(markup).not.toContain('role="tablist"');
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

	it('renders prepared tabs with one active tab and localized tab-list semantics', () => {
		const markup = renderWorkbench(
			() => null,
			[
				{ id: 'first', label: 'First workspace' },
				{ id: 'second', label: 'Second workspace' },
				{ id: 'third', label: 'Third workspace' },
			],
			'second',
			true,
		);

		expect(markup).toContain('role="tablist"');
		expect(markup).toContain('aria-label="Workbench tabs"');
		expect(markup.match(/role="tab"/gu)).toHaveLength(3);
		expect(markup).toContain('aria-selected="true"');
		expect(markup).toContain('Second workspace');
		expect(markup).toContain('data-workbench-tab-id="second"');
		expect(markup.match(/data-workbench-close-tab-id=/gu)).toHaveLength(3);
		expect(markup.match(/draggable="true"/gu)).toHaveLength(3);
		expect(markup.match(/data-slot="tabs-divider"/gu)).toHaveLength(4);
		expect(markup).toContain('data-edge="start"');
		expect(markup).not.toContain('data-edge="end"');
		expect(markup).toContain('aria-label="Close Second workspace"');
		expect(markup).toContain('aria-label="New Tab"');
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
		expect(markup).toContain('data-slot="workbench-sidebar-toggle-space"');
	});
});
