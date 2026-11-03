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
} from '@orchbee/desktop-client-protocol';
import {
	createOrchbeeIntl,
	getOrchbeeTextDirection,
	OrchbeeI18nProvider,
	resolveOrchbeeLocale,
	workbenchMessages,
} from '@orchbee/i18n';
import { normalizePermittedExternalTerminalUrl } from '@orchbee/terminal';
import {
	matchesWorkbenchProjectCreation,
	type WorkbenchShortcutPlatform,
} from '@orchbee/workbench';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench-client-protocol';
import { createMemoryHistory } from '@tanstack/react-router';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import { AgentIntegrationReminderController } from '../../agent-integration-management';
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
	ServerUserPreferencesClient,
	ServerWorkbenchSession,
	workspaceLaunchAction,
	type ProjectedTerminalPlatformEffects,
	type ServerWorkbenchRuntime,
} from '../../workbench-client';
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
			const message = DesktopServerConnectionWindowMessageSchema.safeParse(event.data);
			if (
				message.success &&
				event.ports.length === DESKTOP_SERVER_CONNECTION_PORT_ROLES.length &&
				connectionPort !== undefined &&
				projectFolderPort !== undefined
			) {
				resolve({ bootstrap: message.data.bootstrap, connectionPort, projectFolderPort });
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
	reason: 'recovery-required' | 'unavailable',
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
				reason,
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
	const { bootstrap, connectionPort, projectFolderPort } = await receiveConnection(targetWindow);
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
			bootstrap.sessionRecoveryRequired ? 'recovery-required' : 'unavailable',
		);
		return;
	}

	const userPreferences = new ServerUserPreferencesClient(connection, session.snapshot.sessionId);
	const terminalDefaultSettingsClient = new ServerTerminalDefaultSettingsClient(userPreferences);
	const terminalLaunchProfileClient = new ServerTerminalLaunchProfileClient(
		userPreferences,
		bootstrap.loginShellAvailable,
	);
	const applicationLocaleController = new ApplicationLocaleController({
		documentElement,
		preferredLocales,
		storage: storage.preferences,
		userPreferences,
	});
	const keyboardShortcutSettingsController = new KeyboardShortcutSettingsController({
		platform: shortcutPlatform,
		storage: storage.preferences,
	});
	const macosTerminalInputSettingsController =
		shortcutPlatform === 'apple'
			? new MacosTerminalInputSettingsController(storage.preferences)
			: undefined;
	const systemTheme = targetWindow.matchMedia('(prefers-color-scheme: dark)');
	const terminalSettingsController = new TerminalSettingsController({
		storage: storage.preferences,
		systemTheme,
		themeTarget: documentElement,
		userPreferences,
	});
	const terminalInteractionSettingsController = new TerminalInteractionSettingsController(
		storage.preferences,
		userPreferences,
	);
	const terminalFeedbackSettingsController = new TerminalFeedbackSettingsController({
		notificationCapability: createBrowserTerminalNotificationCapability(targetWindow),
		storage: storage.preferences,
		userPreferences,
	});
	const agentIntegrationManagementClient = new ServerAgentIntegrationClient(
		connection,
		session.snapshot.sessionId,
	);
	// Settings still records dismissals; a reminder waits until this path observes Agent launches.
	const agentIntegrationReminderController = new AgentIntegrationReminderController(
		storage.preferences,
	);

	let routing: WebApplicationRouting | undefined;
	let runtime: ServerWorkbenchRuntime | undefined;
	const projectFolderAcquisition = new DesktopProjectFolderAcquisitionClient(
		projectFolderPort,
		undefined,
		// The Server settles a creation only once its publication carries it, so a Project this
		// document shows is already every Client's.
		(receipt) =>
			Promise.resolve(
				runtime !== undefined &&
					matchesWorkbenchProjectCreation(
						runtime.projectWorkspaceRuntime.getSharedDocument(workbenchSharedDocumentRevision('0')),
						receipt,
					),
			),
	);
	targetWindow.addEventListener(
		'unload',
		() => {
			runApplicationCleanup(
				[
					() => reactRoot.unmount(),
					() => routing?.dispose(),
					() => runtime?.dispose(),
					() => projectFolderAcquisition.dispose(),
					() => keyboardShortcutSettingsController.dispose(),
					() => terminalSettingsController.dispose(),
					() => terminalInteractionSettingsController.dispose(),
					() => terminalFeedbackSettingsController.dispose(),
					() => macosTerminalInputSettingsController?.dispose(),
					() => applicationLocaleController.dispose(),
					() => agentIntegrationReminderController.dispose(),
					() => agentIntegrationManagementClient.dispose(),
					() => terminalLaunchProfileClient.dispose(),
					() => terminalDefaultSettingsClient.dispose(),
					() => userPreferences.dispose(),
					() => session.dispose(),
				],
				'Desktop Server Workbench cleanup failed.',
			);
		},
		{ once: true },
	);

	await userPreferences.refresh();
	runtime = createServerWorkbenchRuntime({
		applicationLocaleSource: applicationLocaleController,
		connection,
		defaultDirectory: bootstrap.defaultDirectory,
		documentElement,
		focusRecoveryStorage: storage.focusRecovery,
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
	});
	routing = createWebApplicationRouting(
		{
			actionRuntime: runtime.actionRuntime,
			agentIntegrationManagementControl: agentIntegrationManagementClient,
			agentIntegrationReminderController,
			agentObservationSource: runtime.agentObservationSource,
			applicationLocaleController,
			authority: runtime.authority,
			keyboardShortcutSettingsController,
			layoutStorage: storage.preferences,
			macosTerminalInputSettingsController,
			projectFolderAcquisition,
			projectWorkspaceRuntime: runtime.projectWorkspaceRuntime,
			shortcutPlatform,
			standaloneTerminalAvailable: false,
			terminalDefaultSettingsControl: terminalDefaultSettingsClient,
			terminalFeedbackSettingsController,
			terminalInteractionSettingsController,
			terminalLaunchProfileClient,
			terminalSettingsController,
			terminalSurface: ProjectedTerminalSurface,
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
		showSessionUnavailable(
			targetWindow,
			reactRoot,
			documentElement,
			preferredLocales,
			'unavailable',
		);
	});
}
