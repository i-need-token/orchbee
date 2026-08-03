import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
import '@orchbee/workbench/styles.css';

import { decodeGatewayBrowserFragment } from '@orchbee/gateway-protocol';
import { startDesktopWebApplication } from '../desktop-client';
import { startGatewayWebApplication } from '../gateway-client';
import { resolveShortcutPlatform } from './resolveShortcutPlatform';
import { startWebApplication } from './startWebApplication';
import { resolveWebRuntimeMode } from './resolveWebRuntimeMode';

const rootElement = document.querySelector('#root');
const shortcutPlatform = resolveShortcutPlatform(navigator.platform);
const runtimeMode = resolveWebRuntimeMode(window.location.search);

if (runtimeMode === 'browser') {
	const session = startWebApplication(
		rootElement,
		document.documentElement,
		navigator.languages,
		shortcutPlatform,
		window.localStorage,
	);
	window.addEventListener('unload', () => session.dispose(), { once: true });
} else if (runtimeMode === 'desktop') {
	startDesktopWebApplication(
		window,
		rootElement,
		document.documentElement,
		navigator.languages,
		shortcutPlatform,
		window.localStorage,
	);
} else if (runtimeMode === 'gateway') {
	const connection = decodeGatewayBrowserFragment(window.location.hash);
	if (connection !== null) {
		const session = startGatewayWebApplication(
			window,
			rootElement,
			document.documentElement,
			navigator.languages,
			shortcutPlatform,
			window.localStorage,
			connection,
		);
		window.addEventListener('unload', () => session.dispose(), { once: true });
	}
}
