import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
import '@orchbee/workbench/styles.css';

import { startDesktopWebApplication, startServerTerminalPage } from '../desktop-client';
import { registerTerminalProductFonts } from '../workbench-client';
import { projectMobileVisualViewport } from './projectMobileVisualViewport';
import { resolveBrowserPreferenceStorage } from './resolveBrowserPreferenceStorage';
import { resolveShortcutPlatform } from './resolveShortcutPlatform';
import { startWebApplication } from './startWebApplication';
import { resolveServerTerminalRequest } from './resolveServerTerminalRequest';
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
} else if (runtimeMode === 'desktop-server-terminal') {
	void startServerTerminalPage(
		window,
		rootElement,
		document.documentElement,
		navigator.languages,
		shortcutPlatform,
		resolveServerTerminalRequest(window.location.search) ?? 'new',
	).catch((error: unknown) => {
		// This page has no Workbench shell to report through; a test or a developer reads the reason.
		document.documentElement.dataset['serverTerminalFailure'] = String(error);
	});
} else if (runtimeMode === 'desktop') {
	let focusRecoveryStorage: Pick<Storage, 'getItem' | 'removeItem' | 'setItem'> | undefined;
	try {
		focusRecoveryStorage = window.sessionStorage;
	} catch {
		focusRecoveryStorage = undefined;
	}
	startDesktopWebApplication(
		window,
		rootElement,
		document.documentElement,
		navigator.languages,
		shortcutPlatform,
		{
			...(focusRecoveryStorage === undefined ? {} : { focusRecovery: focusRecoveryStorage }),
			preferences: resolveBrowserPreferenceStorage(() => window.localStorage),
		},
	);
}
