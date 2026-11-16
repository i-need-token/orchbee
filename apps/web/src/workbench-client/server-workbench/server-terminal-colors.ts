import type { TerminalPresentationConfiguration } from '@orchbee/terminal';

import type { TerminalPresentationConfigurationSource } from '../../terminal-presentation';
import { windowTerminalColors, type TerminalAppearancePreferences } from '../../terminal-settings';
import type { ServerHostColors, ServerHostColorsSource } from '../server-connection';

interface ServerTerminalColors {
	/** Paints every terminal's default-coded cells, as the TUI substitutes the selected theme's. */
	readonly display: ServerHostColors & {
		readonly cursor: number;
		readonly cursorAccent: number;
		readonly palette: readonly number[];
	};
	/** The effective colors reported whenever this Client controls a terminal. */
	readonly host: ServerHostColors | null;
}

export interface ServerTerminalColorsSource {
	current(): ServerTerminalColors;
	subscribe(listener: () => void): () => void;
}

type ThemePreferences = Pick<
	TerminalAppearancePreferences,
	| 'applicationTheme'
	| 'customBackground'
	| 'customForeground'
	| 'darkTheme'
	| 'lightTheme'
	| 'terminalDisplayTheme'
>;

interface ThemePreferencesSource {
	getSnapshot(): {
		/** The committed preferences, which the Worker hears about. */
		readonly preferences: ThemePreferences;
		/** The committed preferences with any previewed theme, which only this window paints. */
		readonly presented: ThemePreferences;
	};
	subscribeSnapshot(listener: () => void): () => void;
}

/**
 * Resolves the theme preferences as the Worker resolves a launch's colors, with the system's
 * appearance where a TUI has its outer terminal. Every display mode reports its effective defaults,
 * cursor and ANSI palette when controlling a terminal; rendering remains Client-local. A previewed
 * theme repaints this window's terminals at once, while the report keeps the committed colors, so
 * the Worker never remembers a theme nobody chose.
 */
export function createServerTerminalColorsSource(
	themePreferences: ThemePreferencesSource,
	systemTheme: Pick<MediaQueryList, 'addEventListener' | 'matches' | 'removeEventListener'>,
): ServerTerminalColorsSource {
	return {
		current: () => {
			const { preferences, presented } = themePreferences.getSnapshot();
			const shown = windowTerminalColors(presented, systemTheme.matches);
			const committed =
				presented === preferences ? shown : windowTerminalColors(preferences, systemTheme.matches);
			return {
				display: {
					background: shown.background,
					cursor: shown.cursor,
					cursorAccent: shown.cursorText,
					foreground: shown.foreground,
					palette: shown.palette,
				},
				host: {
					background: committed.background,
					cursor: committed.cursor,
					foreground: committed.foreground,
					palette: committed.palette,
				},
			};
		},
		subscribe: (listener) => {
			const unsubscribe = themePreferences.subscribeSnapshot(listener);
			systemTheme.addEventListener('change', listener);
			return () => {
				unsubscribe();
				systemTheme.removeEventListener('change', listener);
			};
		},
	};
}

/** What the projection reports to each terminal it controls. */
export function serverHostColors(colors: ServerTerminalColorsSource): ServerHostColorsSource {
	return {
		current: () => colors.current().host,
		subscribe: (listener) => colors.subscribe(listener),
	};
}

/**
 * The renderer's configuration, painting default-coded cells in the window's colors when it has a
 * colors source. The Host keeps the colors programs query. Only reads and subscriptions pass, so no
 * renderer joins a Host theme transaction.
 */
export function presentServerTerminalColors(
	configuration: TerminalPresentationConfigurationSource,
	colors: ServerTerminalColorsSource | undefined,
): TerminalPresentationConfigurationSource {
	if (colors === undefined) {
		return {
			current: () => configuration.current(),
			subscribe: (listener) => configuration.subscribe(listener),
		};
	}
	// The same inputs give the same configuration, so a listener hears only a real change.
	let presented: {
		readonly base: TerminalPresentationConfiguration;
		readonly key: string;
		readonly value: TerminalPresentationConfiguration;
	} | null = null;
	const current = (): TerminalPresentationConfiguration => {
		const base = configuration.current();
		const { display } = colors.current();
		const key = JSON.stringify(display);
		if (presented === null || presented.base !== base || presented.key !== key) {
			presented = {
				base,
				key,
				value: Object.freeze({
					...base,
					defaultColorOverride: Object.freeze({
						background: hexFromRgb(display.background),
						ansi: Object.freeze([...display.palette]),
						cursor: hexFromRgb(display.cursor),
						cursorAccent: hexFromRgb(display.cursorAccent),
						foreground: hexFromRgb(display.foreground),
					}),
				}),
			};
		}
		return presented.value;
	};
	return {
		current,
		subscribe: (listener) => {
			let last = current();
			const update = (): void => {
				const next = current();
				if (next === last) return;
				last = next;
				listener(next);
			};
			const stopConfiguration = configuration.subscribe(update);
			const stopColors = colors.subscribe(update);
			return () => {
				stopConfiguration();
				stopColors();
			};
		},
	};
}

function hexFromRgb(value: number): `#${string}` {
	return `#${value.toString(16).padStart(6, '0')}`;
}
