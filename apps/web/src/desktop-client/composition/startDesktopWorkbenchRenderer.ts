import type { WorkbenchShortcutPlatform } from '@orchbee/workbench';
import type { WorkbenchClientSessionBootstrap } from '@orchbee/workbench-client-protocol';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import { createWebApplicationRouting, type WebApplicationRouting } from '../../application-routing';
import { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import { MacosTerminalInputSettingsController } from '../../macos-terminal-input-settings';
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
import { DesktopProjectFolderAcquisitionClient } from '../project-folder-acquisition';
import { DesktopTerminalLaunchProfileClient } from '../terminal-launch-profiles';
import type { TerminalFileDropCapability } from '../../terminal-file-drop';

export interface DesktopWorkbenchRendererSession {
	dispose(): void;
	readonly ready: Promise<
		{ readonly kind: 'ready' } | { readonly kind: 'unavailable'; readonly reason: string }
	>;
}

export interface DesktopWorkbenchRendererStorage {
	readonly focusRecovery?: Pick<Storage, 'getItem' | 'removeItem' | 'setItem'> | undefined;
	readonly preferences: Pick<Storage, 'getItem' | 'setItem'>;
}

export function startDesktopWorkbenchRenderer(
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dataset' | 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	shortcutPlatform: WorkbenchShortcutPlatform,
	storage: DesktopWorkbenchRendererStorage,
	bootstrap: WorkbenchClientSessionBootstrap,
	lifetimePort: MessagePort,
	clientSessionPort: MessagePort,
	projectionPort: MessagePort,
	browserAccessPort: MessagePort,
	projectFolderAcquisitionPort: MessagePort,
	terminalLaunchProfilePort: MessagePort,
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
	const projectFolderAcquisitionClient = new DesktopProjectFolderAcquisitionClient(
		projectFolderAcquisitionPort,
	);
	const terminalLaunchProfileClient = new DesktopTerminalLaunchProfileClient(
		terminalLaunchProfilePort,
	);
	const keyboardShortcutSettingsController = new KeyboardShortcutSettingsController({
		platform: shortcutPlatform,
		storage: storage.preferences,
	});
	const macosTerminalInputSettingsController =
		shortcutPlatform === 'apple'
			? new MacosTerminalInputSettingsController(storage.preferences)
			: undefined;
	const terminalSettingsController = new TerminalSettingsController({
		storage: storage.preferences,
		themeTarget: documentElement,
	});
	const terminalFeedbackSettingsController = new TerminalFeedbackSettingsController({
		notificationCapability:
			applicationRoot.ownerDocument.defaultView === null
				? UNSUPPORTED_TERMINAL_NOTIFICATION_CAPABILITY
				: createBrowserTerminalNotificationCapability(applicationRoot.ownerDocument.defaultView),
		storage: storage.preferences,
	});
	const runtime = createWorkbenchClientRuntime({
		clientSession,
		documentElement,
		...(storage.focusRecovery === undefined ? {} : { focusRecoveryStorage: storage.focusRecovery }),
		preferredLocales,
		projectionClient,
		shortcutBindingSource: keyboardShortcutSettingsController,
		shortcutPlatform,
		terminalPresentationConfigurationSource: terminalSettingsController,
		terminalCompletionReminderObserver: terminalFeedbackSettingsController,
		...(macosTerminalInputSettingsController === undefined
			? {}
			: { terminalInputConfigurationSource: macosTerminalInputSettingsController }),
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
		projectFolderAcquisitionClient.dispose();
		terminalLaunchProfileClient.dispose();
		routing?.dispose();
		routing = undefined;
		runtime.dispose();
		keyboardShortcutSettingsController.dispose();
		terminalSettingsController.dispose();
		terminalFeedbackSettingsController.dispose();
		macosTerminalInputSettingsController?.dispose();
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
		const [runtimeStart, terminalLaunchProfilesStart] = await Promise.all([
			workbenchClientRuntime.ready,
			terminalLaunchProfileClient.initialize(),
		]);
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
			layoutStorage: storage.preferences,
			macosTerminalInputSettingsController,
			projectWorkspaceRuntime: workbenchClientRuntime.projectWorkspaceRuntime,
			projectFolderAcquisition: projectFolderAcquisitionClient,
			requestBrowserAccess: (returnFocus) => {
				browserAccessReturnFocus = returnFocus;
				browserAccessController.open();
			},
			shortcutPlatform,
			terminalSettingsController,
			terminalFeedbackSettingsController,
			...(terminalLaunchProfilesStart.kind === 'ready' ? { terminalLaunchProfileClient } : {}),
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
