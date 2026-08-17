import type { GatewayBrowserConnection } from '@orchbee/gateway-protocol';
import type { WorkbenchShortcutPlatform } from '@orchbee/workbench';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import { createWebApplicationRouting, type WebApplicationRouting } from '../../application-routing';
import { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import { TerminalSettingsController } from '../../terminal-settings';
import {
	createBrowserTerminalNotificationCapability,
	TerminalFeedbackSettingsController,
} from '../../terminal-feedback-settings';
import {
	createWorkbenchClientRuntime,
	ProjectedTerminalSurface,
	WorkbenchClientApplication,
	WorkbenchClientSessionClient,
	type WorkbenchClientRuntime,
} from '../../workbench-client';
import { connectGatewayControlConnection, type GatewayControlConnection } from '../connection';
import { createBrowserTerminalPlatformEffects } from '../platform-effects';
import { GatewayTerminalProjectionClient } from '../terminal-projection';

export interface GatewayWebApplicationSession {
	dispose(): void;
	readonly ready: Promise<
		{ readonly kind: 'ready' } | { readonly kind: 'unavailable'; readonly reason: string }
	>;
}

export interface GatewayWebApplicationDependencies {
	readonly connectControl: typeof connectGatewayControlConnection;
	readonly createProjectionClient: (
		control: GatewayControlConnection,
		connection: GatewayBrowserConnection,
		hostname: string,
	) => GatewayTerminalProjectionClient;
	readonly createReactRoot: (element: Element) => Root;
	readonly createRouting: typeof createWebApplicationRouting;
	readonly createRuntime: typeof createWorkbenchClientRuntime;
}

const defaultDependencies: GatewayWebApplicationDependencies = {
	connectControl: connectGatewayControlConnection,
	createProjectionClient: (control, connection, hostname) =>
		new GatewayTerminalProjectionClient(control, connection, hostname),
	createReactRoot: (element) => createRoot(element),
	createRouting: createWebApplicationRouting,
	createRuntime: createWorkbenchClientRuntime,
};

export function startGatewayWebApplication(
	targetWindow: Window,
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dataset' | 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	shortcutPlatform: WorkbenchShortcutPlatform,
	storage: Pick<Storage, 'getItem' | 'setItem'>,
	connection: GatewayBrowserConnection,
	dependencies: GatewayWebApplicationDependencies = defaultDependencies,
): GatewayWebApplicationSession {
	if (!(rootElement instanceof HTMLElement)) {
		throw new Error('The Orchbee Gateway application root element is missing.');
	}
	const applicationRoot = rootElement;
	let control: GatewayControlConnection | undefined;
	let disposed = false;
	let reactRoot: Root | undefined;
	let routing: WebApplicationRouting | undefined;
	let runtime: WorkbenchClientRuntime | undefined;
	const keyboardShortcutSettingsController = new KeyboardShortcutSettingsController({
		platform: shortcutPlatform,
		storage,
	});
	const terminalSettingsController = new TerminalSettingsController({
		storage,
		themeTarget: documentElement,
	});
	const terminalFeedbackSettingsController = new TerminalFeedbackSettingsController({
		notificationCapability: createBrowserTerminalNotificationCapability(targetWindow),
		storage,
	});
	const dispose = (): void => {
		if (disposed) return;
		disposed = true;
		reactRoot?.unmount();
		reactRoot = undefined;
		routing?.dispose();
		routing = undefined;
		runtime?.dispose();
		runtime = undefined;
		keyboardShortcutSettingsController.dispose();
		terminalSettingsController.dispose();
		terminalFeedbackSettingsController.dispose();
		control?.close();
		control = undefined;
	};
	const ready = initialize().catch(() => {
		dispose();
		return { kind: 'unavailable', reason: 'gateway-unavailable' } as const;
	});
	return { dispose, ready };

	async function initialize(): Promise<
		{ readonly kind: 'ready' } | { readonly kind: 'unavailable'; readonly reason: string }
	> {
		const hostname = targetWindow.location.hostname;
		const nextControl = await dependencies.connectControl(connection, hostname);
		if (disposed) {
			nextControl.close();
			return { kind: 'unavailable', reason: 'control-unavailable' };
		}
		control = nextControl;
		const projectionClient = dependencies.createProjectionClient(nextControl, connection, hostname);
		const clientSession = new WorkbenchClientSessionClient(
			{
				close: () => nextControl.close(),
				send: (command) => nextControl.sendClientSession(command),
				subscribe: (listener, unavailable) =>
					nextControl.subscribeClientSession(listener, unavailable),
			},
			nextControl.bootstrap,
		);
		const clipboard = targetWindow.navigator.clipboard;
		const platformEffects = createBrowserTerminalPlatformEffects({
			openExternal: (target) => {
				const anchor = targetWindow.document.createElement('a');
				anchor.href = target;
				anchor.rel = 'noopener noreferrer';
				anchor.target = '_blank';
				anchor.click();
			},
			...(clipboard === undefined
				? {}
				: { writeClipboardText: async (text: string) => clipboard.writeText(text) }),
		});
		const nextRuntime = dependencies.createRuntime({
			clientSession,
			documentElement,
			preferredLocales,
			projectionClient,
			shortcutBindingSource: keyboardShortcutSettingsController,
			shortcutPlatform,
			terminalPlatformEffects: platformEffects,
			terminalCompletionReminderObserver: terminalFeedbackSettingsController,
			terminalPresentationConfigurationSource: terminalSettingsController,
		});
		runtime = nextRuntime;
		const runtimeStart = await nextRuntime.ready;
		if (runtimeStart.kind === 'unavailable') {
			dispose();
			return runtimeStart;
		}
		if (disposed) return { kind: 'unavailable', reason: 'control-unavailable' };
		const nextRouting = dependencies.createRouting({
			actionRuntime: nextRuntime.actionRuntime,
			keyboardShortcutSettingsController,
			layoutStorage: storage,
			projectWorkspaceRuntime: nextRuntime.projectWorkspaceRuntime,
			shortcutPlatform,
			terminalSettingsController,
			terminalFeedbackSettingsController,
			terminalSurface: ProjectedTerminalSurface,
		});
		routing = nextRouting;
		reactRoot = dependencies.createReactRoot(applicationRoot);
		reactRoot.render(
			createElement(WorkbenchClientApplication, {
				locale: nextRuntime.locale,
				router: nextRouting.router,
				terminalSurfaceCapability: nextRuntime.terminalSurfaceCapability,
			}),
		);
		return { kind: 'ready' };
	}
}
