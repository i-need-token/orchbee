import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { Workbench } from '../Workbench';
import type { WorkbenchTab } from '../types/workbench-tab';

function renderWorkbench(
	getItem: (key: string) => string | null = () => null,
	tabs: readonly WorkbenchTab[] = [],
	activeTabId?: string,
	isTabManagementEnabled = false,
	locale: OrchbeeLocale = 'en-US',
	isGroupCollapsed = false,
): string {
	return renderToStaticMarkup(
		createElement(
			OrchbeeI18nProvider,
			{ locale },
			createElement(
				TooltipProvider,
				null,
				createElement(Workbench, {
					activeTabId,
					layoutStorage: { getItem, setItem: () => undefined },
					onTabClose: isTabManagementEnabled ? () => undefined : undefined,
					onTabColorChange: isTabManagementEnabled ? () => undefined : undefined,
					onTabCreate: isTabManagementEnabled ? () => undefined : undefined,
					onTabGroupAssign: isTabManagementEnabled ? () => undefined : undefined,
					onTabGroupChange: isTabManagementEnabled ? () => undefined : undefined,
					onTabGroupClose: isTabManagementEnabled ? () => undefined : undefined,
					onTabGroupCollapseChange: isTabManagementEnabled ? () => undefined : undefined,
					onTabGroupCreate: isTabManagementEnabled ? () => undefined : undefined,
					onTabGroupDelete: isTabManagementEnabled ? () => undefined : undefined,
					onTabGroupUngroup: isTabManagementEnabled ? () => undefined : undefined,
					onTabMove: isTabManagementEnabled ? () => undefined : undefined,
					onTabPinChange: isTabManagementEnabled ? () => undefined : undefined,
					onTabRename: isTabManagementEnabled ? () => undefined : undefined,
					onTabsClose: isTabManagementEnabled ? () => undefined : undefined,
					onSearch: isTabManagementEnabled ? () => undefined : undefined,
					onSettings: isTabManagementEnabled ? () => undefined : undefined,
					shortcutPlatform: 'apple',
					tabGroups: isTabManagementEnabled
						? [{ color: 'orange', id: 'group-a', isCollapsed: isGroupCollapsed, label: 'Research' }]
						: undefined,
					tabs,
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
		const markup = renderWorkbench(() => null, [], undefined, false, 'zh-CN');

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

	it('renders prepared tabs with one active tab and localized tab-list semantics', () => {
		const markup = renderWorkbench(
			() => null,
			[
				{ groupId: 'group-a', icon: 'terminal', id: 'second', label: 'Second workspace' },
				{ color: 'blue', id: 'first', isPinned: true, label: 'First workspace' },
				{ groupId: 'group-a', id: 'third', label: 'Third workspace' },
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
		expect(markup.match(/data-workbench-close-tab-id=/gu)).toHaveLength(2);
		expect(markup).toContain('data-workbench-unpin-tab-id="first"');
		expect(markup).toContain('aria-label="Unpin Tab"');
		expect(markup.match(/data-slot="context-menu-trigger"/gu)).toHaveLength(4);
		expect(markup).toContain('data-workbench-tab-color="blue"');
		expect(markup).toContain('data-workbench-tab-pinned="true"');
		expect(markup).toContain('data-workbench-tab-group-id="group-a"');
		expect(markup).toContain('data-workbench-tab-icon="terminal"');
		expect(markup).toContain('aria-label="Collapse Research"');
		expect(markup).toContain('aria-expanded="true"');
		expect(markup).toContain('data-workbench-tab-group-expanded="true"');
		expect(markup).toContain('data-icon="pin"');
		expect(markup).not.toContain('aria-label="Close First workspace"');
		expect(markup.indexOf('data-workbench-tab-item-id="first"')).toBeLessThan(
			markup.indexOf('data-workbench-tab-item-id="second"'),
		);
		expect(markup.match(/data-workbench-tab-reorderable="true"/gu)).toHaveLength(3);
		expect(markup.match(/data-workbench-tab-visual-item="tab"/gu)).toHaveLength(3);
		expect(markup.match(/data-workbench-tab-group-color="orange"/gu)).toHaveLength(2);
		expect(markup).not.toContain('data-workbench-tab-group-dragging');
		expect(markup).toContain('data-workbench-tab-visual-item="group"');
		expect(markup).toContain('data-workbench-tab-drag-overlay="true"');
		expect(markup).not.toContain('data-slot="tabs-divider"');
		expect(markup).not.toContain('data-edge=');
		expect(markup).toContain('aria-label="Close Second workspace"');
		expect(markup).toContain('aria-label="New Tab"');
	});

	it('collapses grouped tab triggers behind the colored group label', () => {
		const markup = renderWorkbench(
			() => null,
			[
				{ groupId: 'group-a', id: 'first', label: 'First workspace' },
				{ groupId: 'group-a', id: 'second', label: 'Second workspace' },
				{ id: 'third', label: 'Third workspace' },
			],
			'third',
			true,
			'en-US',
			true,
		);

		expect(markup.match(/role="tab"/gu)).toHaveLength(1);
		expect(markup).toContain('aria-label="Expand Research"');
		expect(markup).toContain('aria-expanded="false"');
		expect(markup).not.toContain('data-workbench-tab-group-expanded');
		expect(markup).toContain('data-color="orange"');
		expect(markup).not.toContain('data-workbench-tab-item-id="first"');
		expect(markup).not.toContain('data-workbench-tab-item-id="second"');
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
