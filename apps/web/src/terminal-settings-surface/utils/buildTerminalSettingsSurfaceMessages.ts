import {
	keyboardShortcutMessages,
	terminalSettingsMessages,
	type MessageDescriptor,
} from '@orchbee/i18n';

import type { TerminalSettingsSurfaceMessages } from '../types/terminal-settings-surface';

type FormatMessage = (
	descriptor: MessageDescriptor,
	values?: Readonly<Record<string, number>>,
) => string;

export function buildTerminalSettingsSurfaceMessages(
	formatMessage: FormatMessage,
): TerminalSettingsSurfaceMessages {
	return {
		applicationAppearanceSection: formatMessage(
			terminalSettingsMessages.applicationAppearanceSection,
		),
		applicationThemeDark: formatMessage(terminalSettingsMessages.applicationThemeDark),
		applicationThemeDescription: formatMessage(
			terminalSettingsMessages.applicationThemeDescription,
		),
		applicationThemeLabel: formatMessage(terminalSettingsMessages.applicationThemeLabel),
		applicationThemeLight: formatMessage(terminalSettingsMessages.applicationThemeLight),
		back: formatMessage(terminalSettingsMessages.back),
		backgroundDescription: formatMessage(terminalSettingsMessages.backgroundDescription),
		backgroundInvalid: formatMessage(terminalSettingsMessages.backgroundInvalid),
		backgroundLabel: formatMessage(terminalSettingsMessages.backgroundLabel),
		backgroundOpacityDescription: formatMessage(
			terminalSettingsMessages.backgroundOpacityDescription,
		),
		backgroundOpacityLabel: formatMessage(terminalSettingsMessages.backgroundOpacityLabel),
		backgroundOpacityPercentValue: (value) =>
			formatMessage(terminalSettingsMessages.backgroundOpacityPercentValue, { value }),
		browserAccessCategory: formatMessage(terminalSettingsMessages.browserAccessCategory),
		browserAccessDescription: formatMessage(terminalSettingsMessages.browserAccessDescription),
		browserAccessTitle: formatMessage(terminalSettingsMessages.browserAccessTitle),
		close: formatMessage(terminalSettingsMessages.close),
		copyOnSelectDescription: formatMessage(terminalSettingsMessages.copyOnSelectDescription),
		copyOnSelectLabel: formatMessage(terminalSettingsMessages.copyOnSelectLabel),
		currentDeviceOnlyNotice: formatMessage(terminalSettingsMessages.currentDeviceOnlyNotice),
		description: formatMessage(terminalSettingsMessages.description),
		fontFamilyDescription: formatMessage(terminalSettingsMessages.fontFamilyDescription),
		fontFamilyDeviceAvailability: formatMessage(
			terminalSettingsMessages.fontFamilyDeviceAvailability,
		),
		fontFamilyInvalid: formatMessage(terminalSettingsMessages.fontFamilyInvalid),
		fontFamilyLabel: formatMessage(terminalSettingsMessages.fontFamilyLabel),
		fontLigaturesDescription: formatMessage(terminalSettingsMessages.fontLigaturesDescription),
		fontLigaturesLabel: formatMessage(terminalSettingsMessages.fontLigaturesLabel),
		fontSizeDescription: formatMessage(terminalSettingsMessages.fontSizeDescription),
		fontSizeLabel: formatMessage(terminalSettingsMessages.fontSizeLabel),
		fontSizeRangeInvalid: formatMessage(terminalSettingsMessages.fontSizeRangeInvalid, {
			maximum: 48,
			minimum: 8,
		}),
		fontWeightBoldDescription: formatMessage(terminalSettingsMessages.fontWeightBoldDescription),
		fontWeightBoldLabel: formatMessage(terminalSettingsMessages.fontWeightBoldLabel),
		fontWeightDescription: formatMessage(terminalSettingsMessages.fontWeightDescription),
		fontWeightLabel: formatMessage(terminalSettingsMessages.fontWeightLabel),
		foregroundDescription: formatMessage(terminalSettingsMessages.foregroundDescription),
		foregroundInvalid: formatMessage(terminalSettingsMessages.foregroundInvalid),
		foregroundLabel: formatMessage(terminalSettingsMessages.foregroundLabel),
		ghosttyMigrationCategory: formatMessage(terminalSettingsMessages.ghosttyMigrationCategory),
		ghosttyMigrationDescription: formatMessage(
			terminalSettingsMessages.ghosttyMigrationDescription,
		),
		ghosttyMigrationTitle: formatMessage(terminalSettingsMessages.ghosttyMigrationTitle),
		keyboardShortcutsCategory: formatMessage(keyboardShortcutMessages.category),
		keyboardShortcutsDescription: formatMessage(keyboardShortcutMessages.description),
		keyboardShortcutsTitle: formatMessage(keyboardShortcutMessages.title),
		localePersistenceInvalidWarning: formatMessage(
			terminalSettingsMessages.localePersistenceInvalidWarning,
		),
		localePersistenceReadWarning: formatMessage(
			terminalSettingsMessages.localePersistenceReadWarning,
		),
		localePersistenceWriteWarning: formatMessage(
			terminalSettingsMessages.localePersistenceWriteWarning,
		),
		macosInputCategory: formatMessage(terminalSettingsMessages.macosInputCategory),
		macosInputDescription: formatMessage(terminalSettingsMessages.macosInputDescription),
		macosInputTitle: formatMessage(terminalSettingsMessages.macosInputTitle),
		noResults: formatMessage(terminalSettingsMessages.noResults),
		openBrowserAccess: formatMessage(terminalSettingsMessages.openBrowserAccess),
		persistenceInvalidWarning: formatMessage(terminalSettingsMessages.persistenceInvalidWarning),
		persistenceReadWarning: formatMessage(terminalSettingsMessages.persistenceReadWarning),
		persistenceWriteWarning: formatMessage(terminalSettingsMessages.persistenceWriteWarning),
		remoteDevelopmentBaselineDescription: formatMessage(
			terminalSettingsMessages.remoteDevelopmentBaselineDescription,
		),
		remoteDevelopmentBaselineTitle: formatMessage(
			terminalSettingsMessages.remoteDevelopmentBaselineTitle,
		),
		remoteDevelopmentCategory: formatMessage(terminalSettingsMessages.remoteDevelopmentCategory),
		remoteDevelopmentDescription: formatMessage(
			terminalSettingsMessages.remoteDevelopmentDescription,
		),
		remoteDevelopmentDiagnosticDescription: formatMessage(
			terminalSettingsMessages.remoteDevelopmentDiagnosticDescription,
		),
		remoteDevelopmentDiagnosticLabel: formatMessage(
			terminalSettingsMessages.remoteDevelopmentDiagnosticLabel,
		),
		remoteDevelopmentDiagnosticTitle: formatMessage(
			terminalSettingsMessages.remoteDevelopmentDiagnosticTitle,
		),
		remoteDevelopmentNoChangesDescription: formatMessage(
			terminalSettingsMessages.remoteDevelopmentNoChangesDescription,
		),
		remoteDevelopmentNoChangesTitle: formatMessage(
			terminalSettingsMessages.remoteDevelopmentNoChangesTitle,
		),
		remoteDevelopmentResultDescription: formatMessage(
			terminalSettingsMessages.remoteDevelopmentResultDescription,
		),
		remoteDevelopmentTitle: formatMessage(terminalSettingsMessages.remoteDevelopmentTitle),
		reset: formatMessage(terminalSettingsMessages.reset),
		resetDescription: formatMessage(terminalSettingsMessages.resetDescription),
		searchPlaceholder: formatMessage(terminalSettingsMessages.searchPlaceholder),
		scrollbackBytesBusyWarning: formatMessage(terminalSettingsMessages.scrollbackBytesBusyWarning),
		scrollbackBytesControlUnavailableWarning: formatMessage(
			terminalSettingsMessages.scrollbackBytesControlUnavailableWarning,
		),
		scrollbackBytesDescription: formatMessage(terminalSettingsMessages.scrollbackBytesDescription),
		scrollbackBytesFutureNotice: formatMessage(
			terminalSettingsMessages.scrollbackBytesFutureNotice,
		),
		scrollbackBytesHostRejectedWarning: formatMessage(
			terminalSettingsMessages.scrollbackBytesHostRejectedWarning,
		),
		scrollbackBytesInvalid: formatMessage(terminalSettingsMessages.scrollbackBytesInvalid, {
			maximum: 1_000_000_000,
			minimum: 0,
		}),
		scrollbackBytesLabel: formatMessage(terminalSettingsMessages.scrollbackBytesLabel),
		scrollbackBytesPersistenceInvalidWarning: formatMessage(
			terminalSettingsMessages.scrollbackBytesPersistenceInvalidWarning,
		),
		scrollbackBytesPersistenceReadWarning: formatMessage(
			terminalSettingsMessages.scrollbackBytesPersistenceReadWarning,
		),
		scrollbackBytesPersistenceWriteWarning: formatMessage(
			terminalSettingsMessages.scrollbackBytesPersistenceWriteWarning,
		),
		trimTrailingBlankRowsForAgentsDescription: formatMessage(
			terminalSettingsMessages.trimTrailingBlankRowsForAgentsDescription,
		),
		trimTrailingBlankRowsForAgentsLabel: formatMessage(
			terminalSettingsMessages.trimTrailingBlankRowsForAgentsLabel,
		),
		shellIntegrationCategory: formatMessage(terminalSettingsMessages.shellIntegrationCategory),
		shellIntegrationDescription: formatMessage(
			terminalSettingsMessages.shellIntegrationDescription,
		),
		shellIntegrationTitle: formatMessage(terminalSettingsMessages.shellIntegrationTitle),
		terminalCategory: formatMessage(terminalSettingsMessages.terminalCategory),
		terminalClipboardSection: formatMessage(terminalSettingsMessages.terminalClipboardSection),
		terminalColorsSection: formatMessage(terminalSettingsMessages.terminalColorsSection),
		terminalHistorySection: formatMessage(terminalSettingsMessages.terminalHistorySection),
		terminalDisplayThemeCustom: formatMessage(terminalSettingsMessages.terminalDisplayThemeCustom),
		terminalDisplayThemeDark: formatMessage(terminalSettingsMessages.terminalDisplayThemeDark),
		terminalDisplayThemeDescription: formatMessage(
			terminalSettingsMessages.terminalDisplayThemeDescription,
		),
		terminalDisplayThemeHost: formatMessage(terminalSettingsMessages.terminalDisplayThemeHost),
		terminalDisplayThemeLabel: formatMessage(terminalSettingsMessages.terminalDisplayThemeLabel),
		terminalDisplayThemeLight: formatMessage(terminalSettingsMessages.terminalDisplayThemeLight),
		terminalLayoutSection: formatMessage(terminalSettingsMessages.terminalLayoutSection),
		terminalPaddingDescription: formatMessage(terminalSettingsMessages.terminalPaddingDescription),
		terminalPaddingInvalid: formatMessage(terminalSettingsMessages.terminalPaddingInvalid, {
			maximum: 32,
			minimum: 0,
		}),
		terminalPaddingLabel: formatMessage(terminalSettingsMessages.terminalPaddingLabel),
		terminalTextShapingSection: formatMessage(terminalSettingsMessages.terminalTextShapingSection),
		terminalTypographySection: formatMessage(terminalSettingsMessages.terminalTypographySection),
		title: formatMessage(terminalSettingsMessages.title),
	};
}
