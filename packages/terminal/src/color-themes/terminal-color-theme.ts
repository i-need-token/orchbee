/** The background polarity a color theme is designed for. */
export type TerminalColorThemePolarity = 'dark' | 'light';

/** ANSI colors 0-15 as packed `0xRRGGBB` values: the normal set, then the bright set. */
export interface TerminalAnsiColors {
	readonly black: number;
	readonly red: number;
	readonly green: number;
	readonly yellow: number;
	readonly blue: number;
	readonly magenta: number;
	readonly cyan: number;
	readonly white: number;
	readonly brightBlack: number;
	readonly brightRed: number;
	readonly brightGreen: number;
	readonly brightYellow: number;
	readonly brightBlue: number;
	readonly brightMagenta: number;
	readonly brightCyan: number;
	readonly brightWhite: number;
}

/** One shipped color theme's terminal colors, generated from `crates/color-themes`. */
export interface TerminalColorTheme {
	/** The stable kebab-case id that the `lightTheme` and `darkTheme` preferences store. */
	readonly id: string;
	/** The kebab-case family pairing a light and a dark sibling. */
	readonly family: string;
	readonly polarity: TerminalColorThemePolarity;
	readonly background: number;
	readonly foreground: number;
	readonly cursor: number;
	readonly cursorText: number;
	/** The theme's selection fill, or the resolver's derived one when the source publishes none. */
	readonly selectionBackground: number;
	readonly selectionForeground: number;
	readonly ansi: TerminalAnsiColors;
}

/** The default theme of each polarity. */
export interface TerminalColorThemePair {
	readonly dark: TerminalColorTheme;
	readonly light: TerminalColorTheme;
}
