import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
import '@orchbee/workbench/styles.css';

import {
	DesktopClientSessionWindowMessageSchema,
	DesktopApplicationLifetimeWindowMessageSchema,
	TerminalProjectionControlWindowMessageSchema,
	type DesktopClientSessionBootstrap,
} from '../../shared';
import { resolveDesktopShortcutPlatform } from '../terminal-actions';
import {
	startDesktopWorkbenchRenderer,
	type DesktopWorkbenchRendererSession,
} from './startDesktopWorkbenchRenderer';

interface ClientSessionReady {
	readonly bootstrap: DesktopClientSessionBootstrap;
	readonly port: MessagePort;
}

let clientSession: ClientSessionReady | undefined;
let lifetimeControlPort: MessagePort | undefined;
let projectionControlPort: MessagePort | undefined;
let session: DesktopWorkbenchRendererSession | undefined;

window.addEventListener('message', (event) => {
	if (event.source !== window || event.ports.length !== 1 || session !== undefined) return;
	const port = event.ports[0];
	if (port === undefined) return;
	const clientSessionMessage = DesktopClientSessionWindowMessageSchema.safeParse(event.data);
	if (clientSessionMessage.success) {
		if (
			clientSession !== undefined ||
			!clientSessionMessage.data.security.contextIsolated ||
			!clientSessionMessage.data.security.sandboxed
		) {
			port.close();
			return;
		}
		clientSession = { bootstrap: clientSessionMessage.data.bootstrap, port };
		const root = document.querySelector('#root');
		if (root instanceof HTMLElement) {
			root.dataset['desktopContextIsolated'] = String(
				clientSessionMessage.data.security.contextIsolated,
			);
			root.dataset['desktopSandboxed'] = String(clientSessionMessage.data.security.sandboxed);
		}
	} else {
		const lifetimeMessage = DesktopApplicationLifetimeWindowMessageSchema.safeParse(event.data);
		if (lifetimeMessage.success) {
			if (
				lifetimeControlPort !== undefined ||
				!lifetimeMessage.data.security.contextIsolated ||
				!lifetimeMessage.data.security.sandboxed
			) {
				port.close();
				return;
			}
			lifetimeControlPort = port;
		} else {
			const projectionMessage = TerminalProjectionControlWindowMessageSchema.safeParse(event.data);
			if (projectionMessage.success) {
				if (
					projectionControlPort !== undefined ||
					!projectionMessage.data.security.contextIsolated ||
					!projectionMessage.data.security.sandboxed
				) {
					port.close();
					return;
				}
				projectionControlPort = port;
			} else {
				port.close();
				return;
			}
		}
	}
	if (
		clientSession === undefined ||
		lifetimeControlPort === undefined ||
		projectionControlPort === undefined
	) {
		return;
	}
	const root = document.querySelector('#root');
	if (!(root instanceof HTMLElement)) {
		clientSession.port.close();
		lifetimeControlPort.close();
		projectionControlPort.close();
		return;
	}
	session = startDesktopWorkbenchRenderer(
		root,
		document.documentElement,
		navigator.languages,
		resolveDesktopShortcutPlatform(navigator.platform),
		window.localStorage,
		clientSession.bootstrap,
		lifetimeControlPort,
		clientSession.port,
		projectionControlPort,
	);
});

window.addEventListener('unload', () => {
	session?.dispose();
	clientSession?.port.close();
	lifetimeControlPort?.close();
	projectionControlPort?.close();
});
