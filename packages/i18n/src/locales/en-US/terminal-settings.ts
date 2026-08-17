export const enUSTerminalSettingsMessages = {
	'terminal.settings.actions.back': 'Back',
	'terminal.settings.actions.reset': 'Reset',
	'terminal.settings.applicationTheme.dark': 'Dark',
	'terminal.settings.applicationTheme.description':
		'Choose the light or dark appearance for Orchbee.',
	'terminal.settings.applicationTheme.label': 'Application theme',
	'terminal.settings.applicationTheme.light': 'Light',
	'terminal.settings.browserAccess.actions.open': 'Open Browser access',
	'terminal.settings.browserAccess.description':
		'Use Browser access to open this Desktop terminal in a browser.',
	'terminal.settings.browserAccess.title': 'Browser access',
	'terminal.settings.browserAccess.unavailable': 'Browser access is unavailable in this app.',
	'terminal.settings.categories.browserAccess': 'Browser access',
	'terminal.settings.categories.ghosttyMigration': 'Ghostty migration',
	'terminal.settings.categories.terminal': 'Terminal',
	'terminal.settings.colors.background.description':
		'Choose the default color behind terminal text.',
	'terminal.settings.colors.background.invalid': 'Enter a color in #RRGGBB format.',
	'terminal.settings.colors.background.label': 'Background color',
	'terminal.settings.colors.backgroundOpacity.description':
		'Choose how opaque the terminal background appears.',
	'terminal.settings.colors.backgroundOpacity.invalid':
		'Enter a whole number from {minimum, number} to {maximum, number}.',
	'terminal.settings.colors.backgroundOpacity.label': 'Background opacity',
	'terminal.settings.colors.backgroundOpacity.percentValue': '{value, number}%',
	'terminal.settings.colors.displayTheme.currentDeviceOnlyNotice':
		'Themes other than Host change how terminal colors are displayed on this device only.',
	'terminal.settings.colors.displayTheme.custom': 'Custom',
	'terminal.settings.colors.displayTheme.dark': 'Dark',
	'terminal.settings.colors.displayTheme.description':
		'Choose how terminal foreground and background colors are displayed.',
	'terminal.settings.colors.displayTheme.host': 'Host',
	'terminal.settings.colors.displayTheme.label': 'Terminal display theme',
	'terminal.settings.colors.displayTheme.light': 'Light',
	'terminal.settings.colors.foreground.description': 'Choose the default color for terminal text.',
	'terminal.settings.colors.foreground.invalid': 'Enter a color in #RRGGBB format.',
	'terminal.settings.colors.foreground.label': 'Foreground color',
	'terminal.settings.description': 'Customize the application and terminal appearance.',
	'terminal.settings.ghosttyMigration.actions.analyze': 'Analyze configuration',
	'terminal.settings.ghosttyMigration.actions.clear': 'Clear',
	'terminal.settings.ghosttyMigration.description':
		'See which common Ghostty settings can be carried over, need review, or will not be migrated.',
	'terminal.settings.ghosttyMigration.dispositions.ignored': 'Not migrated',
	'terminal.settings.ghosttyMigration.dispositions.invalid': 'Needs correction',
	'terminal.settings.ghosttyMigration.dispositions.manual': 'Review manually',
	'terminal.settings.ghosttyMigration.dispositions.supported': 'Can carry over',
	'terminal.settings.ghosttyMigration.empty':
		'Paste configuration text to create a migration report.',
	'terminal.settings.ghosttyMigration.lineNumber': 'Line {line, number}',
	'terminal.settings.ghosttyMigration.noAssignments': 'No configuration assignments were found.',
	'terminal.settings.ghosttyMigration.reasons.cellOpacity':
		'Ghostty can make explicit cell backgrounds transparent; the current Orchbee setting changes only the default background.',
	'terminal.settings.ghosttyMigration.reasons.color':
		'Ghostty also accepts named X11 colors. Choose an equivalent #RRGGBB value manually.',
	'terminal.settings.ghosttyMigration.reasons.configFile':
		'Included files were not read. Analyze their contents separately.',
	'terminal.settings.ghosttyMigration.reasons.customColorTarget':
		'Choose the Custom terminal display theme before using this value. The other Custom color remains your current Orchbee value.',
	'terminal.settings.ghosttyMigration.reasons.fontDetails':
		'Orchbee does not expose this font detail in the current Settings surface.',
	'terminal.settings.ghosttyMigration.reasons.fontFallback':
		'Ghostty keeps an ordered fallback list, while this Orchbee setting accepts one value.',
	'terminal.settings.ghosttyMigration.reasons.fontFamily':
		'This font family cannot be represented safely as one current Orchbee value.',
	'terminal.settings.ghosttyMigration.reasons.fontSize':
		'Orchbee supports only whole terminal font sizes from 8 through 48.',
	'terminal.settings.ghosttyMigration.reasons.invalidValue':
		'This value cannot be interpreted by the migration analyzer.',
	'terminal.settings.ghosttyMigration.reasons.keybind':
		'Ghostty action names and binding syntax are not imported. Recreate this in Keyboard shortcuts.',
	'terminal.settings.ghosttyMigration.reasons.losslessTarget':
		'This value has a lossless target in the current Orchbee Settings.',
	'terminal.settings.ghosttyMigration.reasons.malformedLine':
		'Use a key = value line for this migration report.',
	'terminal.settings.ghosttyMigration.reasons.notInWhitelist':
		'This key is not in the migration whitelist and may be Ghostty-specific or misspelled.',
	'terminal.settings.ghosttyMigration.reasons.opacity':
		'Orchbee supports whole background-opacity percentages from 10% through 100%; this value would require rounding or clamping.',
	'terminal.settings.ghosttyMigration.reasons.reset':
		'This resets Ghostty to its default, so no custom value is carried over.',
	'terminal.settings.ghosttyMigration.reasons.superseded':
		'A later valid assignment for this key takes precedence.',
	'terminal.settings.ghosttyMigration.reasons.theme':
		'Ghostty themes may load files and many settings. Choose an Orchbee terminal theme manually.',
	'terminal.settings.ghosttyMigration.reasons.windowTheme':
		'Ghostty window theme does not have the same scope as the Orchbee application theme.',
	'terminal.settings.ghosttyMigration.reasons.workingDirectory':
		'Choose a Project or Workspace folder in Orchbee instead of importing this value.',
	'terminal.settings.ghosttyMigration.rejected.lineTooLong':
		'Line {line, number} is longer than the {limit, number}-character limit. Nothing was analyzed.',
	'terminal.settings.ghosttyMigration.rejected.sourceTooLarge':
		'This text is longer than the {limit, number}-character limit. Nothing was analyzed.',
	'terminal.settings.ghosttyMigration.rejected.tooManyLines':
		'This text has more than {limit, number} lines. Nothing was analyzed.',
	'terminal.settings.ghosttyMigration.reportTitle': 'Migration report',
	'terminal.settings.ghosttyMigration.safetyNotice':
		'This is a dry run. Orchbee will not change or save any settings.',
	'terminal.settings.ghosttyMigration.source.description':
		'Paste the contents of a Ghostty config file. Included files and themes are not opened.',
	'terminal.settings.ghosttyMigration.source.label': 'Ghostty configuration',
	'terminal.settings.ghosttyMigration.source.placeholder': 'font-family = JetBrains Mono',
	'terminal.settings.ghosttyMigration.target': 'Orchbee setting',
	'terminal.settings.ghosttyMigration.title': 'Move from Ghostty',
	'terminal.settings.persistence.invalidWarning':
		'Saved settings are invalid. Defaults are in use; reset to try saving them.',
	'terminal.settings.persistence.readWarning':
		'Saved settings could not be read. Defaults are in use; reset to try saving them.',
	'terminal.settings.persistence.writeWarning':
		'Your changes are applied for this session but could not be saved.',
	'terminal.settings.reset.description': 'Restore the default application and terminal settings.',
	'terminal.settings.sections.applicationAppearance': 'Application appearance',
	'terminal.settings.sections.terminalColors': 'Terminal colors',
	'terminal.settings.sections.terminalTypography': 'Terminal typography',
	'terminal.settings.title': 'Settings',
	'terminal.settings.typography.fontFamily.description': 'Set the font used for terminal text.',
	'terminal.settings.typography.fontFamily.deviceAvailability':
		'Font availability depends on this device. A fallback font is used when needed.',
	'terminal.settings.typography.fontFamily.invalid':
		'Enter a font family between 1 and 256 characters.',
	'terminal.settings.typography.fontFamily.label': 'Terminal font family',
	'terminal.settings.typography.fontSize.description': 'Choose a size for terminal text.',
	'terminal.settings.typography.fontSize.invalid':
		'Enter a whole number from {minimum, number} to {maximum, number}.',
	'terminal.settings.typography.fontSize.label': 'Terminal font size',
	'terminal.settings.search.noResults': 'No settings found.',
	'terminal.settings.search.placeholder': 'Search settings',
} as const;

export type TerminalSettingsMessageId = keyof typeof enUSTerminalSettingsMessages;
