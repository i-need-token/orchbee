import type { TerminalAppearancePreferences } from '../../terminal-settings';
import type { ServerHostColors, ServerHostColorsSource } from '../server-connection';

/** The Worker's shipped Light and Dark terminal defaults, so a following terminal matches either. */
const LIGHT_TERMINAL_COLORS: ServerHostColors = Object.freeze({
	background: 0xfc_fc_fc,
	foreground: 0x14_14_14,
});
const DARK_TERMINAL_COLORS: ServerHostColors = Object.freeze({
	background: 0x1e_1e_1e,
	foreground: 0xd4_d4_d4,
});

interface ThemePreferencesSource {
	getSnapshot(): {
		readonly preferences: Pick<
			TerminalAppearancePreferences,
			'applicationTheme' | 'terminalDisplayTheme'
		>;
	};
	subscribeSnapshot(listener: () => void): () => void;
}

/**
 * The Session leaves a terminal's colors to the Client controlling it only under the Host terminal
 * theme with the System application theme; any other choice gave the terminal its own colors at
 * launch. A window then follows the system's appearance, so it offers the terminal defaults of that
 * appearance, as a TUI offers what its outer terminal reports.
 */
export function createServerHostColorsSource(
	preferences: ThemePreferencesSource,
	systemTheme: Pick<MediaQueryList, 'addEventListener' | 'matches' | 'removeEventListener'>,
): ServerHostColorsSource {
	return {
		current: () => {
			const { applicationTheme, terminalDisplayTheme } = preferences.getSnapshot().preferences;
			if (terminalDisplayTheme !== 'host' || applicationTheme !== 'system') return null;
			return systemTheme.matches ? DARK_TERMINAL_COLORS : LIGHT_TERMINAL_COLORS;
		},
		subscribe: (listener) => {
			const unsubscribe = preferences.subscribeSnapshot(listener);
			systemTheme.addEventListener('change', listener);
			return () => {
				unsubscribe();
				systemTheme.removeEventListener('change', listener);
			};
		},
	};
}
