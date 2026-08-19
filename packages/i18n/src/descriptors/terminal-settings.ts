import { defineMessages } from 'react-intl';

import { enUSTerminalSettingsMessages } from '../locales';

export const terminalSettingsMessages = defineMessages({
	applicationAppearanceSection: message('terminal.settings.sections.applicationAppearance'),
	applicationThemeDark: message('terminal.settings.applicationTheme.dark'),
	applicationThemeDescription: message('terminal.settings.applicationTheme.description'),
	applicationThemeLabel: message('terminal.settings.applicationTheme.label'),
	applicationThemeLight: message('terminal.settings.applicationTheme.light'),
	back: message('terminal.settings.actions.back'),
	backgroundDescription: message('terminal.settings.colors.background.description'),
	backgroundInvalid: message('terminal.settings.colors.background.invalid'),
	backgroundLabel: message('terminal.settings.colors.background.label'),
	backgroundOpacityDescription: message('terminal.settings.colors.backgroundOpacity.description'),
	backgroundOpacityLabel: message('terminal.settings.colors.backgroundOpacity.label'),
	backgroundOpacityPercentValue: message(
		'terminal.settings.colors.backgroundOpacity.percentValue',
		'Formatted percentage shown beside the background-opacity control. value is an integer percentage.',
	),
	backgroundOpacityRangeInvalid: message(
		'terminal.settings.colors.backgroundOpacity.invalid',
		'Validation shown when the background opacity is outside the supported inclusive range. minimum and maximum are numeric bounds.',
	),
	browserAccessCategory: message('terminal.settings.categories.browserAccess'),
	browserAccessDescription: message('terminal.settings.browserAccess.description'),
	browserAccessTitle: message('terminal.settings.browserAccess.title'),
	browserAccessUnavailable: message('terminal.settings.browserAccess.unavailable'),
	macosInputCategory: message('terminal.settings.categories.macosInput'),
	macosInputDescription: message('terminal.settings.macosInput.description'),
	macosInputTitle: message('terminal.settings.macosInput.title'),
	macosOptionAsAltBoth: message('terminal.settings.macosInput.optionAsAlt.both'),
	macosOptionAsAltDescription: message('terminal.settings.macosInput.optionAsAlt.description'),
	macosOptionAsAltLabel: message('terminal.settings.macosInput.optionAsAlt.label'),
	macosOptionAsAltLeft: message('terminal.settings.macosInput.optionAsAlt.left'),
	macosOptionAsAltOff: message('terminal.settings.macosInput.optionAsAlt.off'),
	macosOptionAsAltRight: message('terminal.settings.macosInput.optionAsAlt.right'),
	macosOptionKeySection: message('terminal.settings.macosInput.sections.optionKey'),
	currentDeviceOnlyNotice: message(
		'terminal.settings.colors.displayTheme.currentDeviceOnlyNotice',
		'Boundary notice that non-Host display themes affect only the current client device.',
	),
	description: message('terminal.settings.description'),
	fontFamilyDescription: message('terminal.settings.typography.fontFamily.description'),
	fontFamilyDeviceAvailability: message(
		'terminal.settings.typography.fontFamily.deviceAvailability',
	),
	fontFamilyInvalid: message('terminal.settings.typography.fontFamily.invalid'),
	fontFamilyLabel: message('terminal.settings.typography.fontFamily.label'),
	fontSizeDescription: message('terminal.settings.typography.fontSize.description'),
	fontSizeLabel: message('terminal.settings.typography.fontSize.label'),
	fontSizeRangeInvalid: message(
		'terminal.settings.typography.fontSize.invalid',
		'Validation shown when the terminal font size is outside the supported inclusive range. minimum and maximum are numeric bounds.',
	),
	foregroundDescription: message('terminal.settings.colors.foreground.description'),
	foregroundInvalid: message('terminal.settings.colors.foreground.invalid'),
	foregroundLabel: message('terminal.settings.colors.foreground.label'),
	ghosttyMigrationAnalyze: message('terminal.settings.ghosttyMigration.actions.analyze'),
	ghosttyMigrationCategory: message('terminal.settings.categories.ghosttyMigration'),
	ghosttyMigrationClear: message('terminal.settings.ghosttyMigration.actions.clear'),
	ghosttyMigrationDescription: message('terminal.settings.ghosttyMigration.description'),
	ghosttyMigrationEmpty: message('terminal.settings.ghosttyMigration.empty'),
	ghosttyMigrationIgnored: message('terminal.settings.ghosttyMigration.dispositions.ignored'),
	ghosttyMigrationInvalid: message('terminal.settings.ghosttyMigration.dispositions.invalid'),
	ghosttyMigrationLineNumber: message(
		'terminal.settings.ghosttyMigration.lineNumber',
		'Line label for one migration finding. line is a one-based source line number.',
	),
	ghosttyMigrationManual: message('terminal.settings.ghosttyMigration.dispositions.manual'),
	ghosttyMigrationNoAssignments: message('terminal.settings.ghosttyMigration.noAssignments'),
	ghosttyMigrationReasonCellOpacity: message(
		'terminal.settings.ghosttyMigration.reasons.cellOpacity',
	),
	ghosttyMigrationReasonColor: message('terminal.settings.ghosttyMigration.reasons.color'),
	ghosttyMigrationReasonConfigFile: message(
		'terminal.settings.ghosttyMigration.reasons.configFile',
	),
	ghosttyMigrationReasonCustomColorTarget: message(
		'terminal.settings.ghosttyMigration.reasons.customColorTarget',
	),
	ghosttyMigrationReasonFontDetails: message(
		'terminal.settings.ghosttyMigration.reasons.fontDetails',
	),
	ghosttyMigrationReasonFontFallback: message(
		'terminal.settings.ghosttyMigration.reasons.fontFallback',
	),
	ghosttyMigrationReasonFontFamily: message(
		'terminal.settings.ghosttyMigration.reasons.fontFamily',
	),
	ghosttyMigrationReasonFontSize: message('terminal.settings.ghosttyMigration.reasons.fontSize'),
	ghosttyMigrationReasonInvalidValue: message(
		'terminal.settings.ghosttyMigration.reasons.invalidValue',
	),
	ghosttyMigrationReasonKeybind: message('terminal.settings.ghosttyMigration.reasons.keybind'),
	ghosttyMigrationReasonLosslessTarget: message(
		'terminal.settings.ghosttyMigration.reasons.losslessTarget',
	),
	ghosttyMigrationReasonMalformedLine: message(
		'terminal.settings.ghosttyMigration.reasons.malformedLine',
	),
	ghosttyMigrationReasonNotInWhitelist: message(
		'terminal.settings.ghosttyMigration.reasons.notInWhitelist',
	),
	ghosttyMigrationReasonOpacity: message('terminal.settings.ghosttyMigration.reasons.opacity'),
	ghosttyMigrationReasonReset: message('terminal.settings.ghosttyMigration.reasons.reset'),
	ghosttyMigrationReasonSuperseded: message(
		'terminal.settings.ghosttyMigration.reasons.superseded',
	),
	ghosttyMigrationReasonTheme: message('terminal.settings.ghosttyMigration.reasons.theme'),
	ghosttyMigrationReasonWindowTheme: message(
		'terminal.settings.ghosttyMigration.reasons.windowTheme',
	),
	ghosttyMigrationReasonWorkingDirectory: message(
		'terminal.settings.ghosttyMigration.reasons.workingDirectory',
	),
	ghosttyMigrationRejectedLineTooLong: message(
		'terminal.settings.ghosttyMigration.rejected.lineTooLong',
		'Whole-analysis rejection when one line is too long. line is one-based and limit is a character bound.',
	),
	ghosttyMigrationRejectedSourceTooLarge: message(
		'terminal.settings.ghosttyMigration.rejected.sourceTooLarge',
		'Whole-analysis rejection for an oversized source. limit is a character bound.',
	),
	ghosttyMigrationRejectedTooManyLines: message(
		'terminal.settings.ghosttyMigration.rejected.tooManyLines',
		'Whole-analysis rejection for too many source lines. limit is a line-count bound.',
	),
	ghosttyMigrationReportTitle: message('terminal.settings.ghosttyMigration.reportTitle'),
	ghosttyMigrationSafetyNotice: message('terminal.settings.ghosttyMigration.safetyNotice'),
	ghosttyMigrationSourceDescription: message(
		'terminal.settings.ghosttyMigration.source.description',
	),
	ghosttyMigrationSourceLabel: message('terminal.settings.ghosttyMigration.source.label'),
	ghosttyMigrationSourcePlaceholder: message(
		'terminal.settings.ghosttyMigration.source.placeholder',
	),
	ghosttyMigrationSupported: message('terminal.settings.ghosttyMigration.dispositions.supported'),
	ghosttyMigrationTarget: message('terminal.settings.ghosttyMigration.target'),
	ghosttyMigrationTitle: message('terminal.settings.ghosttyMigration.title'),
	noResults: message('terminal.settings.search.noResults'),
	openBrowserAccess: message('terminal.settings.browserAccess.actions.open'),
	persistenceInvalidWarning: message('terminal.settings.persistence.invalidWarning'),
	persistenceReadWarning: message('terminal.settings.persistence.readWarning'),
	persistenceWriteWarning: message(
		'terminal.settings.persistence.writeWarning',
		'Warning that valid changes apply for the current session but could not be saved.',
	),
	profilesAdd: message('terminal.settings.profiles.actions.add'),
	profilesAddNameLabel: message('terminal.settings.profiles.addName.label'),
	profilesAddNamePlaceholder: message('terminal.settings.profiles.addName.placeholder'),
	profilesArgumentAdd: message('terminal.settings.profiles.arguments.actions.add'),
	profilesArgumentLabel: message(
		'terminal.settings.profiles.arguments.argumentLabel',
		'Accessible label for one ordered shell argument. number is its one-based position.',
	),
	profilesArgumentRemove: message(
		'terminal.settings.profiles.arguments.actions.remove',
		'Accessible action for removing one ordered shell argument. number is its one-based position.',
	),
	profilesArgumentsDescription: message('terminal.settings.profiles.arguments.description'),
	profilesArgumentsLabel: message('terminal.settings.profiles.arguments.label'),
	profilesCancel: message('terminal.settings.profiles.actions.cancel'),
	profilesCommandCustom: message('terminal.settings.profiles.command.custom'),
	profilesCommandLabel: message('terminal.settings.profiles.command.label'),
	profilesCommandSystem: message('terminal.settings.profiles.command.system'),
	profilesConfirmDelete: message('terminal.settings.profiles.actions.confirmDelete'),
	profilesConfirmReset: message('terminal.settings.profiles.actions.confirmReset'),
	profilesDefaultBadge: message('terminal.settings.profiles.defaultBadge'),
	profilesDelete: message('terminal.settings.profiles.actions.delete'),
	profilesDuplicate: message('terminal.settings.profiles.actions.duplicate'),
	profilesExecutableDescription: message('terminal.settings.profiles.executable.description'),
	profilesExecutableLabel: message('terminal.settings.profiles.executable.label'),
	profilesExecutablePlaceholder: message('terminal.settings.profiles.executable.placeholder'),
	profilesFailureBusy: message('terminal.settings.profiles.failures.busy'),
	profilesFailureCatalogFull: message('terminal.settings.profiles.failures.catalogFull'),
	profilesFailureControlUnavailable: message(
		'terminal.settings.profiles.failures.controlUnavailable',
	),
	profilesFailureInvalid: message('terminal.settings.profiles.failures.invalid'),
	profilesFailureNotFound: message('terminal.settings.profiles.failures.notFound'),
	profilesFailureProtectedDelete: message('terminal.settings.profiles.failures.protectedDelete'),
	profilesFailureUnsupported: message('terminal.settings.profiles.failures.unsupported'),
	profilesFailureWrite: message('terminal.settings.profiles.failures.write'),
	profilesFutureTerminalsNotice: message('terminal.settings.profiles.futureTerminalsNotice'),
	profilesLoginDescription: message('terminal.settings.profiles.login.description'),
	profilesLoginLabel: message('terminal.settings.profiles.login.label'),
	profilesLoginUnsupported: message('terminal.settings.profiles.login.unsupported'),
	profilesMakeDefault: message('terminal.settings.profiles.actions.makeDefault'),
	profilesNameInvalid: message('terminal.settings.profiles.name.invalid'),
	profilesNameLabel: message('terminal.settings.profiles.name.label'),
	profilesPersistenceInvalid: message('terminal.settings.profiles.persistence.invalid'),
	profilesPersistenceRead: message('terminal.settings.profiles.persistence.read'),
	profilesPersistenceWrite: message('terminal.settings.profiles.persistence.write'),
	profilesReset: message('terminal.settings.profiles.actions.reset'),
	profilesSave: message('terminal.settings.profiles.actions.save'),
	profilesSection: message('terminal.settings.profiles.section'),
	profilesSystemShell: message('terminal.settings.profiles.systemShell'),
	reset: message('terminal.settings.actions.reset'),
	resetDescription: message('terminal.settings.reset.description'),
	remoteDevelopmentBaselineDescription: message(
		'terminal.settings.remoteDevelopment.baseline.description',
	),
	remoteDevelopmentBaselineTitle: message('terminal.settings.remoteDevelopment.baseline.title'),
	remoteDevelopmentCategory: message('terminal.settings.categories.remoteDevelopment'),
	remoteDevelopmentDescription: message('terminal.settings.remoteDevelopment.description'),
	remoteDevelopmentDiagnosticDescription: message(
		'terminal.settings.remoteDevelopment.diagnostic.description',
	),
	remoteDevelopmentDiagnosticLabel: message('terminal.settings.remoteDevelopment.diagnostic.label'),
	remoteDevelopmentDiagnosticTitle: message('terminal.settings.remoteDevelopment.diagnostic.title'),
	remoteDevelopmentNoChangesDescription: message(
		'terminal.settings.remoteDevelopment.noChanges.description',
	),
	remoteDevelopmentNoChangesTitle: message('terminal.settings.remoteDevelopment.noChanges.title'),
	remoteDevelopmentResultDescription: message(
		'terminal.settings.remoteDevelopment.diagnostic.resultDescription',
	),
	remoteDevelopmentTitle: message('terminal.settings.remoteDevelopment.title'),
	shellIntegrationBashTitle: message('terminal.settings.shellIntegration.bash.title'),
	shellIntegrationBashVersion: message('terminal.settings.shellIntegration.bash.version'),
	shellIntegrationCategory: message('terminal.settings.categories.shellIntegration'),
	shellIntegrationDescription: message('terminal.settings.shellIntegration.description'),
	shellIntegrationFishTitle: message('terminal.settings.shellIntegration.fish.title'),
	shellIntegrationManualNotice: message('terminal.settings.shellIntegration.manualNotice'),
	shellIntegrationNotificationDescription: message(
		'terminal.settings.shellIntegration.notification.description',
	),
	shellIntegrationNotificationRequest: message(
		'terminal.settings.shellIntegration.notification.request',
	),
	shellIntegrationNotificationStatusDenied: message(
		'terminal.settings.shellIntegration.notification.status.denied',
	),
	shellIntegrationNotificationStatusGranted: message(
		'terminal.settings.shellIntegration.notification.status.granted',
	),
	shellIntegrationNotificationStatusPrompt: message(
		'terminal.settings.shellIntegration.notification.status.prompt',
	),
	shellIntegrationNotificationStatusUnsupported: message(
		'terminal.settings.shellIntegration.notification.status.unsupported',
	),
	shellIntegrationNotificationTitle: message(
		'terminal.settings.shellIntegration.notification.title',
	),
	shellIntegrationReminderAlways: message('terminal.settings.shellIntegration.reminder.always'),
	shellIntegrationReminderDescription: message(
		'terminal.settings.shellIntegration.reminder.description',
	),
	shellIntegrationReminderMinimumDescription: message(
		'terminal.settings.shellIntegration.reminder.minimum.description',
	),
	shellIntegrationReminderMinimumInvalid: message(
		'terminal.settings.shellIntegration.reminder.minimum.invalid',
	),
	shellIntegrationReminderMinimumLabel: message(
		'terminal.settings.shellIntegration.reminder.minimum.label',
	),
	shellIntegrationReminderModeLabel: message(
		'terminal.settings.shellIntegration.reminder.mode.label',
	),
	shellIntegrationReminderNever: message('terminal.settings.shellIntegration.reminder.never'),
	shellIntegrationReminderTitle: message('terminal.settings.shellIntegration.reminder.title'),
	shellIntegrationReminderUnfocused: message(
		'terminal.settings.shellIntegration.reminder.unfocused',
	),
	shellIntegrationResetDescription: message('terminal.settings.shellIntegration.reset.description'),
	shellIntegrationSetupCommandLabel: message(
		'terminal.settings.shellIntegration.setupCommand.label',
	),
	shellIntegrationTitle: message('terminal.settings.shellIntegration.title'),
	shellIntegrationZshTitle: message('terminal.settings.shellIntegration.zsh.title'),
	searchPlaceholder: message('terminal.settings.search.placeholder'),
	terminalCategory: message('terminal.settings.categories.terminal'),
	terminalColorsSection: message('terminal.settings.sections.terminalColors'),
	terminalDisplayThemeCustom: message('terminal.settings.colors.displayTheme.custom'),
	terminalDisplayThemeDark: message('terminal.settings.colors.displayTheme.dark'),
	terminalDisplayThemeDescription: message('terminal.settings.colors.displayTheme.description'),
	terminalDisplayThemeHost: message('terminal.settings.colors.displayTheme.host'),
	terminalDisplayThemeLabel: message('terminal.settings.colors.displayTheme.label'),
	terminalDisplayThemeLight: message('terminal.settings.colors.displayTheme.light'),
	terminalTypographySection: message('terminal.settings.sections.terminalTypography'),
	title: message('terminal.settings.title'),
});

function message(
	id: keyof typeof enUSTerminalSettingsMessages,
	description?: string,
): {
	readonly defaultMessage: string;
	readonly description?: string;
	readonly id: string;
} {
	return description === undefined
		? { defaultMessage: enUSTerminalSettingsMessages[id], id }
		: { defaultMessage: enUSTerminalSettingsMessages[id], description, id };
}
