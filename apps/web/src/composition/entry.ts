import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
import '@orchbee/workbench/styles.css';

import { decodeGatewayBrowserSearch } from '@orchbee/gateway-protocol';
import { BROWSER_AUTHENTICATION_LOGIN_PATH } from '@orchbee/remote-client-protocol';
import { startDesktopWebApplication } from '../desktop-client';
import { startGatewayWebApplication } from '../gateway-client';
import { registerTerminalProductFonts } from '../workbench-client';
import { projectMobileVisualViewport } from './projectMobileVisualViewport';
import { resolveShortcutPlatform } from './resolveShortcutPlatform';
import { startBrowserAuthenticationApplication } from './startBrowserAuthenticationApplication';
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

if (window.location.pathname === BROWSER_AUTHENTICATION_LOGIN_PATH) {
	const session = startBrowserAuthenticationApplication(
		window,
		rootElement,
		document.documentElement,
		navigator.languages,
		window.localStorage,
	);
	window.addEventListener('unload', () => session.dispose(), { once: true });
} else if (runtimeMode === 'browser') {
	const mobileViewport = projectMobileVisualViewport(
		window,
		rootElement instanceof HTMLElement ? rootElement : null,
	);
	const session = startWebApplication(
		rootElement,
		document.documentElement,
		navigator.languages,
		shortcutPlatform,
		window.localStorage,
	);
	window.addEventListener(
		'unload',
		() => {
			session.dispose();
			mobileViewport.dispose();
		},
		{ once: true },
	);
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
			preferences: window.localStorage,
		},
	);
} else if (runtimeMode === 'gateway') {
	const connection = decodeGatewayBrowserSearch(window.location.search);
	if (connection !== null) {
		const mobileViewport = projectMobileVisualViewport(
			window,
			rootElement instanceof HTMLElement ? rootElement : null,
		);
		const session = startGatewayWebApplication(
			window,
			rootElement,
			document.documentElement,
			navigator.languages,
			shortcutPlatform,
			window.localStorage,
			connection,
		);
		window.addEventListener(
			'unload',
			() => {
				session.dispose();
				mobileViewport.dispose();
			},
			{ once: true },
		);
	}
}
