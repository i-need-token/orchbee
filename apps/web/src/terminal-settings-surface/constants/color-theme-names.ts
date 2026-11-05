import { terminalSettingsMessages, type MessageDescriptor } from '@orchbee/i18n';

/** Localized names of the shipped themes, keyed by theme id; a theme file's own name is never shown. */
export const COLOR_THEME_NAME_MESSAGES: ReadonlyMap<string, MessageDescriptor> = new Map([
	['gruvbox-dark', terminalSettingsMessages.colorThemeNameGruvboxDark],
	['gruvbox-light', terminalSettingsMessages.colorThemeNameGruvboxLight],
	['orchbee-dark', terminalSettingsMessages.colorThemeNameOrchbeeDark],
	['orchbee-light', terminalSettingsMessages.colorThemeNameOrchbeeLight],
]);
