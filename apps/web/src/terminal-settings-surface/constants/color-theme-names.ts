import { terminalSettingsMessages, type MessageDescriptor } from '@orchbee/i18n';

/** Localized names of the shipped themes, keyed by theme id; a theme file's own name is never shown. */
export const COLOR_THEME_NAME_MESSAGES: ReadonlyMap<string, MessageDescriptor> = new Map([
	['catppuccin-mocha', terminalSettingsMessages.colorThemeNameCatppuccinMocha],
	['dayfox', terminalSettingsMessages.colorThemeNameDayfox],
	['dracula', terminalSettingsMessages.colorThemeNameDracula],
	['flexoki-dark', terminalSettingsMessages.colorThemeNameFlexokiDark],
	['flexoki-light', terminalSettingsMessages.colorThemeNameFlexokiLight],
	['gruvbox-dark', terminalSettingsMessages.colorThemeNameGruvboxDark],
	['gruvbox-light', terminalSettingsMessages.colorThemeNameGruvboxLight],
	['nightfox', terminalSettingsMessages.colorThemeNameNightfox],
	['orchbee-dark', terminalSettingsMessages.colorThemeNameOrchbeeDark],
	['orchbee-light', terminalSettingsMessages.colorThemeNameOrchbeeLight],
	['rose-pine', terminalSettingsMessages.colorThemeNameRosePine],
	['rose-pine-dawn', terminalSettingsMessages.colorThemeNameRosePineDawn],
]);
