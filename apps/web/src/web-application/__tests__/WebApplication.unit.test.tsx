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
	WorkbenchUpdateCheckModel,
	type WorkbenchClientUpdate,
	type WorkbenchUpdateCheckAnswer,
} from '../../workbench';
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

// Regression for 0.1.0-beta.15/beta.16: drives the real WebApplication + Workbench + Settings ›
// About composition through one unmocked ActionRuntime, verifying manual check failure recovery and
// Update execution from both surfaces.
it.each(['en-US', 'zh-CN'] as const)(
	'wires Check for updates, the failed-check installer escape hatch, and both Update buttons through the root ActionRuntime in %s',
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
		Object.defineProperty(Element.prototype, 'scrollIntoView', {
			configurable: true,
			value: vi.fn<Element['scrollIntoView']>(),
		});

		let offeredVersion: string | null = null;
		const offerListeners = new Set<() => void>();
		const setOffer = (next: string | null): void => {
			offeredVersion = next;
			for (const listener of offerListeners) listener();
		};
		const offer = {
			getSnapshot: () => offeredVersion,
			subscribe: (listener: () => void) => {
				offerListeners.add(listener);
				return () => offerListeners.delete(listener);
			},
		};

		let resolveStart: ((answer: WorkbenchUpdateCheckAnswer) => void) | undefined;
		const ask = vi.fn<(action: 'read' | 'start') => Promise<WorkbenchUpdateCheckAnswer>>(
			(action) => {
				if (action === 'read') return Promise.resolve('idle');
				return new Promise((resolve) => {
					resolveStart = resolve;
				});
			},
		);
		const updateCheck = new WorkbenchUpdateCheckModel({
			ask,
			offer,
			unavailable: null,
		});
		const request = vi.fn<(version: string) => void>();
		const clientUpdate: WorkbenchClientUpdate = {
			check: updateCheck,
			getSnapshot: offer.getSnapshot,
			request,
			subscribe: offer.subscribe,
			subscribeWaiting: () => () => undefined,
		};
		const openExternal = vi.fn<(target: string) => void>();

		const actionRuntime = new ActionRuntime();
		const terminalSettingsController = new TerminalSettingsController({
			themeTarget: { dataset: {} },
			userSettings: memoryUserSettings(),
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
			updateCheck.dispose();
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
							clientBuild={{ distribution: 'release', version: '0.1.0-beta.16' }}
							clientUpdate={clientUpdate}
							keyboardShortcutSettingsController={keyboardShortcutSettingsController}
							openExternal={openExternal}
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

		expect(container.querySelector('[data-slot="workbench-sidebar-update"]')).toBeNull();

		// Open Settings -> About while no update is offered.
		await React.act(async () =>
			container
				.querySelector<HTMLButtonElement>('[data-slot="workbench-sidebar-settings"]')
				?.click(),
		);
		await React.act(async () =>
			container.querySelector<HTMLButtonElement>("button[data-settings-category='about']")?.click(),
		);
		expect(container.querySelector('[data-about-version]')?.textContent).toBe('0.1.0-beta.16');
		expect(ask).toHaveBeenCalledWith('read');
		expect(container.querySelector('[data-about-download]')).toBeNull();

		const checkButton = container.querySelector<HTMLButtonElement>('[data-about-check]');
		expect(checkButton).not.toBeNull();
		expect(checkButton?.disabled).toBe(false);

		// Press Check for updates -> transitions to Checking…, then fails and exposes the escape hatch.
		await React.act(async () => checkButton?.click());
		expect(ask).toHaveBeenCalledWith('start');
		expect(checkButton?.disabled).toBe(true);
		expect(checkButton?.textContent).toBe(locale === 'en-US' ? 'Checking…' : '正在检查…');
		expect(container.querySelector('[data-about-download]')).toBeNull();

		await React.act(async () => {
			resolveStart?.('run-failed');
			await Promise.resolve();
		});
		expect(checkButton?.disabled).toBe(false);
		expect(container.querySelector('[data-settings-content="about"]')?.textContent).toContain(
			'orchbee.com',
		);
		const downloadButton = container.querySelector<HTMLButtonElement>('[data-about-download]');
		expect(downloadButton).not.toBeNull();
		await React.act(async () => downloadButton?.click());
		expect(openExternal).toHaveBeenCalledWith('https://orchbee.com/#install');

		// Offer an update -> About's Update button invokes clientUpdate.request through ActionRuntime.
		await React.act(async () => setOffer('0.1.0-beta.17'));
		expect(container.querySelector('[data-about-check]')).toBeNull();
		expect(container.querySelector('[data-about-download]')).toBeNull();
		const aboutUpdateButton = container.querySelector<HTMLButtonElement>('[data-about-update]');
		expect(aboutUpdateButton).not.toBeNull();
		await React.act(async () => {
			aboutUpdateButton?.click();
			await Promise.resolve();
		});
		expect(request.mock.calls).toEqual([['0.1.0-beta.17']]);

		// Close Settings -> sidebar footer's primary Update button invokes the same ActionRuntime registration.
		await React.act(async () =>
			container.querySelector<HTMLButtonElement>('[data-settings-close]')?.click(),
		);
		const sidebarUpdateButton = container.querySelector<HTMLButtonElement>(
			'[data-slot="workbench-sidebar-update"]',
		);
		expect(sidebarUpdateButton).not.toBeNull();
		await React.act(async () => {
			sidebarUpdateButton?.click();
			await Promise.resolve();
		});
		expect(request.mock.calls).toEqual([['0.1.0-beta.17'], ['0.1.0-beta.17']]);
	},
);

function createFakeClientUpdate(initialOffer: string | null) {
	let offer = initialOffer;
	const listeners = new Set<() => void>();
	const waitingListeners = new Set<() => void>();
	const failedListeners = new Set<() => void>();
	const request = vi.fn<(version: string) => void>();
	const clientUpdate: WorkbenchClientUpdate = {
		getSnapshot: () => offer,
		request,
		subscribe: (listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		subscribeFailed: (listener) => {
			failedListeners.add(listener);
			return () => failedListeners.delete(listener);
		},
		subscribeWaiting: (listener) => {
			waitingListeners.add(listener);
			return () => waitingListeners.delete(listener);
		},
	};
	return {
		clientUpdate,
		emitFailed: () => {
			for (const listener of failedListeners) listener();
		},
		emitWaiting: () => {
			for (const listener of waitingListeners) listener();
		},
		request,
		setOfferSilently: (next: string | null) => {
			offer = next;
		},
	};
}

async function mountWebApplication({
	clientBuild,
	clientUpdate,
	locale,
}: {
	readonly clientBuild?: {
		readonly distribution: 'development' | 'release' | 'test';
		readonly version: string | null;
	};
	readonly clientUpdate: WorkbenchClientUpdate;
	readonly locale: 'en-US' | 'zh-CN';
}) {
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
	const actionRuntime = new ActionRuntime();
	const terminalSettingsController = new TerminalSettingsController({
		themeTarget: { dataset: {} },
		userSettings: memoryUserSettings(),
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
						clientBuild={clientBuild}
						clientUpdate={clientUpdate}
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
	return { container };
}

const EXPECTED_UPDATE_UNAVAILABLE = {
	'en-US': "Couldn't update here. Quit Orchbee and open it again, or run orchbee update.",
	'zh-CN': '无法在此更新。请退出 Orchbee 后重新打开，或运行 orchbee update。',
} as const;

describe('WebApplication update wiring and refusals', () => {
	it.each(['en-US', 'zh-CN'] as const)(
		'routes pressing Update from both sidebar footer and About to clientUpdate.request and reports failed launches in %s',
		async (locale) => {
			const updatePort = createFakeClientUpdate('0.3.0');
			const { container } = await mountWebApplication({
				clientBuild: { distribution: 'release', version: '0.2.0' },
				clientUpdate: updatePort.clientUpdate,
				locale,
			});

			const sidebarUpdate = container.querySelector<HTMLButtonElement>(
				'[data-slot="workbench-sidebar-update"]',
			);
			expect(sidebarUpdate).not.toBeNull();
			await React.act(async () => sidebarUpdate?.click());
			expect(updatePort.request).toHaveBeenNthCalledWith(1, '0.3.0');

			await React.act(async () => updatePort.emitFailed());
			expect(
				container.querySelector('[data-slot="workbench-status-notice"] span')?.textContent,
			).toBe(EXPECTED_UPDATE_UNAVAILABLE[locale]);

			await React.act(async () =>
				container
					.querySelector<HTMLButtonElement>('[data-slot="workbench-sidebar-settings"]')
					?.click(),
			);
			await React.act(async () =>
				container.querySelector<HTMLButtonElement>('[data-settings-category="about"]')?.click(),
			);

			const aboutUpdate = container.querySelector<HTMLButtonElement>('[data-about-update]');
			expect(aboutUpdate).not.toBeNull();
			await React.act(async () => aboutUpdate?.click());
			expect(updatePort.request).toHaveBeenNthCalledWith(2, '0.3.0');

			await React.act(async () => updatePort.emitFailed());
			expect(container.querySelector('[data-slot="settings-notice"]')?.textContent).toBe(
				EXPECTED_UPDATE_UNAVAILABLE[locale],
			);

			await React.act(async () =>
				container.querySelector<HTMLButtonElement>('[data-slot="settings-notice"] button')?.click(),
			);
			expect(container.querySelector('[data-slot="settings-notice"]')).toBeNull();

			await React.act(async () => aboutUpdate?.click());
			await React.act(async () => updatePort.emitFailed());
			expect(container.querySelector('[data-slot="settings-notice"]')?.textContent).toBe(
				EXPECTED_UPDATE_UNAVAILABLE[locale],
			);
		},
	);

	it.each(['en-US', 'zh-CN'] as const)(
		'reports a refused Update on sidebar footer and Settings About when offer vanishes before invoke in %s',
		async (locale) => {
			const updatePort = createFakeClientUpdate('0.3.0');
			const { container } = await mountWebApplication({
				clientBuild: { distribution: 'release', version: '0.2.0' },
				clientUpdate: updatePort.clientUpdate,
				locale,
			});

			// Drop the offered version right before pressing Update so the Action refuses with client-update-unavailable.
			updatePort.setOfferSilently(null);
			await React.act(async () =>
				container
					.querySelector<HTMLButtonElement>('[data-slot="workbench-sidebar-update"]')
					?.click(),
			);
			expect(updatePort.request).not.toHaveBeenCalled();
			expect(
				container.querySelector('[data-slot="workbench-status-notice"] span')?.textContent,
			).toBe(EXPECTED_UPDATE_UNAVAILABLE[locale]);

			// Restore the offer so About mounts its Update button, then drop the offer before clicking it.
			updatePort.setOfferSilently('0.3.0');
			await React.act(async () =>
				container
					.querySelector<HTMLButtonElement>('[data-slot="workbench-sidebar-settings"]')
					?.click(),
			);
			await React.act(async () =>
				container.querySelector<HTMLButtonElement>('[data-settings-category="about"]')?.click(),
			);

			const aboutUpdate = container.querySelector<HTMLButtonElement>('[data-about-update]');
			expect(aboutUpdate).not.toBeNull();
			updatePort.setOfferSilently(null);
			await React.act(async () => aboutUpdate?.click());
			expect(updatePort.request).not.toHaveBeenCalled();
			expect(container.querySelector('[data-slot="settings-notice"]')?.textContent).toBe(
				EXPECTED_UPDATE_UNAVAILABLE[locale],
			);
		},
	);
});
