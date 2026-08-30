import { createOrchbeeIntl, workbenchTabMessages } from '@orchbee/i18n';
import type {
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchShortcutPlatform,
} from '@orchbee/workbench';
import type { WorkbenchClientSessionBootstrap } from '@orchbee/workbench-client-protocol';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

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
import {
	DesktopBrowserAccessClient,
	DesktopBrowserAccessController,
} from '../desktop-browser-access-dialog';
import { DesktopWorkbench } from '../desktop-workbench';
import { DesktopTerminalProjectionClient } from '../terminal-projection-control';
import { DesktopProjectFolderAcquisitionClient } from '../project-folder-acquisition';
import { DesktopProjectWorktreeClient } from '../project-worktree';
import { DesktopTerminalLaunchProfileClient } from '../terminal-launch-profiles';
import { DesktopTerminalDefaultSettingsClient } from '../terminal-default-settings';
import { DesktopRemoteTerminalCreationClient } from '../remote-terminal-creation';
import {
	DesktopSshProjectEnrollmentClient,
	DesktopSshProjectEnrollmentController,
} from '../desktop-ssh-project-enrollment-dialog';
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
	sshProjectEnrollmentAvailable: boolean,
	lifetimePort: MessagePort,
	clientSessionPort: MessagePort,
	projectionPort: MessagePort,
	agentObservationPort: MessagePort,
	agentIntegrationManagementPort: MessagePort,
	browserAccessPort: MessagePort,
	projectFolderAcquisitionPort: MessagePort,
	projectWorktreePort: MessagePort,
	terminalLaunchProfilePort: MessagePort,
	terminalDefaultSettingsPort: MessagePort,
	clipboardReadPort: MessagePort,
	remoteTerminalCreationPort: MessagePort,
	sshProjectEnrollmentPort: MessagePort,
	terminalFileDropCapability?: TerminalFileDropCapability,
): DesktopWorkbenchRendererSession {
	if (!(rootElement instanceof HTMLElement)) {
		throw new Error('The Orchbee Desktop application root element is missing.');
	}
	const applicationRoot = rootElement;
	const applicationLocaleController = new ApplicationLocaleController({
		documentElement,
		preferredLocales,
		storage: storage.preferences,
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
	const browserAccessController = new DesktopBrowserAccessController(
		new DesktopBrowserAccessClient(browserAccessPort),
		async (text) => {
			const clipboard = applicationRoot.ownerDocument.defaultView?.navigator.clipboard;
			if (clipboard === undefined) throw new Error('The Browser clipboard is unavailable.');
			await clipboard.writeText(text);
		},
	);
	const projectFolderAcquisitionClient = new DesktopProjectFolderAcquisitionClient(
		projectFolderAcquisitionPort,
	);
	const projectWorktreeClient = new DesktopProjectWorktreeClient(projectWorktreePort);
	const terminalLaunchProfileClient = new DesktopTerminalLaunchProfileClient(
		terminalLaunchProfilePort,
	);
	const terminalDefaultSettingsClient = new DesktopTerminalDefaultSettingsClient(
		terminalDefaultSettingsPort,
	);
	const remoteTerminalCreationClient = new DesktopRemoteTerminalCreationClient(
		remoteTerminalCreationPort,
	);
	const sshProjectEnrollmentClient = new DesktopSshProjectEnrollmentClient(
		sshProjectEnrollmentPort,
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
	const terminalInteractionSettingsController = new TerminalInteractionSettingsController(
		storage.preferences,
	);
	const terminalFeedbackSettingsController = new TerminalFeedbackSettingsController({
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
	let applicationLifetimeCoordinator: DesktopApplicationLifetimeCoordinator | undefined;
	let browserAccessReturnFocus: HTMLElement | null = null;
	let disposed = false;
	let lifetimeClient: DesktopApplicationLifetimeClient | undefined;
	let reactRoot: Root | undefined;
	let routing: WebApplicationRouting | undefined;
	const sshProjectEnrollmentController = new DesktopSshProjectEnrollmentController({
		client: sshProjectEnrollmentClient,
		onEnrollmentCommitted: async (result) => {
			if (
				!(await waitForEnrollmentPublication(
					runtime.projectWorkspaceRuntime,
					result.projectId,
					result.workspaceId,
				)) ||
				disposed
			) {
				return false;
			}
			routing?.coordinator.navigateTo({
				kind: 'workspace',
				projectId: result.projectId,
				workspaceId: result.workspaceId,
			});
			return routing !== undefined;
		},
		onFolderAttachmentCommitted: async (result) =>
			(await waitForFolderAttachmentPublication(
				runtime.projectWorkspaceRuntime,
				result.projectId,
				result.directoryId,
				result.canonicalPath,
			)) && !disposed,
	});

	const dispose = (): void => {
		if (disposed) return;
		disposed = true;
		applicationLifetimeCoordinator?.dispose();
		applicationLifetimeCoordinator = undefined;
		browserAccessController.dispose();
		agentIntegrationManagementClient.dispose();
		agentIntegrationReminderController.dispose();
		agentObservationClient.dispose();
		clipboardClient.dispose();
		reactRoot?.unmount();
		reactRoot = undefined;
		projectFolderAcquisitionClient.dispose();
		projectWorktreeClient.dispose();
		remoteTerminalCreationClient.dispose();
		sshProjectEnrollmentController.dispose();
		terminalLaunchProfileClient.dispose();
		terminalDefaultSettingsClient.dispose();
		routing?.dispose();
		routing = undefined;
		runtime.dispose();
		keyboardShortcutSettingsController.dispose();
		terminalSettingsController.dispose();
		terminalInteractionSettingsController.dispose();
		terminalFeedbackSettingsController.dispose();
		macosTerminalInputSettingsController?.dispose();
		applicationLocaleController.dispose();
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
			...(sshProjectEnrollmentAvailable
				? {
						environmentProjectConnection: sshProjectEnrollmentController,
						onAddProjectFolderViaSsh: (projectId: string) =>
							sshProjectEnrollmentController.openFolderAttachment(projectId),
						onConnectViaSsh: () => sshProjectEnrollmentController.openEnrollment(),
						sshConnectionManagementControl: sshProjectEnrollmentController,
					}
				: {}),
			keyboardShortcutSettingsController,
			layoutStorage: storage.preferences,
			macosTerminalInputSettingsController,
			projectWorkspaceRuntime: workbenchClientRuntime.projectWorkspaceRuntime,
			projectFolderAcquisition: projectFolderAcquisitionClient,
			projectWorktree: projectWorktreeClient,
			remoteTerminalCreation: remoteTerminalCreationClient,
			requestBrowserAccess: (returnFocus) => {
				browserAccessReturnFocus = returnFocus;
				browserAccessController.open();
			},
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
				browserAccessFinalFocus: () => {
					const returnFocus = browserAccessReturnFocus;
					browserAccessReturnFocus = null;
					return returnFocus?.isConnected === true ? returnFocus : null;
				},
				browserAccessController,
				router: nextRouting.router,
				sshProjectEnrollmentController,
				terminalSurfaceCapability,
				windowControls: shortcutPlatform === 'apple' ? 'leading' : 'trailing',
			}),
		);
		return { kind: 'ready' };
	}
}

function waitForEnrollmentPublication(
	runtime: WorkbenchProjectWorkspaceRuntime,
	projectId: string,
	workspaceId: string,
): Promise<boolean> {
	return waitForWorkbenchPublication(
		runtime,
		() =>
			runtime
				.getSnapshot()
				.projects.find((project) => project.id === projectId)
				?.workspaces.some((workspace) => workspace.id === workspaceId) ?? false,
	);
}

function waitForFolderAttachmentPublication(
	runtime: WorkbenchProjectWorkspaceRuntime,
	projectId: string,
	directoryId: string,
	canonicalPath: string,
): Promise<boolean> {
	return waitForWorkbenchPublication(
		runtime,
		() =>
			runtime
				.getSnapshot()
				.projects.find((project) => project.id === projectId)
				?.directories.some(
					(directory) =>
						directory.id === directoryId &&
						directory.path === canonicalPath &&
						directory.placement.kind === 'environment',
				) ?? false,
	);
}

function waitForWorkbenchPublication(
	runtime: WorkbenchProjectWorkspaceRuntime,
	published: () => boolean,
): Promise<boolean> {
	if (published()) return Promise.resolve(true);
	return new Promise((resolve) => {
		let settled = false;
		let timeout: ReturnType<typeof setTimeout> | undefined;
		let unsubscribe = (): void => undefined;
		const settle = (result: boolean): void => {
			if (settled) return;
			settled = true;
			if (timeout !== undefined) clearTimeout(timeout);
			unsubscribe();
			resolve(result);
		};
		unsubscribe = runtime.subscribe(() => {
			if (published()) settle(true);
		});
		timeout = setTimeout(() => settle(false), 5_000);
		if (published()) settle(true);
	});
}
