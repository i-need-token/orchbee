/**
 * The Workbench as a Server Client: one Client connection, through Main's relay, carries everything
 * this document reads and writes. It replaces the Main-owned renderer once that path is removed.
 */
import {
	DESKTOP_SERVER_CONNECTION_PORT_ROLES,
	DESKTOP_SERVER_CONNECTION_WINDOW_REQUEST,
	DesktopServerConnectionWindowMessageSchema,
	type DesktopServerConnectionBootstrap,
	type DesktopServerConnectionPortRole,
	type DesktopSessionRefusal,
} from '@orchbee/desktop-client-protocol';
import {
	createOrchbeeIntl,
	getOrchbeeTextDirection,
	OrchbeeI18nProvider,
	resolveOrchbeeLocale,
	workbenchMessages,
} from '@orchbee/i18n';
import { normalizePermittedExternalTerminalUrl } from '@orchbee/terminal';
import { matchesWorkbenchProjectCreation, type WorkbenchShortcutPlatform } from '../../workbench';
import { createMemoryHistory } from '@tanstack/react-router';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import { ApplicationLocaleController } from '../../application-locale';
import { createWebApplicationRouting, type WebApplicationRouting } from '../../application-routing';
import { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import { MacosTerminalInputSettingsController } from '../../macos-terminal-input-settings';
import { runApplicationCleanup } from '../../runApplicationCleanup';
import {
	createBrowserTerminalNotificationCapability,
	TerminalFeedbackSettingsController,
} from '../../terminal-feedback-settings';
import { TerminalInteractionSettingsController } from '../../terminal-interaction-settings';
import { TerminalSettingsController } from '../../terminal-settings';
import {
	admitsAutomaticWorkspace,
	createServerTerminalColorsSource,
	createServerWorkbenchRuntime,
	ProjectedTerminalSurface,
	ServerAgentIntegrationClient,
	ServerConnection,
	ServerTerminalDefaultSettingsClient,
	ServerTerminalLaunchProfileClient,
	ServerUserSettingsClient,
	ServerWorkbenchSession,
	stopServerSession,
	workspaceLaunchAction,
	type ProjectedTerminalPlatformEffects,
	type ServerWorkbenchRuntime,
} from '../../workbench-client';
import { bindDesktopBackgroundStatus, DesktopBackgroundStatusClient } from '../background-status';
import { DesktopWorkbench } from '../desktop-workbench';
import { DesktopProjectFolderAcquisitionClient } from '../project-folder-acquisition';
import { SessionDisconnectedStatus } from '../session-disconnected-status';
import { SessionUnavailable } from '../session-unavailable';
import { MessagePortServerCarrier } from './MessagePortServerCarrier';

type DocumentElement = Pick<HTMLElement, 'dataset' | 'dir' | 'lang'>;

interface DesktopServerWorkbenchStorage {
	readonly focusRecovery?: Pick<Storage, 'getItem' | 'removeItem' | 'setItem'> | undefined;
	readonly preferences: Pick<Storage, 'getItem' | 'setItem'>;
}

interface DesktopServerConnectionAnnouncement {
	readonly backgroundStatusPort: MessagePort;
	readonly bootstrap: DesktopServerConnectionBootstrap;
	readonly connectionPort: MessagePort;
	readonly projectFolderPort: MessagePort;
}

function receiveConnection(targetWindow: Window): Promise<DesktopServerConnectionAnnouncement> {
	return new Promise((resolve, reject) => {
		const handleMessage = (event: MessageEvent<unknown>): void => {
			if (event.source !== targetWindow || event.ports.length === 0) return;
			targetWindow.removeEventListener('message', handleMessage);
			const portFor = (role: DesktopServerConnectionPortRole): MessagePort | undefined =>
				event.ports[DESKTOP_SERVER_CONNECTION_PORT_ROLES.indexOf(role)];
			const connectionPort = portFor('server-connection');
			const projectFolderPort = portFor('project-folder-acquisition');
			const backgroundStatusPort = portFor('background-status');
			const message = DesktopServerConnectionWindowMessageSchema.safeParse(event.data);
			if (
				message.success &&
				event.ports.length === DESKTOP_SERVER_CONNECTION_PORT_ROLES.length &&
				connectionPort !== undefined &&
				projectFolderPort !== undefined &&
				backgroundStatusPort !== undefined
			) {
				resolve({
					backgroundStatusPort,
					bootstrap: message.data.bootstrap,
					connectionPort,
					projectFolderPort,
				});
				return;
			}
			for (const port of event.ports) port.close();
			reject(new Error('The Server connection announcement is invalid.'));
		};
		targetWindow.addEventListener('message', handleMessage);
		targetWindow.postMessage(
			{ type: DESKTOP_SERVER_CONNECTION_WINDOW_REQUEST },
			targetWindow.location.origin,
		);
	});
}

function browserPlatformEffects(targetWindow: Window): ProjectedTerminalPlatformEffects {
	return {
		// Main hands a window's address to the person's browser, as the TUI opens a link itself.
		openExternal: (target) => {
			const url = normalizePermittedExternalTerminalUrl(target);
			if (url === null) return { code: 'link-policy-blocked', kind: 'failed', retryable: false };
			targetWindow.open(url, '_blank', 'noopener,noreferrer');
			return { kind: 'completed' };
		},
		readClipboardText: async () => {
			try {
				return { kind: 'completed', text: await targetWindow.navigator.clipboard.readText() };
			} catch {
				return { code: 'browser-clipboard-read-failed', kind: 'failed', retryable: false };
			}
		},
		// A selection export finishes after its user activation has expired, so this cannot require one.
		writeClipboardText: async (text) => {
			try {
				await targetWindow.navigator.clipboard.writeText(text);
				return { kind: 'completed' };
			} catch {
				return { code: 'browser-clipboard-write-failed', kind: 'failed', retryable: false };
			}
		},
	};
}

/**
 * The Session holds the person's language and theme, so a window that cannot reach it follows the
 * system's. Trying again reloads the document, which has Main start the Session again.
 */
function showSessionUnavailable(
	targetWindow: Window,
	reactRoot: Root,
	documentElement: DocumentElement,
	preferredLocales: readonly string[],
	refusal: DesktopSessionRefusal | null,
): void {
	const locale = resolveOrchbeeLocale(preferredLocales);
	documentElement.lang = locale;
	documentElement.dir = getOrchbeeTextDirection(locale);
	documentElement.dataset['theme'] = targetWindow.matchMedia('(prefers-color-scheme: dark)').matches
		? 'dark'
		: 'light';
	reactRoot.render(
		createElement(
			OrchbeeI18nProvider,
			{ locale },
			createElement(SessionUnavailable, {
				onTryAgain: () => targetWindow.location.reload(),
				refusal,
			}),
		),
	);
}

async function run(
	targetWindow: Window,
	reactRoot: Root,
	documentElement: DocumentElement,
	preferredLocales: readonly string[],
	shortcutPlatform: WorkbenchShortcutPlatform,
	storage: DesktopServerWorkbenchStorage,
): Promise<void> {
	const { backgroundStatusPort, bootstrap, connectionPort, projectFolderPort } =
		await receiveConnection(targetWindow);
	const backgroundStatus = new DesktopBackgroundStatusClient(backgroundStatusPort);
	targetWindow.addEventListener('unload', () => backgroundStatus.dispose(), { once: true });
	const connection = new ServerConnection(new MessagePortServerCarrier(connectionPort));
	targetWindow.addEventListener('unload', () => connection.close(), { once: true });
	const session = await ServerWorkbenchSession.open(connection).catch(() => null);
	if (session === null) {
		// A refused start leaves Main's connection nothing to reach, so it ends before any read.
		showSessionUnavailable(
			targetWindow,
			reactRoot,
			documentElement,
			preferredLocales,
			bootstrap.sessionRefusal,
		);
		return;
	}
	// Quit All in Main's notification-area menu stops the Session through this connection.
	backgroundStatus.serveSessionStop((stopId) => stopServerSession(session, stopId));

	// Search is withheld in every distribution until it is finished (ORC-424); its code stays
	// for later.
	const searchOffered = false;
	const environmentMarker =
		bootstrap.distribution === 'release'
			? undefined
			: { checkoutName: bootstrap.checkoutName, environment: bootstrap.distribution };
	const userSettings = new ServerUserSettingsClient(connection, session.snapshot.sessionId);
	const terminalDefaultSettingsClient = new ServerTerminalDefaultSettingsClient(userSettings);
	const terminalLaunchProfileClient = new ServerTerminalLaunchProfileClient(
		userSettings,
		bootstrap.loginShellAvailable,
	);
	const applicationLocaleController = new ApplicationLocaleController({
		documentElement,
		preferredLocales,
		userSettings,
	});
	// Shortcut overrides are this device's own, so browser storage keeps them.
	const keyboardShortcutSettingsController = new KeyboardShortcutSettingsController({
		platform: shortcutPlatform,
		searchOffered,
		storage: storage.preferences,
	});
	const macosTerminalInputSettingsController =
		shortcutPlatform === 'apple'
			? new MacosTerminalInputSettingsController(userSettings)
			: undefined;
	const systemTheme = targetWindow.matchMedia('(prefers-color-scheme: dark)');
	const terminalSettingsController = new TerminalSettingsController({
		systemTheme,
		themeTarget: documentElement,
		userSettings,
	});
	const terminalInteractionSettingsController = new TerminalInteractionSettingsController(
		userSettings,
	);
	const terminalFeedbackSettingsController = new TerminalFeedbackSettingsController({
		// A page cannot show a window Main hid, so a notification click asks Main to show it first.
		notificationCapability: createBrowserTerminalNotificationCapability({
			focus: () => {
				backgroundStatus.requestWindow();
				targetWindow.focus();
			},
		}),
		userSettings,
	});
	const agentIntegrationManagementClient = new ServerAgentIntegrationClient(
		connection,
		session.snapshot.sessionId,
	);

	let routing: WebApplicationRouting | undefined;
	let runtime: ServerWorkbenchRuntime | undefined;
	let stopBackgroundStatus: (() => void) | undefined;
	const projectFolderAcquisition = new DesktopProjectFolderAcquisitionClient(
		projectFolderPort,
		undefined,
		// The Server settles a creation only once its publication carries it, so a Project this
		// window shows is already every Client's.
		(receipt) =>
			Promise.resolve(
				runtime !== undefined &&
					matchesWorkbenchProjectCreation(runtime.projectWorkspaceRuntime, receipt),
			),
	);
	targetWindow.addEventListener(
		'unload',
		() => {
			runApplicationCleanup(
				[
					() => reactRoot.unmount(),
					() => stopBackgroundStatus?.(),
					() => routing?.dispose(),
					() => runtime?.dispose(),
					() => projectFolderAcquisition.dispose(),
					() => keyboardShortcutSettingsController.dispose(),
					() => terminalSettingsController.dispose(),
					() => terminalInteractionSettingsController.dispose(),
					() => terminalFeedbackSettingsController.dispose(),
					() => macosTerminalInputSettingsController?.dispose(),
					() => applicationLocaleController.dispose(),
					() => agentIntegrationManagementClient.dispose(),
					() => terminalLaunchProfileClient.dispose(),
					() => terminalDefaultSettingsClient.dispose(),
					() => userSettings.dispose(),
					() => session.dispose(),
				],
				'Desktop Server Workbench cleanup failed.',
			);
		},
		{ once: true },
	);

	await userSettings.refresh();
	runtime = createServerWorkbenchRuntime({
		applicationLocaleSource: applicationLocaleController,
		connection,
		defaultDirectory: bootstrap.defaultDirectory,
		documentElement,
		focusRecoveryStorage: storage.focusRecovery,
		// Main names the person's home folder, and the Server it started runs beside it.
		homeDirectory: bootstrap.defaultDirectory,
		launchProfiles: terminalLaunchProfileClient,
		// A publication this document cannot apply leaves nothing current to show, so it disconnects.
		onPublicationFailure: () => session.dispose(),
		preferredLocales,
		session,
		shortcutBindingSource: keyboardShortcutSettingsController,
		shortcutPlatform,
		terminalCompletionReminderObserver: terminalFeedbackSettingsController,
		terminalColorsSource: createServerTerminalColorsSource(terminalSettingsController, systemTheme),
		terminalHostName: bootstrap.terminalHostName,
		terminalInputConfigurationSource: macosTerminalInputSettingsController,
		terminalInteractionConfigurationSource: terminalInteractionSettingsController,
		terminalPlatformEffects: browserPlatformEffects(targetWindow),
		terminalPresentationConfigurationSource: terminalSettingsController,
		terminalTrailingBlankRowsPreferenceSource:
			terminalInteractionSettingsController.trailingBlankRowsPreferenceSource,
		// Electron names the window after its page, which the taskbar and window switcher show.
		titleWindow: targetWindow,
	});
	// What Main's notification-area icon shows while the window is hidden.
	stopBackgroundStatus = bindDesktopBackgroundStatus({
		agentObservationSource: runtime.agentObservationSource,
		client: backgroundStatus,
		projectWorkspaceRuntime: runtime.projectWorkspaceRuntime,
		session,
	});
	routing = createWebApplicationRouting(
		{
			actionRuntime: runtime.actionRuntime,
			agentIntegrationManagementControl: agentIntegrationManagementClient,
			agentNameSource: runtime.agentNameSource,
			agentObservationSource: runtime.agentObservationSource,
			applicationLocaleController,
			authority: runtime.authority,
			environmentMarker,
			keyboardShortcutSettingsController,
			layoutStorage: storage.preferences,
			macosTerminalInputSettingsController,
			projectFolderAcquisition,
			projectWorkspaceRuntime: runtime.projectWorkspaceRuntime,
			searchOffered,
			shortcutPlatform,
			terminalDefaultSettingsControl: terminalDefaultSettingsClient,
			terminalFeedbackSettingsController,
			terminalInteractionSettingsController,
			terminalLaunchProfileClient,
			terminalSettingsController,
			terminalSurface: ProjectedTerminalSurface,
			workspaceLocationSource: runtime.workspaceLocationSource,
		},
		// A window has no address bar, and a reload needs the URL Main loaded it with. Focus recovery
		// returns it to where it was.
		createMemoryHistory(),
	);
	reactRoot.render(
		createElement(DesktopWorkbench, {
			applicationLocaleSource: applicationLocaleController,
			// Reconnecting is a reload: Main starts the Session again when it stopped, then connects.
			connectionStatus: createElement(SessionDisconnectedStatus, {
				ended: {
					getSnapshot: () => session.ended !== null,
					subscribe: (listener) => session.onEnded(listener),
				},
				onReconnect: () => targetWindow.location.reload(),
			}),
			router: routing.router,
			terminalSurfaceCapability: runtime.terminalSurfaceCapability,
			windowControls: shortcutPlatform === 'apple' ? 'leading' : 'trailing',
		}),
	);

	if (bootstrap.automaticWorkspaceAllowed) {
		// A refusal leaves the Session as it is and the empty state offers the same creation; a lost
		// answer must not be repeated, because the Workspace may exist.
		void session.submit((snapshot) =>
			admitsAutomaticWorkspace(snapshot)
				? workspaceLaunchAction({
						directory: bootstrap.defaultDirectory,
						fallbackLabel: createOrchbeeIntl(applicationLocaleController.current()).formatMessage(
							workbenchMessages.workspaceDefaultName,
						),
						placement: { kind: 'ungrouped' },
						snapshot,
					})
				: null,
		);
	}
}

export function startDesktopServerWorkbench(
	targetWindow: Window,
	rootElement: Element | null,
	documentElement: DocumentElement,
	preferredLocales: readonly string[],
	shortcutPlatform: WorkbenchShortcutPlatform,
	storage: DesktopServerWorkbenchStorage,
): void {
	if (!(rootElement instanceof HTMLElement)) {
		throw new Error('The Orchbee Desktop application root element is missing.');
	}
	const reactRoot = createRoot(rootElement);
	void run(
		targetWindow,
		reactRoot,
		documentElement,
		preferredLocales,
		shortcutPlatform,
		storage,
	).catch(() => {
		showSessionUnavailable(targetWindow, reactRoot, documentElement, preferredLocales, null);
	});
}
