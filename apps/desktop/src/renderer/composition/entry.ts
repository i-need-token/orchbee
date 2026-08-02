import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
import '@orchbee/workbench/styles.css';

import {
	DesktopApplicationLifetimeWindowMessageSchema,
	TerminalDocumentControlWindowMessageSchema,
	TerminalProjectionControlWindowMessageSchema,
	WorkbenchDocumentControlWindowMessageSchema,
	type TerminalDocumentControlBootstrap,
} from '../../shared';
import { resolveDesktopShortcutPlatform } from '../terminal-actions';
import {
	startDesktopWorkbenchRenderer,
	type DesktopWorkbenchRendererSession,
} from './startDesktopWorkbenchRenderer';

interface DocumentControlReady {
	readonly bootstrap: TerminalDocumentControlBootstrap;
	readonly port: MessagePort;
}

let documentControl: DocumentControlReady | undefined;
let lifetimeControlPort: MessagePort | undefined;
let projectionControlPort: MessagePort | undefined;
let session: DesktopWorkbenchRendererSession | undefined;
let workbenchDocumentControlPort: MessagePort | undefined;

window.addEventListener('message', (event) => {
	if (event.source !== window || event.ports.length !== 1 || session !== undefined) return;
	const port = event.ports[0];
	if (port === undefined) return;
	const documentMessage = TerminalDocumentControlWindowMessageSchema.safeParse(event.data);
	if (documentMessage.success) {
		if (
			documentControl !== undefined ||
			!documentMessage.data.security.contextIsolated ||
			!documentMessage.data.security.sandboxed
		) {
			port.close();
			return;
		}
		documentControl = { bootstrap: documentMessage.data.bootstrap, port };
		const root = document.querySelector('#root');
		if (root instanceof HTMLElement) {
			root.dataset['desktopContextIsolated'] = String(
				documentMessage.data.security.contextIsolated,
			);
			root.dataset['desktopSandboxed'] = String(documentMessage.data.security.sandboxed);
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
				const workbenchDocumentMessage = WorkbenchDocumentControlWindowMessageSchema.safeParse(
					event.data,
				);
				if (
					!workbenchDocumentMessage.success ||
					workbenchDocumentControlPort !== undefined ||
					!workbenchDocumentMessage.data.security.contextIsolated ||
					!workbenchDocumentMessage.data.security.sandboxed
				) {
					port.close();
					return;
				}
				workbenchDocumentControlPort = port;
			}
		}
	}
	if (
		documentControl === undefined ||
		lifetimeControlPort === undefined ||
		projectionControlPort === undefined ||
		workbenchDocumentControlPort === undefined
	) {
		return;
	}
	const root = document.querySelector('#root');
	if (!(root instanceof HTMLElement)) {
		documentControl.port.close();
		lifetimeControlPort.close();
		projectionControlPort.close();
		workbenchDocumentControlPort.close();
		return;
	}
	session = startDesktopWorkbenchRenderer(
		root,
		document.documentElement,
		navigator.languages,
		resolveDesktopShortcutPlatform(navigator.platform),
		window.localStorage,
		documentControl.bootstrap,
		lifetimeControlPort,
		documentControl.port,
		projectionControlPort,
		workbenchDocumentControlPort,
	);
});

window.addEventListener('unload', () => {
	session?.dispose();
	documentControl?.port.close();
	lifetimeControlPort?.close();
	projectionControlPort?.close();
	workbenchDocumentControlPort?.close();
});
