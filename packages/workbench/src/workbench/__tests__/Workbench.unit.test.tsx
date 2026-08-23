import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { ActionRuntime } from '@orchbee/actions';
import { TooltipProvider } from '@orchbee/ui/components';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { terminalPresentationId } from '../../terminal-presentation';
import { Workbench } from '../Workbench';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../model/createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';

function createTestTerminalPresentationComposition(scope: string) {
	let sequence = 0;
	return createWorkbenchPreviewTerminalPresentationComposition(
		terminalPresentationId(`${scope}:initial`),
		() => terminalPresentationId(`${scope}:dynamic:${(sequence += 1)}`),
	);
}

function renderWorkbench(
	getItem: (key: string) => string | null = () => null,
	locale: OrchbeeLocale = 'en-US',
	withProject = false,
): string {
	const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Terminal 1',
		terminalPresentation: createTestTerminalPresentationComposition('workbench-render'),
	});
	projectWorkspaceRuntime.createTerminal({
		groupId: null,
		source: 'button',
		title: 'Terminal 2',
	});
	if (withProject) {
		projectWorkspaceRuntime.transition({
			directory: {
				available: true,
				git: null,
				id: 'directory-orchbee',
				kind: 'folder',
				name: 'orchbee',
				path: '/workspace/orchbee',
				source: 'selected',
			},
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: 'Primary',
			projectName: 'orchbee',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
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
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('surface'),
		});
		const markup = renderToStaticMarkup(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(
					TooltipProvider,
					null,
					createElement(Workbench, {
						actionRuntime: new ActionRuntime(),
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

		expect(markup).toContain('data-mounted-presentation="surface:initial"');
	});

	it('renders the sidebar chrome and the host-owned tab model', () => {
		const markup = renderWorkbench();

		expect(markup).toContain('data-slot="workbench-left-panel"');
		expect(markup).toContain('data-slot="workbench-right-panel"');
		expect(markup).toContain('data-slot="workbench-window-drag-region"');
		expect(markup).toContain('aria-label="Hide Sidebar"');
		expect(markup).toContain('data-slot="workbench-sidebar-toggle-position"');
		expect(markup).toContain('aria-label="Workbench navigation"');
		expect(markup).toContain('data-slot="workbench-sidebar-terminal"');
		expect(markup).toContain('data-icon="terminal"');
		expect(markup).toContain('Terminal');
		expect(markup).toContain('data-slot="workbench-tab-bar"');
		expect(markup).toContain('data-orientation="horizontal"');
		expect(markup).toContain('data-tab-strip-orientation="horizontal"');
		expect(markup).not.toContain('data-slot="workbench-tab-bar-leading-actions"');
		expect(markup).toContain('data-slot="workbench-sidebar-search"');
		expect(markup).toContain('data-icon="search"');
		expect(markup).toContain('Search');
		expect(markup).toContain('data-shortcut-action="workbench.search.open"');
		expect(markup).toContain('⇧⌘F');
		expect(markup).toContain('Project');
		expect(markup).toContain('data-slot="workbench-sidebar-project-create"');
		expect(markup).toContain('data-empty="true"');
		expect(markup).toContain('data-icon="add"');
		expect(markup).toContain('data-icon-size="section-action"');
		expect(markup).toContain('data-slot="workbench-sidebar-settings"');
		expect(markup).toContain('data-icon="settings"');
		expect(markup).toContain('data-shortcut-action="workbench.settings.open"');
		expect(markup).toContain('Settings');
		expect(markup).toContain('⌘,');
		expect(markup).toContain('role="tablist"');
		expect(markup).toContain('aria-label="Terminal tabs"');
		expect(markup).toContain('data-tab-id="terminal-context-1-tab-1"');
		expect(markup).toContain('data-tab-id="terminal-context-1-tab-2"');
		expect(markup).toContain('aria-label="system"');
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
		expect(markup).toContain('终端');
		expect(markup).toContain('搜索');
		expect(markup).toContain('项目');
		expect(markup).toContain('设置');
		expect(markup).toContain('aria-label="终端标签页"');
	});

	it('renders the expanded Project and active Workspace hierarchy', () => {
		const markup = renderWorkbench(() => null, 'en-US', true);

		expect(markup).toContain('data-slot="workbench-sidebar-project"');
		expect(markup).toContain('data-slot="workbench-sidebar-workspace"');
		expect(markup).toContain('aria-label="Collapse project: orchbee"');
		expect(markup).toContain('aria-expanded="true"');
		expect(markup).toContain('data-icon="folder"');
		expect(markup).toContain('data-icon="layout"');
		expect(markup).toContain('orchbee');
		expect(markup).toContain('Primary');
		expect(markup).not.toContain('data-empty="true"');
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
