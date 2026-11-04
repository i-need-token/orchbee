import { startDesktopServerWorkbench } from '../desktop-client';
import { registerTerminalProductFonts } from '../workbench-client';
import { resolveBrowserPreferenceStorage } from './resolveBrowserPreferenceStorage';
import { resolveShortcutPlatform } from './resolveShortcutPlatform';
import { resolveWebRuntimeMode } from './resolveWebRuntimeMode';

/** Starts the Workbench in a Desktop window. Any other page renders nothing. */
export function startWebRenderer(targetWindow: Window): void {
	const { document, location, navigator } = targetWindow;
	const runtimeMode = resolveWebRuntimeMode({
		protocol: location.protocol,
		search: location.search,
		userAgent: navigator.userAgent,
	});
	if (runtimeMode !== 'desktop') return;
	registerTerminalProductFonts(document);
	let focusRecoveryStorage: Pick<Storage, 'getItem' | 'removeItem' | 'setItem'> | undefined;
	try {
		focusRecoveryStorage = targetWindow.sessionStorage;
	} catch {
		focusRecoveryStorage = undefined;
	}
	startDesktopServerWorkbench(
		targetWindow,
		document.querySelector('#root'),
		document.documentElement,
		navigator.languages,
		resolveShortcutPlatform(navigator.platform),
		{
			...(focusRecoveryStorage === undefined ? {} : { focusRecovery: focusRecoveryStorage }),
			preferences: resolveBrowserPreferenceStorage(() => targetWindow.localStorage),
		},
	);
}
