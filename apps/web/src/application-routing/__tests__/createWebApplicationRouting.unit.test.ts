// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { WorkbenchProjectWorkspaceRuntime } from '../../workbench';
import { createMemoryHistory, RouterProvider } from '@tanstack/react-router';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import {
	createSnapshotRuntime,
	fixtureProjectId,
	fixtureTabId,
	fixtureWorkspaceId,
	snapshotContext,
	snapshotDirectory,
	snapshotProject,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../__tests__/server-workbench-snapshot';
import { TerminalSettingsController } from '../../terminal-settings';
import { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import { ApplicationLocaleController } from '../../application-locale';
import { createWebApplicationRouting } from '../createWebApplicationRouting';
import { inertWorkbenchAuthority } from '../../__tests__/inertWorkbenchAuthority';

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

const WORKSPACE_1 = fixtureWorkspaceId(1);
const TAB_1 = fixtureTabId(1);

/** Project 1 holds Workspace 1, which holds Tab 1. */
function createRuntime(): WorkbenchProjectWorkspaceRuntime {
	return createSnapshotRuntime(
		workbenchSnapshot(
			1,
			{
				directories: [snapshotDirectory(1)],
				projects: [snapshotProject(1)],
				workspaces: [snapshotWorkspace(1, { projectId: fixtureProjectId(1) })],
			},
			[snapshotContext(1, [snapshotTab(1)])],
		),
	);
}

const inertAuthority = inertWorkbenchAuthority();

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
	const applicationLocaleController = new ApplicationLocaleController({
		documentElement: document.documentElement,
		preferredLocales: ['en-US'],
		storage,
	});
	const routing = createWebApplicationRouting(
		{
			actionRuntime,
			applicationLocaleController,
			authority: inertAuthority,
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
			applicationLocaleController.dispose();
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
			`/workspaces/${WORKSPACE_1}/tabs/${TAB_1}`,
		);
		expect(fixture.runtime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',
			workspaceId: WORKSPACE_1,
		});
		fixture.dispose();
	});

	it('keeps an explicit stale route unavailable instead of selecting another entity', async () => {
		const fixture = createRouting(`/workspaces/${WORKSPACE_1}/tabs/missing-tab`);
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
			`/workspaces/${WORKSPACE_1}/tabs/missing-tab`,
		);
		expect(container.textContent).toContain('This page isn’t available');
		await act(async () => root.unmount());
		fixture.dispose();
	});

	it('applies Back navigation without appending another history entry', async () => {
		const fixture = createRouting('/terminal');
		await fixture.routing.router.load();
		const initialLength = fixture.routing.router.history.length;

		fixture.routing.coordinator.navigation.navigate(
			{
				executionId: 'back-navigation',
				markCommitted: vi.fn<() => void>(),
				reportProgress: vi.fn<() => void>(),
				scope: { id: 'test', parent: null },
				signal: new AbortController().signal,
				source: 'button',
			},
			{ kind: 'workspace', workspaceId: WORKSPACE_1 },
		);
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
