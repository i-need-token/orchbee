import type { GatewayBrowserConnection } from '@orchbee/gateway-protocol';
import type { WorkbenchShortcutPlatform } from '@orchbee/workbench';
import { createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

import {
	createWorkbenchClientRuntime,
	WorkbenchClientApplication,
	WorkbenchClientSessionClient,
	type WorkbenchClientRuntime,
} from '../../workbench-client';
import { connectGatewayControlConnection, type GatewayControlConnection } from '../connection';
import { createBrowserTerminalPlatformEffects } from '../platform-effects';
import { GatewayTerminalProjectionClient } from '../terminal-projection';

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
	) => GatewayTerminalProjectionClient;
	readonly createReactRoot: (element: Element) => Root;
	readonly createRuntime: typeof createWorkbenchClientRuntime;
}

const defaultDependencies: GatewayWebApplicationDependencies = {
	connectControl: connectGatewayControlConnection,
	createProjectionClient: (control, connection) =>
		new GatewayTerminalProjectionClient(control, connection),
	createReactRoot: (element) => createRoot(element),
	createRuntime: createWorkbenchClientRuntime,
};

export function startGatewayWebApplication(
	targetWindow: Window,
	rootElement: Element | null,
	documentElement: Pick<HTMLElement, 'dir' | 'lang'>,
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
	let control: GatewayControlConnection | undefined;
	let disposed = false;
	let reactRoot: Root | undefined;
	let runtime: WorkbenchClientRuntime | undefined;
	const dispose = (): void => {
		if (disposed) return;
		disposed = true;
		reactRoot?.unmount();
		reactRoot = undefined;
		runtime?.dispose();
		runtime = undefined;
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
		const nextControl = await dependencies.connectControl(connection);
		if (disposed) {
			nextControl.close();
			return { kind: 'unavailable', reason: 'control-unavailable' };
		}
		control = nextControl;
		const projectionClient = dependencies.createProjectionClient(nextControl, connection);
		const clientSession = new WorkbenchClientSessionClient(
			{
				close: () => nextControl.close(),
				send: (command) => nextControl.sendClientSession(command),
				subscribe: (listener, unavailable) =>
					nextControl.subscribeClientSession(listener, unavailable),
			},
			nextControl.bootstrap,
		);
		const clipboard = targetWindow.navigator.clipboard;
		const platformEffects = createBrowserTerminalPlatformEffects({
			openExternal: (target) => {
				const anchor = targetWindow.document.createElement('a');
				anchor.href = target;
				anchor.rel = 'noopener noreferrer';
				anchor.target = '_blank';
				anchor.click();
			},
			...(clipboard === undefined
				? {}
				: { writeClipboardText: async (text: string) => clipboard.writeText(text) }),
		});
		const nextRuntime = dependencies.createRuntime({
			clientSession,
			documentElement,
			preferredLocales,
			projectionClient,
			shortcutPlatform,
			terminalPlatformEffects: platformEffects,
		});
		runtime = nextRuntime;
		const runtimeStart = await nextRuntime.ready;
		if (runtimeStart.kind === 'unavailable') {
			dispose();
			return runtimeStart;
		}
		if (disposed) return { kind: 'unavailable', reason: 'control-unavailable' };
		reactRoot = dependencies.createReactRoot(applicationRoot);
		reactRoot.render(
			createElement(WorkbenchClientApplication, {
				actionRuntime: nextRuntime.actionRuntime,
				layoutStorage: storage,
				locale: nextRuntime.locale,
				projectWorkspaceRuntime: nextRuntime.projectWorkspaceRuntime,
				shortcutPlatform,
				terminalSurfaceCapability: nextRuntime.terminalSurfaceCapability,
			}),
		);
		return { kind: 'ready' };
	}
}
