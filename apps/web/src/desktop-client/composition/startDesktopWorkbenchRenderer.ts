import { createOrchbeeIntl, workbenchTabMessages } from '@orchbee/i18n';
import type { WorkbenchShortcutPlatform } from '@orchbee/workbench';
import type { WorkbenchClientSessionBootstrap } from '@orchbee/workbench-client-protocol';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import { runApplicationCleanup } from '../../runApplicationCleanup';
import { createWebApplicationRouting, type WebApplicationRouting } from '../../application-routing';
import { ApplicationLocaleController } from '../../application-locale';
import { AgentIntegrationReminderController } from '../../agent-integration-management';
import { KeyboardShortcutSettingsController } from '../../keyboard-shortcuts';
import { MacosTerminalInputSettingsController } from '../../macos-terminal-input-settings';
import { TerminalInteractionSettingsController } from '../../terminal-interaction-settings';
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
import { DesktopAgentIntegrationManagementClient } from '../agent-integration-management';
import { DesktopAgentObservationClient } from '../agent-observation';
import { createDesktopTerminalPlatformEffects, DesktopClipboardClient } from '../desktop-clipboard';
import {
	DesktopApplicationLifetimeClient,
	DesktopApplicationLifetimeCoordinator,
} from '../desktop-application-lifetime-dialog';

import { DesktopWorkbench } from '../desktop-workbench';
import { DesktopTerminalProjectionClient } from '../terminal-projection-control';
import {
	DesktopProjectFolderAcquisitionClient,
	confirmProjectFolderCreation,
} from '../project-folder-acquisition';
import { DesktopProjectWorktreeClient } from '../project-worktree';
import { DesktopTerminalLaunchProfileClient } from '../terminal-launch-profiles';
import { DesktopTerminalDefaultSettingsClient } from '../terminal-default-settings';
import { DesktopUserPreferencesClient, readDesktopLegacyPreferences } from '../user-preferences';

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
	agentObservationPort: MessagePort,
	agentIntegrationManagementPort: MessagePort,
	projectFolderAcquisitionPort: MessagePort,
	projectWorktreePort: MessagePort,
	terminalLaunchProfilePort: MessagePort,
	terminalDefaultSettingsPort: MessagePort,
	clipboardReadPort: MessagePort,
	userPreferencesPort: MessagePort,
	terminalFileDropCapability?: TerminalFileDropCapability,
): DesktopWorkbenchRendererSession {
	if (!(rootElement instanceof HTMLElement)) {
		throw new Error('The Orchbee Desktop application root element is missing.');
	}
	const applicationRoot = rootElement;
	const userPreferences = new DesktopUserPreferencesClient(userPreferencesPort);
	const applicationLocaleController = new ApplicationLocaleController({
		documentElement,
		preferredLocales,
		storage: storage.preferences,
		userPreferences,
	});
	const clientSession = new DesktopClientSessionClient(clientSessionPort, bootstrap);
	const agentIntegrationManagementClient = new DesktopAgentIntegrationManagementClient(
		agentIntegrationManagementPort,
	);
	const agentObservationClient = new DesktopAgentObservationClient(agentObservationPort);
	const agentIntegrationReminderController = new AgentIntegrationReminderController(
		storage.preferences,
	);
	const clipboardClient = new DesktopClipboardClient(clipboardReadPort);
	const projectionClient = new DesktopTerminalProjectionClient(projectionPort);
	const projectWorktreeClient = new DesktopProjectWorktreeClient(projectWorktreePort);
	const terminalLaunchProfileClient = new DesktopTerminalLaunchProfileClient(
		terminalLaunchProfilePort,
		undefined,
		userPreferences,
	);
	const terminalDefaultSettingsClient = new DesktopTerminalDefaultSettingsClient(
		terminalDefaultSettingsPort,
		undefined,
		userPreferences,
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
		userPreferences,
		...(applicationRoot.ownerDocument.defaultView?.matchMedia === undefined
			? {}
			: {
					systemTheme: applicationRoot.ownerDocument.defaultView.matchMedia(
						'(prefers-color-scheme: dark)',
					),
				}),
	});
	const terminalInteractionSettingsController = new TerminalInteractionSettingsController(
		storage.preferences,
		userPreferences,
	);
	const terminalFeedbackSettingsController = new TerminalFeedbackSettingsController({
		userPreferences,
		notificationCapability:
			applicationRoot.ownerDocument.defaultView === null
				? UNSUPPORTED_TERMINAL_NOTIFICATION_CAPABILITY
				: createBrowserTerminalNotificationCapability(applicationRoot.ownerDocument.defaultView),
		storage: storage.preferences,
	});
	const runtime = createWorkbenchClientRuntime({
		applicationLocaleSource: applicationLocaleController,
		clientSession,
		documentElement,
		...(storage.focusRecovery === undefined ? {} : { focusRecoveryStorage: storage.focusRecovery }),
		preferredLocales,
		projectionClient,
		shortcutBindingSource: keyboardShortcutSettingsController,
		shortcutPlatform,
		terminalPresentationConfigurationSource: terminalSettingsController,
		terminalInteractionConfigurationSource: terminalInteractionSettingsController,
		terminalTrailingBlankRowsPreferenceSource:
			terminalInteractionSettingsController.trailingBlankRowsPreferenceSource,
		terminalCompletionReminderObserver: terminalFeedbackSettingsController,
		terminalPlatformEffects: createDesktopTerminalPlatformEffects(clipboardClient),
		...(macosTerminalInputSettingsController === undefined
			? {}
			: { terminalInputConfigurationSource: macosTerminalInputSettingsController }),
		...(terminalFileDropCapability === undefined ? {} : { terminalFileDropCapability }),
	});
	const projectFolderAcquisitionClient = new DesktopProjectFolderAcquisitionClient(
		projectFolderAcquisitionPort,
		undefined,
		(receipt) =>
			confirmProjectFolderCreation(
				runtime.documentCoordinator,
				runtime.projectWorkspaceRuntime,
				receipt,
			),
	);

	let applicationLifetimeCoordinator: DesktopApplicationLifetimeCoordinator | undefined;

	let disposed = false;
	let lifetimeClient: DesktopApplicationLifetimeClient | undefined;
	let reactRoot: Root | undefined;
	let routing: WebApplicationRouting | undefined;

	const dispose = (): void => {
		if (disposed) return;
		disposed = true;
		const retiredLifetimeCoordinator = applicationLifetimeCoordinator;
		applicationLifetimeCoordinator = undefined;
		const retiredRoot = reactRoot;
		reactRoot = undefined;
		const retiredRouting = routing;
		routing = undefined;
		const retiredLifetimeClient = lifetimeClient;
		lifetimeClient = undefined;
		runApplicationCleanup(
			[
				() => retiredLifetimeCoordinator?.dispose(),
				() => agentIntegrationManagementClient.dispose(),
				() => agentIntegrationReminderController.dispose(),
				() => agentObservationClient.dispose(),
				() => clipboardClient.dispose(),
				() => retiredRoot?.unmount(),
				() => projectFolderAcquisitionClient.dispose(),
				() => projectWorktreeClient.dispose(),
				() => terminalLaunchProfileClient.dispose(),
				() => terminalDefaultSettingsClient.dispose(),
				() => retiredRouting?.dispose(),
				() => runtime.dispose(),
				() => keyboardShortcutSettingsController.dispose(),
				() => terminalSettingsController.dispose(),
				() => terminalInteractionSettingsController.dispose(),
				() => terminalFeedbackSettingsController.dispose(),
				() => macosTerminalInputSettingsController?.dispose(),
				() => applicationLocaleController.dispose(),
				() => userPreferences.dispose(),
				() => {
					if (retiredLifetimeClient === undefined) lifetimePort.close();
					else retiredLifetimeClient.dispose();
				},
			],
			'Desktop Workbench renderer cleanup failed.',
		);
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
		await userPreferences.initialize(() => readDesktopLegacyPreferences(storage.preferences));
		if (disposed) return { kind: 'unavailable', reason: 'control-unavailable' };
		const [
			runtimeStart,
			agentIntegrationManagementStart,
			agentObservationStart,
			terminalLaunchProfilesStart,
			terminalDefaultSettingsStart,
		] = await Promise.all([
			workbenchClientRuntime.ready,
			agentIntegrationManagementClient.initialize(),
			agentObservationClient.initialize(),
			terminalLaunchProfileClient.initialize(),
			terminalDefaultSettingsClient.initialize(),
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
		const getInitialTerminalTitle = (): string =>
			createOrchbeeIntl(applicationLocaleController.current()).formatMessage(
				workbenchTabMessages.terminalTitle,
			);
		const nextRouting = createWebApplicationRouting({
			actionRuntime: workbenchClientRuntime.actionRuntime,
			applicationLocaleController,
			...(agentIntegrationManagementStart.kind === 'ready'
				? {
						agentIntegrationManagementControl: agentIntegrationManagementClient,
						agentIntegrationReminderController,
					}
				: {}),
			...(agentObservationStart.kind === 'ready'
				? { agentObservationSource: agentObservationClient }
				: {}),
			getInitialTerminalTitle,
			keyboardShortcutSettingsController,
			layoutStorage: storage.preferences,
			macosTerminalInputSettingsController,
			projectWorkspaceRuntime: workbenchClientRuntime.projectWorkspaceRuntime,
			projectFolderAcquisition: projectFolderAcquisitionClient,
			projectWorktree: projectWorktreeClient,

			shortcutPlatform,
			terminalSettingsController,
			terminalInteractionSettingsController,
			terminalFeedbackSettingsController,
			...(terminalLaunchProfilesStart.kind === 'ready' ? { terminalLaunchProfileClient } : {}),
			...(terminalDefaultSettingsStart.kind === 'ready'
				? { terminalDefaultSettingsControl: terminalDefaultSettingsClient }
				: {}),
			terminalSurface: ProjectedTerminalSurface,
		});
		routing = nextRouting;
		const terminalSurfaceCapability =
			agentObservationStart.kind === 'ready'
				? {
						...workbenchClientRuntime.terminalSurfaceCapability,
						agentObservationSource: agentObservationClient,
					}
				: workbenchClientRuntime.terminalSurfaceCapability;
		reactRoot = createRoot(applicationRoot);
		reactRoot.render(
			createElement(DesktopWorkbench, {
				applicationLifetimeCoordinator,
				applicationLocaleSource: applicationLocaleController,
				router: nextRouting.router,
				terminalSurfaceCapability,
				windowControls: shortcutPlatform === 'apple' ? 'leading' : 'trailing',
			}),
		);

		return { kind: 'ready' };
	}
}
