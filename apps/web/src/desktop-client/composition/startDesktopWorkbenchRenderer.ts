import {
	DesktopWindowLayoutDestinationSchema,
	DesktopWindowLayoutDragSessionIdSchema,
} from '@orchbee/desktop-client-protocol';
import type { WorkbenchShortcutPlatform, WorkbenchTabWindowControl } from '@orchbee/workbench';
import type { WorkbenchClientSessionBootstrap } from '@orchbee/workbench-client-protocol';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import { createWebApplicationRouting, type WebApplicationRouting } from '../../application-routing';
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
import { DesktopWindowLayoutClient } from '../window-layout';
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
	projectWorktreePort: MessagePort,
	terminalLaunchProfilePort: MessagePort,
	terminalDefaultSettingsPort: MessagePort,
	clipboardReadPort: MessagePort,
	windowLayoutPort: MessagePort,
	terminalFileDropCapability?: TerminalFileDropCapability,
): DesktopWorkbenchRendererSession {
	if (!(rootElement instanceof HTMLElement)) {
		throw new Error('The Orchbee Desktop application root element is missing.');
	}
	const applicationRoot = rootElement;
	const clientSession = new DesktopClientSessionClient(clientSessionPort, bootstrap);
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
	const windowLayoutClient = new DesktopWindowLayoutClient(windowLayoutPort);
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
		clientSession,
		documentElement,
		...(storage.focusRecovery === undefined ? {} : { focusRecoveryStorage: storage.focusRecovery }),
		preferredLocales,
		projectionClient,
		shortcutBindingSource: keyboardShortcutSettingsController,
		shortcutPlatform,
		terminalPresentationConfigurationSource: terminalSettingsController,
		terminalInteractionConfigurationSource: terminalInteractionSettingsController,
		terminalCompletionReminderObserver: terminalFeedbackSettingsController,
		terminalPlatformEffects: createDesktopTerminalPlatformEffects(clipboardClient),
		...(macosTerminalInputSettingsController === undefined
			? {}
			: { terminalInputConfigurationSource: macosTerminalInputSettingsController }),
		...(terminalFileDropCapability === undefined ? {} : { terminalFileDropCapability }),
	});
	let applicationLifetimeCoordinator: DesktopApplicationLifetimeCoordinator | undefined;
	let activeContextUnsubscribe: (() => void) | undefined;
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
		activeContextUnsubscribe?.();
		activeContextUnsubscribe = undefined;
		browserAccessController.dispose();
		clipboardClient.dispose();
		reactRoot?.unmount();
		reactRoot = undefined;
		projectFolderAcquisitionClient.dispose();
		projectWorktreeClient.dispose();
		terminalLaunchProfileClient.dispose();
		terminalDefaultSettingsClient.dispose();
		windowLayoutClient.dispose();
		routing?.dispose();
		routing = undefined;
		runtime.dispose();
		keyboardShortcutSettingsController.dispose();
		terminalSettingsController.dispose();
		terminalInteractionSettingsController.dispose();
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
		const [
			runtimeStart,
			terminalLaunchProfilesStart,
			terminalDefaultSettingsStart,
			windowLayoutStart,
		] = await Promise.all([
			workbenchClientRuntime.ready,
			terminalLaunchProfileClient.initialize(),
			terminalDefaultSettingsClient.initialize(),
			windowLayoutClient.initialize(),
		]);
		if (runtimeStart.kind === 'unavailable') {
			dispose();
			return runtimeStart;
		}
		if (windowLayoutStart.kind === 'unavailable') {
			dispose();
			return { kind: 'unavailable', reason: 'window-layout-unavailable' };
		}
		if (disposed) return { kind: 'unavailable', reason: 'control-unavailable' };
		let activeTerminalContextId: string | undefined;
		const publishActiveTerminalContext = () => {
			const nextTerminalContextId =
				workbenchClientRuntime.projectWorkspaceRuntime.getActiveTerminalContext().terminalContextId;
			if (nextTerminalContextId === activeTerminalContextId) return;
			activeTerminalContextId = nextTerminalContextId;
			windowLayoutClient.setActiveTerminalContext(nextTerminalContextId);
		};
		publishActiveTerminalContext();
		activeContextUnsubscribe = workbenchClientRuntime.projectWorkspaceRuntime.subscribe(
			publishActiveTerminalContext,
		);
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
			projectWorktree: projectWorktreeClient,
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
			tabScope: windowLayoutClient,
			tabWindowControl: {
				finishDrag: (sessionId, input) =>
					windowLayoutClient.finishDrag(
						DesktopWindowLayoutDragSessionIdSchema.parse(sessionId),
						input,
					),
				getDragTarget: (sessionId) =>
					windowLayoutClient.getDragTarget(DesktopWindowLayoutDragSessionIdSchema.parse(sessionId)),
				getSnapshot: windowLayoutClient.getSnapshot,
				moveGroup: (terminalContextId, groupId, destination) =>
					windowLayoutClient.moveGroup(
						terminalContextId,
						groupId,
						DesktopWindowLayoutDestinationSchema.parse(destination),
					),
				moveTabs: (terminalContextId, tabIds, destination) =>
					windowLayoutClient.moveTabs(
						terminalContextId,
						tabIds,
						DesktopWindowLayoutDestinationSchema.parse(destination),
					),
				startDrag: (terminalContextId, tabIds) =>
					windowLayoutClient.startDrag(terminalContextId, tabIds),
				subscribe: windowLayoutClient.subscribe,
				updateDrag: (sessionId, point) =>
					windowLayoutClient.updateDrag(
						DesktopWindowLayoutDragSessionIdSchema.parse(sessionId),
						point,
					),
			} satisfies WorkbenchTabWindowControl,
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
