/**
 * The Workbench as a Server Client: one Client connection, through Main's relay, carries everything
 * this document reads and writes. It replaces the Main-owned renderer once that path is removed.
 */
import {
	DESKTOP_SERVER_CONNECTION_WINDOW_REQUEST,
	DesktopServerConnectionWindowMessageSchema,
	type DesktopServerConnectionBootstrap,
} from '@orchbee/desktop-client-protocol';
import { createOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import { normalizePermittedExternalTerminalUrl } from '@orchbee/terminal';
import type { WorkbenchShortcutPlatform } from '@orchbee/workbench';
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
	createServerWorkbenchRuntime,
	ProjectedTerminalSurface,
	ServerConnection,
	ServerUserPreferencesClient,
	ServerWorkbenchSession,
	workspaceLaunchAction,
	type ProjectedTerminalPlatformEffects,
	type ServerWorkbenchRuntime,
} from '../../workbench-client';
import { DesktopWorkbench } from '../desktop-workbench';
import { MessagePortServerCarrier } from './MessagePortServerCarrier';

interface DesktopServerWorkbenchStorage {
	readonly focusRecovery?: Pick<Storage, 'getItem' | 'removeItem' | 'setItem'> | undefined;
	readonly preferences: Pick<Storage, 'getItem' | 'setItem'>;
}

function receiveConnection(
	targetWindow: Window,
): Promise<{ readonly bootstrap: DesktopServerConnectionBootstrap; readonly port: MessagePort }> {
	return new Promise((resolve, reject) => {
		const handleMessage = (event: MessageEvent<unknown>): void => {
			const port = event.ports[0];
			if (event.source !== targetWindow || port === undefined) return;
			targetWindow.removeEventListener('message', handleMessage);
			const message = DesktopServerConnectionWindowMessageSchema.safeParse(event.data);
			if (message.success) resolve({ bootstrap: message.data.bootstrap, port });
			else reject(new Error('The Server connection announcement is invalid.'));
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

async function run(
	targetWindow: Window,
	rootElement: HTMLElement,
	documentElement: Pick<HTMLElement, 'dataset' | 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	shortcutPlatform: WorkbenchShortcutPlatform,
	storage: DesktopServerWorkbenchStorage,
): Promise<void> {
	const { bootstrap, port } = await receiveConnection(targetWindow);
	const connection = new ServerConnection(new MessagePortServerCarrier(port));
	targetWindow.addEventListener('unload', () => connection.close(), { once: true });
	const session = await ServerWorkbenchSession.open(connection);
	const stopSession = (reason: string): void => {
		// Recovering from this is the connection lifecycle's job; until then a developer reads the reason.
		documentElement.dataset['serverWorkbenchEnded'] = reason;
	};
	session.onEnded((end) => stopSession(end.kind));

	const userPreferences = new ServerUserPreferencesClient(connection, session.snapshot.sessionId);
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
	const terminalSettingsController = new TerminalSettingsController({
		storage: storage.preferences,
		systemTheme: targetWindow.matchMedia('(prefers-color-scheme: dark)'),
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

	let reactRoot: Root | undefined;
	let routing: WebApplicationRouting | undefined;
	let runtime: ServerWorkbenchRuntime | undefined;
	targetWindow.addEventListener(
		'unload',
		() => {
			runApplicationCleanup(
				[
					() => reactRoot?.unmount(),
					() => routing?.dispose(),
					() => runtime?.dispose(),
					() => keyboardShortcutSettingsController.dispose(),
					() => terminalSettingsController.dispose(),
					() => terminalInteractionSettingsController.dispose(),
					() => terminalFeedbackSettingsController.dispose(),
					() => macosTerminalInputSettingsController?.dispose(),
					() => applicationLocaleController.dispose(),
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
		onPublicationFailure: (error) => {
			stopSession(String(error));
			session.dispose();
		},
		preferredLocales,
		session,
		shortcutBindingSource: keyboardShortcutSettingsController,
		shortcutPlatform,
		terminalCompletionReminderObserver: terminalFeedbackSettingsController,
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
			applicationLocaleController,
			authority: runtime.authority,
			keyboardShortcutSettingsController,
			layoutStorage: storage.preferences,
			macosTerminalInputSettingsController,
			projectWorkspaceRuntime: runtime.projectWorkspaceRuntime,
			shortcutPlatform,
			standaloneTerminalAvailable: false,
			terminalFeedbackSettingsController,
			terminalInteractionSettingsController,
			terminalSettingsController,
			terminalSurface: ProjectedTerminalSurface,
		},
		// A window has no address bar, and a reload needs the URL Main loaded it with. Focus recovery
		// returns it to where it was.
		createMemoryHistory(),
	);
	reactRoot = createRoot(rootElement);
	reactRoot.render(
		createElement(DesktopWorkbench, {
			applicationLocaleSource: applicationLocaleController,
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
	documentElement: Pick<HTMLElement, 'dataset' | 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	shortcutPlatform: WorkbenchShortcutPlatform,
	storage: DesktopServerWorkbenchStorage,
): void {
	if (!(rootElement instanceof HTMLElement)) {
		throw new Error('The Orchbee Desktop application root element is missing.');
	}
	void run(
		targetWindow,
		rootElement,
		documentElement,
		preferredLocales,
		shortcutPlatform,
		storage,
	).catch((error: unknown) => {
		documentElement.dataset['serverWorkbenchFailure'] = String(error);
	});
}
