// @vitest-environment jsdom
import { ActionRuntime, type ActionScope } from '@orchbee/actions';
import type { DesktopServerConnectionBootstrap } from '@orchbee/desktop-client-protocol';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import * as React from 'react';
import { createRoot } from 'react-dom/client';
import { describe, expect, it, onTestFinished, vi } from 'vitest';

import { ApplicationLocaleController } from '../../../application-locale';
import { MemoryFileSessionPort } from '../../../file-session';
import { KeyboardShortcutSettingsController } from '../../../keyboard-shortcuts';
import { TerminalSettingsController } from '../../../terminal-settings';
import { WebApplication } from '../../../web-application';
import { inertWorkbenchAuthority } from '../../../__tests__/inertWorkbenchAuthority';
import { memoryUserSettings } from '../../../__tests__/memoryUserSettings';
import {
	createSnapshotRuntime,
	fixtureWorkspaceId,
	snapshotContext,
	snapshotDirectory,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../../__tests__/server-workbench-snapshot';
import { offersFileBar } from '../offersFileBar';

type Distribution = DesktopServerConnectionBootstrap['distribution'];

const FILE_BAR_ACTION_PREFIX = 'workbench.file-bar.';

/**
 * How long the File Bar's views may take to show once it is first shown. The first test to show it
 * imports the views' chunk, whose modules the runner transforms on that first import: over four
 * seconds on an idle machine, and longer while a full gate loads it.
 */
const FILE_BAR_LOAD_TIMEOUT = 20_000;

/**
 * The window the Desktop composition root builds for a distribution, down to the Workbench, with
 * the storage reads, Actions and file sessions it makes observable. Workspace 1 of two is active.
 */
async function composeWindow(distribution: Distribution) {
	class TestResizeObserver {
		public disconnect(): void {}
		public observe(_target: Element): void {}
		public unobserve(_target: Element): void {}
	}
	vi.stubGlobal('ResizeObserver', TestResizeObserver);
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
	const storageReads: string[] = [];
	const storage = {
		getItem: (key: string) => {
			storageReads.push(key);
			return null;
		},
		setItem: () => undefined,
	};
	const actionRuntime = new ActionRuntime();
	const createScope = vi.spyOn(actionRuntime, 'createScope');
	const fileSessions = new MemoryFileSessionPort();
	const projectWorkspaceRuntime = createSnapshotRuntime(
		workbenchSnapshot(
			1,
			{
				directories: [snapshotDirectory(1), snapshotDirectory(2)],
				workspaces: [snapshotWorkspace(1), snapshotWorkspace(2)],
			},
			[snapshotContext(1, []), snapshotContext(2, [])],
		),
	);
	const terminalSettingsController = new TerminalSettingsController({
		themeTarget: { dataset: {} },
		userSettings: memoryUserSettings(),
	});
	const keyboardShortcutSettingsController = new KeyboardShortcutSettingsController({
		platform: 'standard',
		storage,
	});
	const applicationLocaleController = new ApplicationLocaleController({
		documentElement: { dir: '', lang: '' },
		preferredLocales: ['en-US'],
		userSettings: memoryUserSettings(),
	});
	const container = document.createElement('div');
	document.body.append(container);
	const root = createRoot(container);
	onTestFinished(async () => {
		await React.act(async () => root.unmount());
		container.remove();
		actionRuntime.dispose();
		terminalSettingsController.dispose();
		keyboardShortcutSettingsController.dispose();
		applicationLocaleController.dispose();
		vi.unstubAllGlobals();
		Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
			configurable: true,
			value: false,
		});
	});
	await React.act(async () =>
		root.render(
			<OrchbeeI18nProvider locale='en-US'>
				<TooltipProvider>
					<WebApplication
						actionRuntime={actionRuntime}
						applicationLocaleController={applicationLocaleController}
						authority={inertWorkbenchAuthority()}
						fileBarOffered={offersFileBar(distribution)}
						fileSessionPort={fileSessions}
						keyboardShortcutSettingsController={keyboardShortcutSettingsController}
						layoutStorage={storage}
						projectWorkspaceRuntime={projectWorkspaceRuntime}
						shortcutPlatform='standard'
						terminalSettingsController={terminalSettingsController}
					/>
				</TooltipProvider>
			</OrchbeeI18nProvider>,
		),
	);
	const windowScope = (): ActionScope => {
		for (const result of createScope.mock.results) {
			if (result.type === 'return' && result.value.parent === null) return result.value;
		}
		throw new Error('The window created no Action scope.');
	};
	// Every Action the window's surfaces could list, in its own scope and every one beneath it.
	const fileBarActions = () =>
		actionRuntime
			.discover({ scope: windowScope(), source: 'command-surface' }, { includeDescendants: true })
			.filter((result) => result.descriptor.id.startsWith(FILE_BAR_ACTION_PREFIX));
	const toggleFileBar = async (): Promise<void> => {
		const toggle = fileBarActions().find(
			(action) => action.descriptor.id === 'workbench.file-bar.toggle',
		);
		if (toggle?.kind !== 'ready') throw new Error('Toggle File Bar takes no input.');
		await React.act(async () => {
			await actionRuntime.executePrepared(toggle.invocation).completion;
		});
	};
	/** Opens Settings and returns the categories its navigation lists. */
	const openSettings = async (): Promise<string[]> => {
		await React.act(async () => {
			terminalSettingsController.open();
		});
		return [...container.querySelectorAll<HTMLElement>('[data-settings-category]')].map(
			(category) => category.dataset['settingsCategory'] ?? '',
		);
	};
	return {
		container,
		fileBarActions,
		fileSessions,
		openSettings,
		projectWorkspaceRuntime,
		storageReads,
		toggleFileBar,
	};
}

// Each test renders the whole window, and one of them waits for the File Bar's chunk as well.
describe('the Desktop window composition', { timeout: FILE_BAR_LOAD_TIMEOUT + 10_000 }, () => {
	it('offers no File Bar in a release', async () => {
		const composed = await composeWindow('release');
		const settingsCategories = await composed.openSettings();

		expect(settingsCategories).toContain('general');
		expect(settingsCategories).not.toContain('file-bar');
		expect(composed.container.querySelector('[data-slot="file-bar"]')).toBeNull();
		expect(composed.fileBarActions()).toEqual([]);
		expect(composed.fileSessions.openWorkspaces()).toEqual([]);
		expect(
			composed.storageReads.filter(
				(key) => key.startsWith('orchbee:file-bar:') || key.startsWith('orchbee:file-overlay:'),
			),
		).toEqual([]);
	});

	it.each(['development', 'test'] as const)(
		'offers the File Bar in a %s build and loads its views when it first shows',
		async (distribution) => {
			const composed = await composeWindow(distribution);
			expect(composed.fileBarActions().map((action) => action.descriptor.id)).toContain(
				'workbench.file-bar.toggle',
			);
			// It starts hidden, so nothing of it renders and no file session starts until a person
			// shows it.
			expect(composed.container.querySelector('[data-slot="file-bar"]')).toBeNull();
			expect(composed.fileSessions.openWorkspaces()).toEqual([]);

			await composed.toggleFileBar();

			const fileBar = composed.container.querySelector('aside[data-slot="file-bar"]');
			expect(fileBar?.getAttribute('aria-label')).toBe('File Bar');
			await vi.waitFor(
				() => expect(fileBar?.querySelector('[data-slot="file-bar-toggles-row"]')).not.toBeNull(),
				{ timeout: FILE_BAR_LOAD_TIMEOUT },
			);
			expect(await composed.openSettings()).toContain('file-bar');
		},
	);

	it('holds the file session of the active Workspace once the File Bar has shown it, hidden or not', async () => {
		const composed = await composeWindow('development');
		await composed.toggleFileBar();
		expect(composed.fileSessions.openWorkspaces()).toEqual([fixtureWorkspaceId(1)]);

		// Hidden, the File Bar keeps the session for the count badge on its button.
		await composed.toggleFileBar();
		expect(composed.fileSessions.openWorkspaces()).toEqual([fixtureWorkspaceId(1)]);

		// A Workspace it has not shown in this run gets no session, and so no Git read, until it shows.
		await React.act(async () => {
			composed.projectWorkspaceRuntime.transition({
				type: 'workspace-activated',
				workspaceId: fixtureWorkspaceId(2),
			});
		});
		expect(composed.fileSessions.openWorkspaces()).toEqual([]);

		await composed.toggleFileBar();
		expect(composed.fileSessions.openWorkspaces()).toEqual([fixtureWorkspaceId(2)]);
	});
});
