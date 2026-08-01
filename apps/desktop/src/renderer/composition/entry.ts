import '@orchbee/ui/styles.css';

import { ProjectedAttachmentWindowMessageSchema } from '../../shared';
import { resolveDesktopShortcutPlatform } from '../terminal-actions';
import styles from './DesktopTerminalApplication.module.css';
import { startDesktopRenderer, type DesktopRendererSession } from './startDesktopRenderer';

let session: DesktopRendererSession | undefined;

window.addEventListener('message', (event) => {
	if (event.source !== window || event.ports.length !== 1 || session !== undefined) return;
	const message = ProjectedAttachmentWindowMessageSchema.safeParse(event.data);
	const port = event.ports[0];
	if (!message.success || port === undefined) return;
	const root = document.querySelector('#root');
	if (root instanceof HTMLElement) {
		root.className = styles['application'] ?? '';
		root.dataset['desktopContextIsolated'] = String(message.data.security.contextIsolated);
		root.dataset['desktopSandboxed'] = String(message.data.security.sandboxed);
	}
	void startDesktopRenderer(
		root,
		document.documentElement,
		navigator.languages,
		resolveDesktopShortcutPlatform(navigator.platform),
		message.data.bootstrap,
		port,
	).then((rendererSession) => {
		session = rendererSession;
		const status = root?.querySelector('[role="alert"]');
		if (status instanceof HTMLElement) status.className = styles['status'] ?? '';
	});
});

window.addEventListener('unload', () => session?.dispose());
