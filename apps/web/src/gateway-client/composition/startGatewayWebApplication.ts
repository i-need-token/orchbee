import type { GatewayBrowserConnection } from '@orchbee/gateway-protocol';
import { createOrchbeeIntl, workbenchTabMessages } from '@orchbee/i18n';
import type { WorkbenchShortcutPlatform } from '@orchbee/workbench';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import { createWebApplicationRouting, type WebApplicationRouting } from '../../application-routing';
import { ApplicationLocaleController } from '../../application-locale';
import { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import { TerminalInteractionSettingsController } from '../../terminal-interaction-settings';
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
import { createGatewayRoutingHistory } from './createGatewayRoutingHistory';

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
	readonly createRoutingHistory: typeof createGatewayRoutingHistory;
	readonly createRuntime: typeof createWorkbenchClientRuntime;
}

const defaultDependencies: GatewayWebApplicationDependencies = {
	connectControl: connectGatewayControlConnection,
	createProjectionClient: (control, connection, hostname) =>
		new GatewayTerminalProjectionClient(control, connection, hostname),
	createReactRoot: (element) => createRoot(element),
	createRouting: createWebApplicationRouting,
	createRoutingHistory: createGatewayRoutingHistory,
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
	const applicationLocaleController = new ApplicationLocaleController({
		documentElement,
		preferredLocales,
		storage,
	});
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
	const terminalInteractionSettingsController = new TerminalInteractionSettingsController(storage);
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
		terminalInteractionSettingsController.dispose();
		terminalFeedbackSettingsController.dispose();
		applicationLocaleController.dispose();
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
		const platformEffects = createBrowserTerminalPlatformEffects({
			getClipboardTextWriter: () => {
				const clipboard = targetWindow.navigator.clipboard;
				return clipboard === undefined
					? undefined
					: async (text: string) => clipboard.writeText(text);
			},
			hasTransientUserActivation: () => targetWindow.navigator.userActivation?.isActive === true,
			isSecureContext: () => targetWindow.isSecureContext,
			openExternal: (target) => {
				const anchor = targetWindow.document.createElement('a');
				anchor.href = target;
				anchor.rel = 'noopener noreferrer';
				anchor.target = '_blank';
				anchor.click();
			},
		});
		const nextRuntime = dependencies.createRuntime({
			applicationLocaleSource: applicationLocaleController,
			clientSession,
			documentElement,
			preferredLocales,
			projectionClient,
			shortcutBindingSource: keyboardShortcutSettingsController,
			shortcutPlatform,
			terminalPlatformEffects: platformEffects,
			terminalCompletionReminderObserver: terminalFeedbackSettingsController,
			terminalPresentationConfigurationSource: terminalSettingsController,
			terminalInteractionConfigurationSource: terminalInteractionSettingsController,
			terminalTrailingBlankRowsPreferenceSource:
				terminalInteractionSettingsController.trailingBlankRowsPreferenceSource,
		});
		runtime = nextRuntime;
		const runtimeStart = await nextRuntime.ready;
		if (runtimeStart.kind === 'unavailable') {
			dispose();
			return runtimeStart;
		}
		if (disposed) return { kind: 'unavailable', reason: 'control-unavailable' };
		const routingHistory = dependencies.createRoutingHistory(targetWindow, connection);
		const getInitialTerminalTitle = (): string =>
			createOrchbeeIntl(applicationLocaleController.current()).formatMessage(
				workbenchTabMessages.terminalTitle,
			);
		let nextRouting: WebApplicationRouting;
		try {
			nextRouting = dependencies.createRouting(
				{
					actionRuntime: nextRuntime.actionRuntime,
					applicationLocaleController,
					getInitialTerminalTitle,
					keyboardShortcutSettingsController,
					layoutStorage: storage,
					projectWorkspaceRuntime: nextRuntime.projectWorkspaceRuntime,
					shortcutPlatform,
					terminalSettingsController,
					terminalInteractionSettingsController,
					terminalFeedbackSettingsController,
					terminalSurface: ProjectedTerminalSurface,
				},
				routingHistory,
			);
		} catch (error: unknown) {
			routingHistory.destroy();
			throw error;
		}
		routing = nextRouting;
		reactRoot = dependencies.createReactRoot(applicationRoot);
		reactRoot.render(
			createElement(WorkbenchClientApplication, {
				applicationLocaleSource: applicationLocaleController,
				router: nextRouting.router,
				terminalSurfaceCapability: nextRuntime.terminalSurfaceCapability,
			}),
		);
		return { kind: 'ready' };
	}
}
