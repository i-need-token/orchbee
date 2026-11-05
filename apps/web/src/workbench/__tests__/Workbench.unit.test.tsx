import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { ActionRuntime } from '@orchbee/actions';
import { TooltipProvider } from '@orchbee/ui/components';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import {
	createSnapshotRuntime,
	fixtureDirectoryId,
	fixturePresentationId,
	fixtureProjectId,
	fixtureTabId,
	fixtureWorkspaceId,
	oneWorkspaceSnapshot,
	snapshotContext,
	snapshotDirectory,
	snapshotProject,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../__tests__/server-workbench-snapshot';
import { Workbench } from '../Workbench';
import { inertWorkbenchAuthority } from '../../__tests__/inertWorkbenchAuthority';

function renderWorkbench(
	getItem: (key: string) => string | null = () => null,
	locale: OrchbeeLocale = 'en-US',
	withProject = false,
	collapseProject = false,
	activateRecentWorkspace = false,
): string {
	const tabs = [snapshotTab(1), snapshotTab(2)];
	// A Workspace outside the Project, which Recents holds.
	const recentWorkspaces = activateRecentWorkspace
		? [snapshotWorkspace(2, { primaryDirectoryId: fixtureDirectoryId(1) })]
		: [];
	const recentContexts = activateRecentWorkspace ? [snapshotContext(2, [snapshotTab(3)])] : [];
	const projectWorkspaceRuntime = createSnapshotRuntime(
		withProject
			? workbenchSnapshot(
					1,
					{
						directories: [snapshotDirectory(1)],
						projects: [snapshotProject(1, 'orchbee')],
						workspaces: [
							snapshotWorkspace(1, { label: 'Primary', projectId: fixtureProjectId(1) }),
							...recentWorkspaces,
						],
					},
					[snapshotContext(1, tabs), ...recentContexts],
				)
			: oneWorkspaceSnapshot(1, tabs),
	);
	if (collapseProject) {
		projectWorkspaceRuntime.transition({
			projectId: fixtureProjectId(1),
			type: 'project-expansion-toggled',
		});
	}
	if (activateRecentWorkspace) {
		projectWorkspaceRuntime.transition({
			type: 'workspace-activated',
			workspaceId: fixtureWorkspaceId(2),
		});
	}
	return renderToStaticMarkup(
		createElement(
			OrchbeeI18nProvider,
			{ locale },
			createElement(
				TooltipProvider,
				null,
				createElement(Workbench, {
					actionRuntime: new ActionRuntime(),
					authority: inertWorkbenchAuthority(),
					layoutStorage: { getItem, setItem: () => undefined },
					projectWorkspaceRuntime,
					shortcutPlatform: 'apple',
				}),
			),
		),
	);
}

describe('Workbench', () => {
	it('mounts an application-owned terminal surface only inside the displayed viewport', () => {
		const projectWorkspaceRuntime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1)]),
		);
		const markup = renderToStaticMarkup(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(
					TooltipProvider,
					null,
					createElement(Workbench, {
						actionRuntime: new ActionRuntime(),
						authority: inertWorkbenchAuthority(),
						projectWorkspaceRuntime,
						shortcutPlatform: 'apple',
						terminalSurface: ({ presentation }) =>
							createElement('div', {
								'data-mounted-presentation': presentation.presentationId,
							}),
					}),
				),
			),
		);

		expect(markup).toContain(`data-mounted-presentation="${fixturePresentationId(1)}"`);
	});

	it('renders the sidebar chrome and the host-owned tab model', () => {
		const markup = renderWorkbench();

		expect(markup).toContain('data-slot="workbench-left-panel"');
		expect(markup).toContain('data-slot="workbench-right-panel"');
		expect(markup).toContain('data-slot="workbench-window-drag-region"');
		expect(markup).toContain('aria-label="Hide Sidebar"');
		expect(markup).toContain('data-slot="workbench-sidebar-toggle-position"');
		expect(markup).toContain('aria-label="Workbench navigation"');
		expect(markup).toContain('data-slot="workbench-sidebar-new-workspace"');
		expect(markup).toContain('New workspace');
		expect(markup).toContain('data-slot="workbench-tab-bar"');
		expect(markup).toContain('data-orientation="horizontal"');
		expect(markup).toContain('data-tab-strip-orientation="horizontal"');
		expect(markup).not.toContain('data-slot="workbench-tab-bar-leading-actions"');
		expect(markup).toContain('data-slot="workbench-sidebar-search"');
		expect(markup).toContain('data-icon="search"');
		expect(markup).toContain('Search');
		expect(markup).toContain('data-shortcut-action="workbench.search.open"');
		expect(markup).toContain('⇧⌘F');
		expect(markup).toContain('data-slot="workbench-sidebar-settings"');
		expect(markup).toContain('data-icon="settings"');
		expect(markup).toContain('data-shortcut-action="workbench.settings.open"');
		expect(markup).toContain('Settings');
		expect(markup).toContain('⌘,');
		expect(markup).toContain('role="tablist"');
		expect(markup).toContain('aria-label="Terminal tabs"');
		expect(markup).toContain(`data-tab-id="${fixtureTabId(1)}"`);
		expect(markup).toContain(`data-tab-id="${fixtureTabId(2)}"`);
		expect(markup).toContain('aria-label="Tab 1"');
		expect(markup).toContain('data-slot="workbench-tab-content"');
		expect(markup).toContain('data-slot="workbench-panel-workspace"');
		expect(markup).toContain('data-slot="workbench-panel"');
		expect(markup).toContain('data-slot="workbench-terminal-viewport"');
		expect(markup).toContain('data-terminal-role="primary"');
		expect(markup).not.toContain('data-slot="workbench-panel-header"');
		expect(markup).not.toContain('data-slot="workbench-child-terminal-strip"');
	});

	it('renders the sidebar chrome in Simplified Chinese', () => {
		const markup = renderWorkbench(() => null, 'zh-CN');

		expect(markup).toContain('aria-label="工作区导航"');
		expect(markup).toContain('新建工作区');
		expect(markup).toContain('搜索');
		expect(markup).toContain('设置');
		expect(markup).toContain('aria-label="终端标签页"');
	});

	it('renders the expanded Project and active Workspace hierarchy', () => {
		const markup = renderWorkbench(() => null, 'en-US', true);

		expect(markup).toContain('data-slot="workbench-sidebar-project"');
		expect(markup).toContain('data-slot="workbench-sidebar-workspace"');
		expect(markup).toContain('aria-label="Collapse project: orchbee"');
		expect(markup).toContain('aria-expanded="true"');
		expect(markup).not.toMatch(
			/<button(?=[^>]*data-slot="workbench-sidebar-project")(?=[^>]*aria-current=)[^>]*>/,
		);
		expect(markup).toMatch(
			/<button(?=[^>]*data-slot="workbench-sidebar-workspace")(?=[^>]*aria-current="page")[^>]*>/,
		);
		expect(markup).toContain('orchbee');
		expect(markup).toContain('Primary');
		expect(markup).not.toContain('data-empty="true"');
	});

	it('marks the folded Project hiding the active Workspace for sidebar focus, never as current', () => {
		const activeMarkup = renderWorkbench(() => null, 'en-US', true, true);
		const recentMarkup = renderWorkbench(() => null, 'en-US', true, true, true);

		expect(activeMarkup).toContain('aria-label="Expand project: orchbee"');
		expect(activeMarkup).toMatch(
			/<button(?=[^>]*data-slot="workbench-sidebar-project")(?=[^>]*data-holds-active-workspace="true")(?![^>]*aria-current=)[^>]*>/,
		);
		expect(activeMarkup).not.toContain('data-slot="workbench-sidebar-workspace"');
		expect(recentMarkup).not.toContain('data-holds-active-workspace');
	});

	it('restores the vertical tab-strip layout from host-provided storage', () => {
		const markup = renderWorkbench((key) =>
			key === 'orchbee:workbench:tab-strip-orientation:v1' ? '{"orientation":"vertical"}' : null,
		);

		expect(markup).toContain('data-orientation="vertical"');
		expect(markup).toContain('data-tab-strip-orientation="vertical"');
		expect(markup).toContain('aria-orientation="vertical"');
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
		expect(markup.match(/data-slot="workbench-sidebar-toggle-position"/gu)).toHaveLength(1);
		expect(markup).toContain('data-slot="workbench-tab-bar-leading-actions"');
		expect(markup.indexOf('role="tablist"')).toBeLessThan(
			markup.indexOf('data-slot="workbench-tab-bar-leading-actions"'),
		);
	});
});
