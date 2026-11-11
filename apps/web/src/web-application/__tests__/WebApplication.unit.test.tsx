// @vitest-environment jsdom
import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import * as React from 'react';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, onTestFinished, vi } from 'vitest';

import { TerminalSettingsController } from '../../terminal-settings';
import { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import { ApplicationLocaleController } from '../../application-locale';
import {
	createSnapshotRuntime,
	workbenchSnapshot,
} from '../../__tests__/server-workbench-snapshot';
import { WebApplication } from '../WebApplication';
import { inertWorkbenchAuthority } from '../../__tests__/inertWorkbenchAuthority';
import { memoryUserSettings } from '../../__tests__/memoryUserSettings';

describe('WebApplication', () => {
	it('renders the Workbench before the Server holds any Workspace', () => {
		const actionRuntime = new ActionRuntime();
		const projectWorkspaceRuntime = createSnapshotRuntime(
			workbenchSnapshot(1, { workspaces: [] }, []),
		);
		const terminalSettingsController = new TerminalSettingsController({
			themeTarget: { dataset: {} },
			userSettings: memoryUserSettings(),
		});
		const keyboardShortcutSettingsController = new KeyboardShortcutSettingsController({
			platform: 'apple',
			storage: { getItem: () => null, setItem: () => undefined },
		});
		const markup = renderToStaticMarkup(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(
					TooltipProvider,
					undefined,
					createElement(WebApplication, {
						actionRuntime,
						applicationLocaleController: createApplicationLocaleController(),
						authority: inertAuthority,
						keyboardShortcutSettingsController,
						layoutStorage: {
							getItem: () => '{"width":320}',
							setItem: () => undefined,
						},
						projectWorkspaceRuntime,
						shortcutPlatform: 'apple',
						terminalSettingsController,
					}),
				),
			),
		);

		expect(markup).toContain('data-slot="workbench-left-panel"');
		expect(markup).toContain('data-slot="workbench-right-panel"');
		expect(markup).toContain('flex-basis:320px');
		// As in the TUI, the main area stays blank: no Tab Strip and no statement.
		expect(markup).not.toContain('role="tablist"');
		expect(markup).not.toContain('data-slot="workbench-empty-workspace"');
		expect(markup).not.toContain('data-slot="workbench-sidebar-project"');
		expect(markup).not.toContain('data-slot="workbench-sidebar-workspace"');
		actionRuntime.dispose();
		keyboardShortcutSettingsController.dispose();
		terminalSettingsController.dispose();
	});
});

const inertAuthority = inertWorkbenchAuthority();

function createApplicationLocaleController(): ApplicationLocaleController {
	return new ApplicationLocaleController({
		documentElement: { dir: '', lang: '' },
		preferredLocales: ['en-US'],
		userSettings: memoryUserSettings(),
	});
}

// Covers the live application boundary: file notice -> Settings -> the file is fixed.
it.each(['en-US', 'zh-CN'] as const)(
	'opens recovery from the main window and clears it once the file is fixed in %s',
	async (locale) => {
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
		const control = memoryUserSettings({
			file: { column: 3, line: 7, reason: 'invalid-toml', status: 'syntax-error' },
		});
		const actionRuntime = new ActionRuntime();
		const terminalSettingsController = new TerminalSettingsController({
			themeTarget: { dataset: {} },
			userSettings: control,
		});
		const keyboardShortcutSettingsController = new KeyboardShortcutSettingsController({
			platform: 'standard',
			storage: { getItem: () => null, setItem: () => undefined },
		});
		const applicationLocaleController = createApplicationLocaleController();
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
				<OrchbeeI18nProvider locale={locale}>
					<TooltipProvider>
						<WebApplication
							actionRuntime={actionRuntime}
							applicationLocaleController={applicationLocaleController}
							authority={inertAuthority}
							keyboardShortcutSettingsController={keyboardShortcutSettingsController}
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
		const notice = container.querySelector<HTMLButtonElement>(
			'[data-slot="workbench-status-notice"] button',
		);
		expect(notice).not.toBeNull();
		expect(terminalSettingsController.getSnapshot().open).toBe(false);
		await React.act(async () => notice?.click());
		expect(terminalSettingsController.getSnapshot().open).toBe(true);
		expect(container.querySelector('[data-slot="workbench-status-notice"] button')).toBeNull();
		await React.act(async () =>
			container.querySelector<HTMLButtonElement>('[data-settings-close]')?.click(),
		);
		const returnedNotice = container.querySelector<HTMLButtonElement>(
			'[data-slot="workbench-status-notice"] button',
		);
		expect(returnedNotice).not.toBeNull();
		await React.act(async () => returnedNotice?.click());

		await React.act(async () => control.setFile({ status: 'ok' }));
		await React.act(async () =>
			container.querySelector<HTMLButtonElement>('[data-settings-close]')?.click(),
		);

		expect(container.querySelector('[data-slot="workbench-status-notice"] button')).toBeNull();
	},
);
