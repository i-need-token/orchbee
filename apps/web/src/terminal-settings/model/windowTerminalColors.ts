import {
	resolveTerminalColorTheme,
	terminalColorThemeAnsi,
	type TerminalColorTheme,
} from '@orchbee/terminal';

import type { TerminalAppearancePreferences } from './terminal-appearance-preferences';

/** The colors a window paints its terminals' default-coded cells with, as 0xRRGGBB. */
export interface WindowTerminalColors {
	readonly background: number;
	readonly cursor: number;
	readonly cursorText: number;
	readonly foreground: number;
	/** ANSI colors 0 through 15. */
	readonly palette: readonly number[];
}

/**
 * Resolves the theme preferences as the Worker resolves a launch's colors: the active slot's theme,
 * with the custom pair as the default text and background under Custom. Host shows the theme too,
 * because a window has no outer terminal whose colors it could show.
 */
export function windowTerminalColors(
	preferences: Pick<
		TerminalAppearancePreferences,
		| 'applicationTheme'
		| 'customBackground'
		| 'customForeground'
		| 'darkTheme'
		| 'lightTheme'
		| 'terminalDisplayTheme'
	>,
	systemDark: boolean,
	/** The themes a slot may name; the shipped themes unless a test supplies more. */
	colorThemes?: readonly TerminalColorTheme[],
): WindowTerminalColors {
	const { applicationTheme, terminalDisplayTheme } = preferences;
	const polarity =
		applicationTheme === 'system' ? (systemDark ? 'dark' : 'light') : applicationTheme;
	const theme = resolveTerminalColorTheme(
		polarity,
		polarity === 'dark' ? preferences.darkTheme : preferences.lightTheme,
		colorThemes,
	);
	const custom = terminalDisplayTheme === 'custom';
	return {
		background: custom ? rgbFromHex(preferences.customBackground) : theme.background,
		cursor: theme.cursor,
		cursorText: theme.cursorText,
		foreground: custom ? rgbFromHex(preferences.customForeground) : theme.foreground,
		palette: terminalColorThemeAnsi(theme),
	};
}

function rgbFromHex(value: string): number {
	return Number.parseInt(value.slice(1), 16);
}
