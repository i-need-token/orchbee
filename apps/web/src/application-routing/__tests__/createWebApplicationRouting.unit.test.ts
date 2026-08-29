// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import {
	createWorkbenchPreviewTerminalPresentationComposition,
	terminalPresentationId,
	WorkbenchProjectWorkspaceRuntime,
} from '@orchbee/workbench';
import { createMemoryHistory, RouterProvider } from '@tanstack/react-router';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

import { TerminalSettingsController } from '../../terminal-settings';
import { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import { createWebApplicationRouting } from '../createWebApplicationRouting';

const originalScrollTo = window.scrollTo;

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
	Object.defineProperty(window, 'scrollTo', {
		configurable: true,
		value: () => undefined,
	});
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
	Object.defineProperty(window, 'scrollTo', {
		configurable: true,
		value: originalScrollTo,
	});
});

function createRuntime(): WorkbenchProjectWorkspaceRuntime {
	let presentationSequence = 0;
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Standalone',
		terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
			terminalPresentationId('router-test:initial'),
			() => terminalPresentationId(`router-test:dynamic:${(presentationSequence += 1)}`),
		),
	});
	runtime.transition({
		directory: {
			available: true,
			git: null,
			id: 'directory-1',
			kind: 'folder',
			name: 'orchbee',
			path: '/workspace/orchbee',
			source: 'selected',
		},
		launchConfigurations: [],
		primaryWorkspaceLaunchConfigurationId: null,
		primaryWorkspaceName: 'Primary',
		projectName: 'Orchbee',
		terminalTitle: 'Workspace',
		type: 'project-created',
	});
	return runtime;
}

function createRouting(path: string, runtime = createRuntime()) {
	const actionRuntime = new ActionRuntime();
	const storage = { getItem: () => null, setItem: () => undefined };
	const keyboardShortcutSettingsController = new KeyboardShortcutSettingsController({
		platform: 'standard',
		storage,
	});
	const terminalSettingsController = new TerminalSettingsController({
		storage,
		themeTarget: document.documentElement,
	});
	const routing = createWebApplicationRouting(
		{
			actionRuntime,
			initialTerminalTitle: 'Terminal',
			keyboardShortcutSettingsController,
			projectWorkspaceRuntime: runtime,
			shortcutPlatform: 'standard',
			terminalSettingsController,
		},
		createMemoryHistory({ initialEntries: [path] }),
	);
	return {
		dispose: () => {
			routing.dispose();
			actionRuntime.dispose();
			keyboardShortcutSettingsController.dispose();
			terminalSettingsController.dispose();
		},
		routing,
		runtime,
	};
}

describe('createWebApplicationRouting', () => {
	it('uses Browser-style history and replaces a partial Project route with its first descendants', async () => {
		const fixture = createRouting('/projects');
		await fixture.routing.router.load();

		expect(fixture.routing.router.history.location.pathname).toBe(
			'/projects/project-1/workspaces/workspace-1/tabs/terminal-context-2-tab-1',
		);
		expect(fixture.runtime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',
			projectId: 'project-1',
			workspaceId: 'workspace-1',
		});
		fixture.dispose();
	});

	it('creates one terminal before rendering an initially empty route and preserves a later empty state', async () => {
		const runtime = createRuntime();
		runtime.requestActiveTerminalTabsClose(['terminal-context-2-tab-1']);
		const fixture = createRouting('/', runtime);

		await fixture.routing.router.load();
		expect(fixture.routing.router.history.location.pathname).toBe(
			'/projects/project-1/workspaces/workspace-1/tabs/terminal-context-2-tab-2',
		);
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toHaveLength(1);

		runtime.requestActiveTerminalTabsClose(['terminal-context-2-tab-2']);
		await fixture.routing.router.load();
		expect(fixture.routing.router.history.location.pathname).toBe(
			'/projects/project-1/workspaces/workspace-1',
		);
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toEqual([]);
		fixture.dispose();
	});

	it('decodes an encoded route parameter exactly once', async () => {
		const runtime = createRuntime();
		runtime
			.getActiveTerminalContext()
			.tabModel.addTab({ group: null, pinned: false, title: 'Encoded' }, { id: 'tab/encoded' });
		const fixture = createRouting(
			'/projects/project-1/workspaces/workspace-1/tabs/tab%2Fencoded',
			runtime,
		);
		await fixture.routing.router.load();

		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().activeTabId).toBe(
			'tab/encoded',
		);
		expect(fixture.routing.router.history.location.pathname).toBe(
			'/projects/project-1/workspaces/workspace-1/tabs/tab%2Fencoded',
		);
		fixture.dispose();
	});

	it('keeps an explicit stale route unavailable instead of selecting another entity', async () => {
		const fixture = createRouting('/projects/project-1/workspaces/workspace-1/tabs/missing-tab');
		const container = document.createElement('div');
		const root = createRoot(container);
		await act(async () => {
			root.render(
				createElement(
					OrchbeeI18nProvider,
					{ locale: 'en-US' },
					createElement(RouterProvider, { router: fixture.routing.router }),
				),
			);
		});
		expect(fixture.routing.router.history.location.pathname).toBe(
			'/projects/project-1/workspaces/workspace-1/tabs/missing-tab',
		);
		expect(container.textContent).toContain('This page isn’t available');
		await act(async () => root.unmount());
		fixture.dispose();
	});

	it('applies Back navigation without appending another history entry', async () => {
		const fixture = createRouting('/terminal/tabs/terminal-context-1-tab-1');
		await fixture.routing.router.load();
		const initialLength = fixture.routing.router.history.length;

		fixture.routing.coordinator.navigateTo({
			kind: 'workspace',
			projectId: 'project-1',
			workspaceId: 'workspace-1',
		});
		await fixture.routing.router.load();
		expect(fixture.runtime.getSnapshot().activeLocation).toMatchObject({ kind: 'workspace' });
		expect(fixture.routing.router.history.length).toBe(initialLength + 1);

		fixture.routing.router.history.back();
		await fixture.routing.router.load();
		expect(fixture.runtime.getSnapshot().activeLocation).toEqual({ kind: 'standalone' });
		expect(fixture.routing.router.history.length).toBe(initialLength + 1);
		fixture.dispose();
	});
});
