import { namedTerminalColorThemes, orchbeeTerminalColorThemes } from './color-themes.generated';
import type { TerminalColorTheme, TerminalColorThemePolarity } from './terminal-color-theme';

export type {
	TerminalAnsiColors,
	TerminalColorTheme,
	TerminalColorThemePolarity,
} from './terminal-color-theme';

/** Every shipped color theme, the Orchbee pair first. */
export const terminalColorThemes: readonly TerminalColorTheme[] = Object.freeze([
	orchbeeTerminalColorThemes.light,
	orchbeeTerminalColorThemes.dark,
	...namedTerminalColorThemes,
]);

/** The Orchbee theme of `polarity`, the default of that slot. */
export function orchbeeTerminalColorTheme(
	polarity: TerminalColorThemePolarity,
): TerminalColorTheme {
	return orchbeeTerminalColorThemes[polarity];
}

/**
 * The theme `id` names in `themes` when it has `polarity`; otherwise the Orchbee theme of
 * `polarity`. An id this build does not ship, such as one a newer Client saved, and an id saved in
 * the other polarity's slot both resolve to the slot's default, as `orchbee_color_themes::select`
 * resolves them for the Worker.
 */
export function resolveTerminalColorTheme(
	polarity: TerminalColorThemePolarity,
	id: string,
	themes: readonly TerminalColorTheme[] = terminalColorThemes,
): TerminalColorTheme {
	return (
		themes.find((theme) => theme.polarity === polarity && theme.id === id) ??
		orchbeeTerminalColorTheme(polarity)
	);
}

/** The theme's ANSI colors 0-15 in palette index order. */
export function terminalColorThemeAnsi(theme: TerminalColorTheme): readonly number[] {
	const { ansi } = theme;
	return [
		ansi.black,
		ansi.red,
		ansi.green,
		ansi.yellow,
		ansi.blue,
		ansi.magenta,
		ansi.cyan,
		ansi.white,
		ansi.brightBlack,
		ansi.brightRed,
		ansi.brightGreen,
		ansi.brightYellow,
		ansi.brightBlue,
		ansi.brightMagenta,
		ansi.brightCyan,
		ansi.brightWhite,
	];
}
