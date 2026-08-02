import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
import '@orchbee/workbench/styles.css';

import {
	TerminalDocumentControlWindowMessageSchema,
	TerminalProjectionControlWindowMessageSchema,
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
let projectionControlPort: MessagePort | undefined;
let session: DesktopWorkbenchRendererSession | undefined;

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
		const projectionMessage = TerminalProjectionControlWindowMessageSchema.safeParse(event.data);
		if (
			!projectionMessage.success ||
			projectionControlPort !== undefined ||
			!projectionMessage.data.security.contextIsolated ||
			!projectionMessage.data.security.sandboxed
		) {
			port.close();
			return;
		}
		projectionControlPort = port;
	}
	if (documentControl === undefined || projectionControlPort === undefined) return;
	const root = document.querySelector('#root');
	if (!(root instanceof HTMLElement)) return;
	session = startDesktopWorkbenchRenderer(
		root,
		document.documentElement,
		navigator.languages,
		resolveDesktopShortcutPlatform(navigator.platform),
		window.localStorage,
		documentControl.bootstrap,
		documentControl.port,
		projectionControlPort,
	);
});

window.addEventListener('unload', () => {
	session?.dispose();
	documentControl?.port.close();
	projectionControlPort?.close();
});
