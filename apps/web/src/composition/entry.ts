import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
import '@orchbee/workbench/styles.css';

import { startDesktopWebApplication } from '../desktop-client';
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
}
