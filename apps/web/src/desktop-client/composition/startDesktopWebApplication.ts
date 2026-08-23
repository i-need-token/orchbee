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

export function startDesktopWebApplication(
	targetWindow: Window,
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dataset' | 'dir' | 'lang'>,
	preferredLocales: readonly string[],
	shortcutPlatform: WorkbenchShortcutPlatform,
	storage: DesktopWorkbenchRendererStorage,
	dependencies: DesktopWebApplicationDependencies = defaultDependencies,
): DesktopWebApplicationSession {
	let disposed = false;
	let rendererSession: DesktopWorkbenchRendererSession | undefined;

	const dispose = (): void => {
		if (disposed) return;
		disposed = true;
		targetWindow.removeEventListener('message', handleMessage);
		targetWindow.removeEventListener('unload', handleUnload);
		rendererSession?.dispose();
		rendererSession = undefined;
	};
	const handleUnload = (): void => dispose();
	const handleMessage = (event: MessageEvent<unknown>): void => {
		if (event.source !== targetWindow || disposed) {
			closeMessagePorts(event.ports);
			return;
		}
		if (
			!isDesktopRendererBootstrapCandidate(event.data) ||
			rendererSession !== undefined ||
			event.ports.length !== DESKTOP_RENDERER_PORT_ROLES.length
		) {
			closeMessagePorts(event.ports);
			return;
		}
		const message = DesktopRendererBootstrapWindowMessageSchema.safeParse(event.data);
		const clientSessionPort = event.ports[0];
		const projectionPort = event.ports[1];
		const lifetimePort = event.ports[2];
		const browserAccessPort = event.ports[3];
		const projectFolderAcquisitionPort = event.ports[4];
		const projectWorktreePort = event.ports[5];
		const terminalLaunchProfilePort = event.ports[6];
		const terminalDefaultSettingsPort = event.ports[7];
		if (
			!message.success ||
			!message.data.security.contextIsolated ||
			!message.data.security.sandboxed ||
			clientSessionPort === undefined ||
			projectionPort === undefined ||
			lifetimePort === undefined ||
			browserAccessPort === undefined ||
			projectFolderAcquisitionPort === undefined ||
			projectWorktreePort === undefined ||
			terminalLaunchProfilePort === undefined ||
			terminalDefaultSettingsPort === undefined
		) {
			closeMessagePorts(event.ports);
			dispose();
			return;
		}
		try {
			const terminalFileDropCapability = resolveDesktopTerminalFileDropCapability(targetWindow);
			rendererSession = dependencies.startRenderer(
				rootElement,
				documentElement,
				preferredLocales,
				shortcutPlatform,
				storage,
				message.data.bootstrap.clientSession,
				lifetimePort,
				clientSessionPort,
				projectionPort,
				browserAccessPort,
				projectFolderAcquisitionPort,
				projectWorktreePort,
				terminalLaunchProfilePort,
				terminalDefaultSettingsPort,
				terminalFileDropCapability,
			);
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
