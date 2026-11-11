import { terminalSettingsMessages, type MessageDescriptor } from '@orchbee/i18n';
import { resolveTerminalColorTheme, type TerminalColorTheme } from '@orchbee/terminal';

import type { TerminalAppearancePreferences } from '../../terminal-settings';
import { COLOR_THEME_NAME_MESSAGES } from '../constants/color-theme-names';
import type {
	TerminalColorThemeListView,
	TerminalColorThemeOptionView,
} from '../types/terminal-settings-surface';

type Polarity = TerminalColorTheme['polarity'];

interface BuildColorThemePickerViewsOptions {
	readonly colorThemes: readonly TerminalColorTheme[];
	readonly formatMessage: (descriptor: MessageDescriptor) => string;
	readonly preferences: Pick<
		TerminalAppearancePreferences,
		'applicationTheme' | 'darkTheme' | 'lightTheme'
	>;
	/** Whether the operating system asks for dark appearance now. */
	readonly systemDark: boolean;
	readonly onCommit: (polarity: Polarity, themeId: string) => void;
	readonly onPreview: (polarity: Polarity, themeId: string | null) => void;
}

const LIST_MESSAGES = {
	dark: {
		label: terminalSettingsMessages.pageGroupDarkTheme,
		note: terminalSettingsMessages.pageThemesAppliesDark,
	},
	light: {
		label: terminalSettingsMessages.pageGroupLightTheme,
		note: terminalSettingsMessages.pageThemesAppliesLight,
	},
} as const;

/**
 * The Light theme and Dark theme lists. Each lists the themes of its polarity and marks the slot's
 * effective theme, so an id this build does not ship shows the Orchbee theme it resolves to. The
 * list the app appearance shows now is in use; under a fixed appearance the other list explains
 * when it would apply.
 */
export function buildColorThemePickerViews({
	colorThemes,
	formatMessage,
	preferences,
	systemDark,
	onCommit,
	onPreview,
}: BuildColorThemePickerViewsOptions): readonly TerminalColorThemeListView[] {
	const { applicationTheme } = preferences;
	const activePolarity: Polarity =
		applicationTheme === 'system' ? (systemDark ? 'dark' : 'light') : applicationTheme;
	return (['light', 'dark'] as const).map((polarity) => {
		const messages = LIST_MESSAGES[polarity];
		const slot = polarity === 'dark' ? preferences.darkTheme : preferences.lightTheme;
		return {
			inUse: polarity === activePolarity,
			label: formatMessage(messages.label),
			note:
				applicationTheme !== 'system' && polarity !== activePolarity
					? formatMessage(messages.note)
					: null,
			options: colorThemes
				.filter((theme) => theme.polarity === polarity)
				.map((theme) => colorThemeOption(theme, formatMessage)),
			polarity,
			selectedId: resolveTerminalColorTheme(polarity, slot, colorThemes).id,
			settingId: polarity === 'dark' ? 'themes.dark' : 'themes.light',
			onCommit: (themeId) => onCommit(polarity, themeId),
			onPreview: (themeId) => onPreview(polarity, themeId),
		};
	});
}

function colorThemeOption(
	theme: TerminalColorTheme,
	formatMessage: (descriptor: MessageDescriptor) => string,
): TerminalColorThemeOptionView {
	const name = COLOR_THEME_NAME_MESSAGES.get(theme.id);
	return {
		id: theme.id,
		name: name === undefined ? theme.id : formatMessage(name),
		swatch: [
			{ color: hexColor(theme.background), role: 'background' },
			{ color: hexColor(theme.foreground), role: 'foreground' },
			{ color: hexColor(theme.ansi.red), role: 'red' },
			{ color: hexColor(theme.ansi.green), role: 'green' },
			{ color: hexColor(theme.ansi.yellow), role: 'yellow' },
			{ color: hexColor(theme.ansi.blue), role: 'blue' },
			{ color: hexColor(theme.ansi.magenta), role: 'magenta' },
		],
	};
}

function hexColor(rgb: number): string {
	return `#${rgb.toString(16).padStart(6, '0')}`;
}
