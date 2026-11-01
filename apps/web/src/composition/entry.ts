import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
import '@orchbee/workbench/styles.css';

import { startDesktopServerWorkbench, startDesktopWebApplication } from '../desktop-client';
import { registerTerminalProductFonts } from '../workbench-client';
import { projectMobileVisualViewport } from './projectMobileVisualViewport';
import { resolveBrowserPreferenceStorage } from './resolveBrowserPreferenceStorage';
import { resolveShortcutPlatform } from './resolveShortcutPlatform';
import { startWebApplication } from './startWebApplication';
import { resolveWebRuntimeMode } from './resolveWebRuntimeMode';

const rootElement = document.querySelector('#root');
registerTerminalProductFonts(document);
const shortcutPlatform = resolveShortcutPlatform(navigator.platform);
const runtimeMode = resolveWebRuntimeMode({
	protocol: window.location.protocol,
	search: window.location.search,
	userAgent: navigator.userAgent,
});

if (runtimeMode === 'browser') {
	const mobileViewport = projectMobileVisualViewport(
		window,
		rootElement instanceof HTMLElement ? rootElement : null,
	);
	const session = startWebApplication(
		rootElement,
		document.documentElement,
		navigator.languages,
		shortcutPlatform,
		resolveBrowserPreferenceStorage(() => window.localStorage),
	);
	window.addEventListener(
		'unload',
		() => {
			session.dispose();
			mobileViewport.dispose();
		},
		{ once: true },
	);
} else if (runtimeMode === 'desktop' || runtimeMode === 'desktop-server-client') {
	let focusRecoveryStorage: Pick<Storage, 'getItem' | 'removeItem' | 'setItem'> | undefined;
	try {
		focusRecoveryStorage = window.sessionStorage;
	} catch {
		focusRecoveryStorage = undefined;
	}
	const start =
		runtimeMode === 'desktop' ? startDesktopWebApplication : startDesktopServerWorkbench;
	start(window, rootElement, document.documentElement, navigator.languages, shortcutPlatform, {
		...(focusRecoveryStorage === undefined ? {} : { focusRecovery: focusRecoveryStorage }),
		preferences: resolveBrowserPreferenceStorage(() => window.localStorage),
	});
}
