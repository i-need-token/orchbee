import {
	resolveTerminalColorTheme,
	type TerminalPresentationConfiguration,
} from '@orchbee/terminal';

import type { TerminalPresentationConfigurationSource } from '../../terminal-presentation';
import type { TerminalAppearancePreferences } from '../../terminal-settings';
import type { ServerHostColors, ServerHostColorsSource } from '../server-connection';

interface ServerTerminalColors {
	/** Paints every terminal's default-coded cells, as the TUI substitutes the selected theme's. */
	readonly display: ServerHostColors;
	/** Reported to a controlled terminal; null unless a terminal launched now follows its controller. */
	readonly host: ServerHostColors | null;
}

export interface ServerTerminalColorsSource {
	current(): ServerTerminalColors;
	subscribe(listener: () => void): () => void;
}

interface ThemePreferencesSource {
	getSnapshot(): {
		readonly preferences: Pick<
			TerminalAppearancePreferences,
			| 'applicationTheme'
			| 'customBackground'
			| 'customForeground'
			| 'darkTheme'
			| 'lightTheme'
			| 'terminalDisplayTheme'
		>;
	};
	subscribeSnapshot(listener: () => void): () => void;
}

/**
 * Resolves the theme preferences as the Worker resolves a launch's colors, with the system's
 * appearance where a TUI has its outer terminal: Theme and Host take the canvas of the selected
 * theme of that polarity. Theme and Host with the System application theme launch a terminal that
 * follows its controller, so only then does the window have colors to report.
 */
export function createServerTerminalColorsSource(
	preferences: ThemePreferencesSource,
	systemTheme: Pick<MediaQueryList, 'addEventListener' | 'matches' | 'removeEventListener'>,
): ServerTerminalColorsSource {
	return {
		current: () => {
			const {
				applicationTheme,
				customBackground,
				customForeground,
				darkTheme,
				lightTheme,
				terminalDisplayTheme,
			} = preferences.getSnapshot().preferences;
			switch (terminalDisplayTheme) {
				case 'custom':
					return {
						display: {
							background: rgbFromHex(customBackground),
							foreground: rgbFromHex(customForeground),
						},
						host: null,
					};
				case 'host':
				case 'theme':
					break;
			}
			const canvas = (polarity: 'dark' | 'light'): ServerHostColors => {
				const theme = resolveTerminalColorTheme(
					polarity,
					polarity === 'dark' ? darkTheme : lightTheme,
				);
				return { background: theme.background, foreground: theme.foreground };
			};
			if (applicationTheme !== 'system') return { display: canvas(applicationTheme), host: null };
			const system = canvas(systemTheme.matches ? 'dark' : 'light');
			return { display: system, host: system };
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
		const key = `${display.foreground}:${display.background}`;
		if (presented === null || presented.base !== base || presented.key !== key) {
			presented = {
				base,
				key,
				value: Object.freeze({
					...base,
					defaultColorOverride: Object.freeze({
						background: hexFromRgb(display.background),
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

function rgbFromHex(value: string): number {
	return Number.parseInt(value.slice(1), 16);
}

function hexFromRgb(value: number): `#${string}` {
	return `#${value.toString(16).padStart(6, '0')}`;
}
