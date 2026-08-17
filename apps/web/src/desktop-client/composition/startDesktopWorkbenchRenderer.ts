import type { WorkbenchShortcutPlatform } from '@orchbee/workbench';
import type { WorkbenchClientSessionBootstrap } from '@orchbee/workbench-client-protocol';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import { createWebApplicationRouting, type WebApplicationRouting } from '../../application-routing';
import { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import { TerminalSettingsController } from '../../terminal-settings';
import {
	createBrowserTerminalNotificationCapability,
	TerminalFeedbackSettingsController,
	UNSUPPORTED_TERMINAL_NOTIFICATION_CAPABILITY,
} from '../../terminal-feedback-settings';
import {
	createWorkbenchClientRuntime,
	ProjectedTerminalSurface,
	type WorkbenchClientRuntime,
} from '../../workbench-client';
import { DesktopClientSessionClient } from '../client-session';
import {
	DesktopApplicationLifetimeClient,
	DesktopApplicationLifetimeCoordinator,
} from '../desktop-application-lifetime-dialog';
import {
	DesktopBrowserAccessClient,
	DesktopBrowserAccessController,
} from '../desktop-browser-access-dialog';
import { DesktopWorkbench } from '../desktop-workbench';
import { DesktopTerminalProjectionClient } from '../terminal-projection-control';
import type { TerminalFileDropCapability } from '../../terminal-file-drop';

export interface DesktopWorkbenchRendererSession {
	dispose(): void;
	readonly ready: Promise<
		{ readonly kind: 'ready' } | { readonly kind: 'unavailable'; readonly reason: string }
	>;
}

export function startDesktopWorkbenchRenderer(
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dataset' | 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	shortcutPlatform: WorkbenchShortcutPlatform,
	storage: Pick<Storage, 'getItem' | 'setItem'>,
	bootstrap: WorkbenchClientSessionBootstrap,
	lifetimePort: MessagePort,
	clientSessionPort: MessagePort,
	projectionPort: MessagePort,
	browserAccessPort: MessagePort,
	terminalFileDropCapability?: TerminalFileDropCapability,
): DesktopWorkbenchRendererSession {
	if (!(rootElement instanceof HTMLElement)) {
		throw new Error('The Orchbee Desktop application root element is missing.');
	}
	const applicationRoot = rootElement;
	const clientSession = new DesktopClientSessionClient(clientSessionPort, bootstrap);
	const projectionClient = new DesktopTerminalProjectionClient(projectionPort);
	const browserAccessController = new DesktopBrowserAccessController(
		new DesktopBrowserAccessClient(browserAccessPort),
	);
	const keyboardShortcutSettingsController = new KeyboardShortcutSettingsController({
		platform: shortcutPlatform,
		storage,
	});
	const terminalSettingsController = new TerminalSettingsController({
		storage,
		themeTarget: documentElement,
	});
	const terminalFeedbackSettingsController = new TerminalFeedbackSettingsController({
		notificationCapability:
			applicationRoot.ownerDocument.defaultView === null
				? UNSUPPORTED_TERMINAL_NOTIFICATION_CAPABILITY
				: createBrowserTerminalNotificationCapability(applicationRoot.ownerDocument.defaultView),
		storage,
	});
	const runtime = createWorkbenchClientRuntime({
		clientSession,
		documentElement,
		preferredLocales,
		projectionClient,
		shortcutBindingSource: keyboardShortcutSettingsController,
		shortcutPlatform,
		terminalPresentationConfigurationSource: terminalSettingsController,
		terminalCompletionReminderObserver: terminalFeedbackSettingsController,
		...(terminalFileDropCapability === undefined ? {} : { terminalFileDropCapability }),
	});
	let applicationLifetimeCoordinator: DesktopApplicationLifetimeCoordinator | undefined;
	let browserAccessReturnFocus: HTMLElement | null = null;
	let disposed = false;
	let lifetimeClient: DesktopApplicationLifetimeClient | undefined;
	let reactRoot: Root | undefined;
	let routing: WebApplicationRouting | undefined;

	const dispose = (): void => {
		if (disposed) return;
		disposed = true;
		applicationLifetimeCoordinator?.dispose();
		applicationLifetimeCoordinator = undefined;
		browserAccessController.dispose();
		reactRoot?.unmount();
		reactRoot = undefined;
		routing?.dispose();
		routing = undefined;
		runtime.dispose();
		keyboardShortcutSettingsController.dispose();
		terminalSettingsController.dispose();
		terminalFeedbackSettingsController.dispose();
		if (lifetimeClient === undefined) lifetimePort.close();
		else lifetimeClient.dispose();
	};

	const ready = initialize(runtime).catch(() => {
		dispose();
		return { kind: 'unavailable', reason: 'runtime-unavailable' } as const;
	});
	return { dispose, ready };

	async function initialize(
		workbenchClientRuntime: WorkbenchClientRuntime,
	): Promise<
		{ readonly kind: 'ready' } | { readonly kind: 'unavailable'; readonly reason: string }
	> {
		const runtimeStart = await workbenchClientRuntime.ready;
		if (runtimeStart.kind === 'unavailable') {
			dispose();
			return runtimeStart;
		}
		if (disposed) return { kind: 'unavailable', reason: 'control-unavailable' };
		lifetimeClient = new DesktopApplicationLifetimeClient(lifetimePort);
		applicationLifetimeCoordinator = new DesktopApplicationLifetimeCoordinator(
			workbenchClientRuntime.projectWorkspaceRuntime,
			workbenchClientRuntime.documentCoordinator,
			lifetimeClient,
		);
		const nextRouting = createWebApplicationRouting({
			actionRuntime: workbenchClientRuntime.actionRuntime,
			keyboardShortcutSettingsController,
			layoutStorage: storage,
			projectWorkspaceRuntime: workbenchClientRuntime.projectWorkspaceRuntime,
			requestBrowserAccess: (returnFocus) => {
				browserAccessReturnFocus = returnFocus;
				browserAccessController.open();
			},
			shortcutPlatform,
			terminalSettingsController,
			terminalFeedbackSettingsController,
			terminalSurface: ProjectedTerminalSurface,
		});
		routing = nextRouting;
		reactRoot = createRoot(applicationRoot);
		reactRoot.render(
			createElement(DesktopWorkbench, {
				applicationLifetimeCoordinator,
				browserAccessFinalFocus: () => {
					const returnFocus = browserAccessReturnFocus;
					browserAccessReturnFocus = null;
					return returnFocus?.isConnected === true ? returnFocus : null;
				},
				browserAccessController,
				locale: workbenchClientRuntime.locale,
				router: nextRouting.router,
				terminalSurfaceCapability: workbenchClientRuntime.terminalSurfaceCapability,
				windowControls: shortcutPlatform === 'apple' ? 'leading' : 'trailing',
			}),
		);
		return { kind: 'ready' };
	}
}
