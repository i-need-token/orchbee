// @vitest-environment jsdom
import { ActionRuntime, type ActionScope } from '@orchbee/actions';
import type { DesktopServerConnectionBootstrap } from '@orchbee/desktop-client-protocol';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import * as React from 'react';
import { createRoot } from 'react-dom/client';
import { describe, expect, it, onTestFinished, vi } from 'vitest';

import { ApplicationLocaleController } from '../../../application-locale';
import { KeyboardShortcutSettingsController } from '../../../keyboard-shortcuts';
import { TerminalSettingsController } from '../../../terminal-settings';
import { WebApplication } from '../../../web-application';
import { inertWorkbenchAuthority } from '../../../__tests__/inertWorkbenchAuthority';
import { memoryUserSettings } from '../../../__tests__/memoryUserSettings';
import {
	createSnapshotRuntime,
	workbenchSnapshot,
} from '../../../__tests__/server-workbench-snapshot';
import { offersFileBar } from '../offersFileBar';

type Distribution = DesktopServerConnectionBootstrap['distribution'];

const FILE_BAR_ACTION_PREFIX = 'workbench.file-bar.';

/**
 * The window the Desktop composition root builds for a distribution, down to the Workbench, with
 * the storage reads and Actions it makes observable.
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
						keyboardShortcutSettingsController={keyboardShortcutSettingsController}
						layoutStorage={storage}
						projectWorkspaceRuntime={createSnapshotRuntime(
							workbenchSnapshot(1, { workspaces: [] }, []),
						)}
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
	return { actionRuntime, container, fileBarActions, storageReads };
}

describe('the Desktop window composition', () => {
	it('offers no File Bar in a release', async () => {
		const composed = await composeWindow('release');

		expect(composed.container.querySelector('[data-slot="file-bar"]')).toBeNull();
		expect(composed.fileBarActions()).toEqual([]);
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
			const actions = composed.fileBarActions();
			expect(actions.map((action) => action.descriptor.id)).toEqual(['workbench.file-bar.toggle']);
			// It starts hidden, so nothing of it renders until a person shows it.
			expect(composed.container.querySelector('[data-slot="file-bar"]')).toBeNull();

			const toggle = actions[0];
			if (toggle?.kind !== 'ready') throw new Error('Toggle File Bar takes no input.');
			await React.act(async () => {
				await composed.actionRuntime.executePrepared(toggle.invocation).completion;
			});

			const fileBar = composed.container.querySelector('aside[data-slot="file-bar"]');
			expect(fileBar?.getAttribute('aria-label')).toBe('File Bar');
			await vi.waitFor(() =>
				expect(fileBar?.querySelector('[data-slot="file-bar-toggles-row"]')).not.toBeNull(),
			);
		},
	);
});
