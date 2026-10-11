import {
	DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE,
	DESKTOP_RENDERER_PORT_ROLES,
	DesktopRendererBootstrapWindowMessageSchema,
} from '@orchbee/desktop-client-protocol';
import type { WorkbenchShortcutPlatform } from '@orchbee/workbench';

import {
	startDesktopWorkbenchRenderer,
	type DesktopWorkbenchRendererSession,
	type DesktopWorkbenchRendererStorage,
} from './startDesktopWorkbenchRenderer';
import { resolveDesktopTerminalFileDropCapability } from '../terminal-file-drop';

export interface DesktopWebApplicationSession {
	dispose(): void;
}

export interface DesktopWebApplicationDependencies {
	readonly startRenderer: typeof startDesktopWorkbenchRenderer;
}

const defaultDependencies: DesktopWebApplicationDependencies = {
	startRenderer: startDesktopWorkbenchRenderer,
};

type RendererLifecycle =
	| { readonly kind: 'waiting' }
	| { readonly kind: 'starting' }
	| { readonly kind: 'running'; readonly session: DesktopWorkbenchRendererSession }
	| { readonly kind: 'disposed' };

export function startDesktopWebApplication(
	targetWindow: Window,
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dataset' | 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	shortcutPlatform: WorkbenchShortcutPlatform,
	storage: DesktopWorkbenchRendererStorage,
	dependencies: DesktopWebApplicationDependencies = defaultDependencies,
): DesktopWebApplicationSession {
	let lifecycle: RendererLifecycle = { kind: 'waiting' };
	const isDisposed = (): boolean => lifecycle.kind === 'disposed';

	const dispose = (): void => {
		const retired = lifecycle;
		if (retired.kind === 'disposed') return;
		lifecycle = { kind: 'disposed' };
		targetWindow.removeEventListener('message', handleMessage);
		targetWindow.removeEventListener('unload', handleUnload);
		if (retired.kind === 'running') retired.session.dispose();
	};
	const handleUnload = (): void => dispose();
	const handleMessage = (event: MessageEvent<unknown>): void => {
		if (event.source !== targetWindow || lifecycle.kind !== 'waiting') {
			closeMessagePorts(event.ports);
			return;
		}
		if (
			!isDesktopRendererBootstrapCandidate(event.data) ||
			event.ports.length !== DESKTOP_RENDERER_PORT_ROLES.length
		) {
			closeMessagePorts(event.ports);
			return;
		}
		const message = DesktopRendererBootstrapWindowMessageSchema.safeParse(event.data);
		const clientSessionPort = event.ports[0];
		const projectionPort = event.ports[1];
		const agentObservationPort = event.ports[2];
		const agentIntegrationManagementPort = event.ports[3];
		const lifetimePort = event.ports[4];
		const projectFolderAcquisitionPort = event.ports[5];
		const projectWorktreePort = event.ports[6];
		const terminalLaunchProfilePort = event.ports[7];
		const terminalDefaultSettingsPort = event.ports[8];
		const clipboardReadPort = event.ports[9];
		const userPreferencesPort = event.ports[10];
		if (
			!message.success ||
			!message.data.security.contextIsolated ||
			!message.data.security.sandboxed ||
			clientSessionPort === undefined ||
			projectionPort === undefined ||
			agentObservationPort === undefined ||
			agentIntegrationManagementPort === undefined ||
			lifetimePort === undefined ||
			projectFolderAcquisitionPort === undefined ||
			projectWorktreePort === undefined ||
			terminalLaunchProfilePort === undefined ||
			terminalDefaultSettingsPort === undefined ||
			clipboardReadPort === undefined ||
			userPreferencesPort === undefined
		) {
			closeMessagePorts(event.ports);
			dispose();
			return;
		}
		lifecycle = { kind: 'starting' };
		try {
			const terminalFileDropCapability = resolveDesktopTerminalFileDropCapability(targetWindow);
			if (isDisposed()) {
				closeMessagePorts(event.ports);
				return;
			}
			const rendererSession = dependencies.startRenderer(
				rootElement,
				documentElement,
				preferredLocales,
				shortcutPlatform,
				storage,
				message.data.bootstrap.clientSession,
				lifetimePort,
				clientSessionPort,
				projectionPort,
				agentObservationPort,
				agentIntegrationManagementPort,
				projectFolderAcquisitionPort,
				projectWorktreePort,
				terminalLaunchProfilePort,
				terminalDefaultSettingsPort,
				clipboardReadPort,
				userPreferencesPort,
				terminalFileDropCapability,
			);
			if (isDisposed()) {
				rendererSession.dispose();
				return;
			}
			lifecycle = { kind: 'running', session: rendererSession };
			if (rootElement instanceof HTMLElement) {
				rootElement.dataset['desktopContextIsolated'] = 'true';
				rootElement.dataset['desktopSandboxed'] = 'true';
			}
		} catch {
			closeMessagePorts(event.ports);
			dispose();
		}
	};

	targetWindow.addEventListener('message', handleMessage);
	targetWindow.addEventListener('unload', handleUnload);
	return { dispose };
}

function closeMessagePorts(ports: readonly MessagePort[]): void {
	for (const port of ports) port.close();
}

function isDesktopRendererBootstrapCandidate(value: unknown): boolean {
	return (
		typeof value === 'object' &&
		value !== null &&
		'type' in value &&
		value.type === DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE
	);
}
