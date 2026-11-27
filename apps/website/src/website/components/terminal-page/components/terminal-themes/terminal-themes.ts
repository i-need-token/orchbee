import { terminalSettingsMessages as names, type MessageDescriptor } from '@orchbee/i18n';
import {
	orchbeeTerminalColorTheme,
	terminalColorThemes,
	type TerminalColorTheme,
} from '@orchbee/terminal';

// Build time only: the section lists the product's own terminal themes, in the product's order and
// under the names its settings show. An activation never imports this module, which would put the
// product's terminal package into the published script.

// The names the product's settings show for its themes, keyed by theme id. A theme the product
// adds needs its name here, or the page build stops.
const THEME_NAMES: ReadonlyMap<string, MessageDescriptor> = new Map([
	['catppuccin-mocha', names.colorThemeNameCatppuccinMocha],
	['dayfox', names.colorThemeNameDayfox],
	['dracula', names.colorThemeNameDracula],
	['flexoki-dark', names.colorThemeNameFlexokiDark],
	['flexoki-light', names.colorThemeNameFlexokiLight],
	['gruvbox-dark', names.colorThemeNameGruvboxDark],
	['gruvbox-light', names.colorThemeNameGruvboxLight],
	['kanagawa-wave', names.colorThemeNameKanagawaWave],
	['nightfox', names.colorThemeNameNightfox],
	['orchbee-dark', names.colorThemeNameOrchbeeDark],
	['orchbee-light', names.colorThemeNameOrchbeeLight],
	['rose-pine', names.colorThemeNameRosePine],
	['rose-pine-dawn', names.colorThemeNameRosePineDawn],
	['tango-light', names.colorThemeNameTangoLight],
	['tokyo-night', names.colorThemeNameTokyoNight],
	['tomorrow-night', names.colorThemeNameTomorrowNight],
]);

/** A theme as the section shows it: the product's theme and its name. */
export interface NamedTerminalTheme {
	readonly theme: TerminalColorTheme;
	readonly name: MessageDescriptor;
}

function named(theme: TerminalColorTheme): NamedTerminalTheme {
	const name = THEME_NAMES.get(theme.id);
	if (name === undefined) throw new Error(`The terminal theme "${theme.id}" has no name.`);
	return { theme, name };
}

/** The theme the page shows first, which the stylesheet already draws. */
export const DEFAULT_TERMINAL_THEME: NamedTerminalTheme = named(orchbeeTerminalColorTheme('dark'));

/** The product's themes of each polarity, in the order its settings list them. */
export const TERMINAL_THEMES_BY_POLARITY: readonly {
	readonly polarity: TerminalColorTheme['polarity'];
	readonly themes: readonly NamedTerminalTheme[];
}[] = (['dark', 'light'] as const).map((polarity) => ({
	polarity,
	themes: terminalColorThemes.filter((theme) => theme.polarity === polarity).map(named),
}));

/** How many themes the product ships. */
export const TERMINAL_THEME_COUNT = terminalColorThemes.length;
