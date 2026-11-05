import { terminalSettingsMessages, type MessageDescriptor } from '@orchbee/i18n';
import { resolveTerminalColorTheme, type TerminalColorTheme } from '@orchbee/terminal';

import type { TerminalAppearancePreferences } from '../../terminal-settings';
import { COLOR_THEME_NAME_MESSAGES } from '../constants/color-theme-names';
import type {
	TerminalColorThemeOptionView,
	TerminalColorThemePickerView,
} from '../types/terminal-settings-surface';

type Polarity = TerminalColorTheme['polarity'];

interface BuildColorThemePickerViewsOptions {
	readonly colorThemes: readonly TerminalColorTheme[];
	readonly formatMessage: (descriptor: MessageDescriptor) => string;
	readonly preferences: Pick<
		TerminalAppearancePreferences,
		'applicationTheme' | 'darkTheme' | 'lightTheme'
	>;
	readonly onCommit: (polarity: Polarity, themeId: string) => void;
	readonly onPreview: (polarity: Polarity, themeId: string | null) => void;
}

const SLOT_MESSAGES = {
	dark: {
		description: terminalSettingsMessages.colorThemeDarkDescription,
		label: terminalSettingsMessages.colorThemeDarkLabel,
		notice: terminalSettingsMessages.colorThemeDarkSystemNotice,
	},
	light: {
		description: terminalSettingsMessages.colorThemeLightDescription,
		label: terminalSettingsMessages.colorThemeLightLabel,
		notice: terminalSettingsMessages.colorThemeLightSystemNotice,
	},
} as const;

/**
 * The Light theme and Dark theme pickers. Each lists the themes of its polarity and marks the
 * slot's effective theme, so an id this build does not ship shows the Orchbee theme it resolves to.
 */
export function buildColorThemePickerViews({
	colorThemes,
	formatMessage,
	preferences,
	onCommit,
	onPreview,
}: BuildColorThemePickerViewsOptions): readonly TerminalColorThemePickerView[] {
	return (['light', 'dark'] as const).map((polarity) => {
		const messages = SLOT_MESSAGES[polarity];
		const slot = polarity === 'dark' ? preferences.darkTheme : preferences.lightTheme;
		return {
			description: formatMessage(messages.description),
			label: formatMessage(messages.label),
			notice: preferences.applicationTheme === 'system' ? formatMessage(messages.notice) : null,
			options: colorThemes
				.filter((theme) => theme.polarity === polarity)
				.map((theme) => colorThemeOption(theme, formatMessage)),
			selectedId: resolveTerminalColorTheme(polarity, slot, colorThemes).id,
			settingId: polarity === 'dark' ? 'appearance.dark-theme' : 'appearance.light-theme',
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
