export const enUSTerminalSettingsMessages = {
	'terminal.settings.actions.back': 'Back',
	'terminal.settings.actions.reset': 'Reset',
	'terminal.settings.applicationTheme.dark': 'Dark',
	'terminal.settings.applicationTheme.description':
		'Choose the light or dark appearance for Orchbee.',
	'terminal.settings.applicationTheme.label': 'Application theme',
	'terminal.settings.applicationTheme.light': 'Light',
	'terminal.settings.appearance.terminalPadding.description':
		'Add the same amount of space, in CSS pixels, around terminal text. Existing terminals update immediately.',
	'terminal.settings.appearance.terminalPadding.invalid':
		'Enter a whole number from {minimum, number} to {maximum, number}.',
	'terminal.settings.appearance.terminalPadding.label': 'Terminal padding',
	'terminal.settings.browserAccess.actions.open': 'Open Browser access',
	'terminal.settings.browserAccess.description':
		'Create or reset your Personal Access Token and open this Desktop terminal in a browser.',
	'terminal.settings.browserAccess.title': 'Browser access',
	'terminal.settings.browserAccess.unavailable': 'Browser access is unavailable in this app.',
	'terminal.settings.categories.browserAccess': 'Browser access',
	'terminal.settings.categories.ghosttyMigration': 'Ghostty migration',
	'terminal.settings.categories.macosInput': 'macOS input',
	'terminal.settings.categories.remoteDevelopment': 'Remote development',
	'terminal.settings.categories.shellIntegration': 'Shell integration',
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
	'terminal.settings.interaction.copyOnSelect.description':
		'Copy selected terminal text when the selection completes. Existing terminals update immediately.',
	'terminal.settings.interaction.copyOnSelect.label': 'Copy on select',
	'terminal.settings.macosInput.description':
		'Choose how macOS keyboard input is sent to terminal programs on this Mac.',
	'terminal.settings.macosInput.optionAsAlt.both': 'Both Option keys',
	'terminal.settings.macosInput.optionAsAlt.description':
		'For the selected side, Option sends terminal Alt combinations instead of macOS special characters. Existing terminals update immediately.',
	'terminal.settings.macosInput.optionAsAlt.label': 'Use Option as Alt',
	'terminal.settings.macosInput.optionAsAlt.left': 'Left Option only',
	'terminal.settings.macosInput.optionAsAlt.off': 'Off — type macOS special characters',
	'terminal.settings.macosInput.optionAsAlt.right': 'Right Option only',
	'terminal.settings.macosInput.sections.optionKey': 'Option key',
	'terminal.settings.macosInput.title': 'macOS input',
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
	'terminal.settings.profiles.actions.add': 'Add profile',
	'terminal.settings.profiles.actions.cancel': 'Cancel',
	'terminal.settings.profiles.actions.confirmDelete': 'Delete this profile',
	'terminal.settings.profiles.actions.confirmReset': 'Reset all profiles',
	'terminal.settings.profiles.actions.delete': 'Delete',
	'terminal.settings.profiles.actions.duplicate': 'Duplicate',
	'terminal.settings.profiles.actions.makeDefault': 'Make default',
	'terminal.settings.profiles.actions.reset': 'Reset profiles',
	'terminal.settings.profiles.actions.save': 'Save changes',
	'terminal.settings.profiles.addName.label': 'New profile name',
	'terminal.settings.profiles.addName.placeholder': 'Profile name',
	'terminal.settings.profiles.arguments.actions.add': 'Add argument',
	'terminal.settings.profiles.arguments.actions.remove': 'Remove argument {number, number}',
	'terminal.settings.profiles.arguments.argumentLabel': 'Argument {number, number}',
	'terminal.settings.profiles.arguments.description':
		'Arguments are passed literally, in this order. Shell quoting is not interpreted.',
	'terminal.settings.profiles.arguments.label': 'Arguments',
	'terminal.settings.profiles.command.custom': 'Custom executable',
	'terminal.settings.profiles.command.label': 'Command',
	'terminal.settings.profiles.command.system': 'System shell',
	'terminal.settings.profiles.defaultBadge': 'Default',
	'terminal.settings.profiles.executable.description':
		'Enter an absolute path to an executable on this device.',
	'terminal.settings.profiles.executable.label': 'Executable path',
	'terminal.settings.profiles.executable.placeholder': '/usr/local/bin/fish',
	'terminal.settings.profiles.failures.busy': 'Wait for the current profile change to finish.',
	'terminal.settings.profiles.failures.catalogFull': 'The profile limit has been reached.',
	'terminal.settings.profiles.failures.controlUnavailable':
		'Profile controls are unavailable. Reopen Settings and try again.',
	'terminal.settings.profiles.failures.invalid':
		'Check the profile name, executable path, and arguments.',
	'terminal.settings.profiles.failures.notFound': 'This profile no longer exists.',
	'terminal.settings.profiles.failures.protectedDelete':
		'Choose another default before deleting this profile.',
	'terminal.settings.profiles.failures.unsupported':
		'This launch option is not supported on this operating system.',
	'terminal.settings.profiles.failures.write':
		'The change was not saved. Your previous profiles are still active.',
	'terminal.settings.profiles.futureTerminalsNotice':
		'The default profile is captured when a new terminal starts. Running and restored terminals keep their original command.',
	'terminal.settings.profiles.login.description':
		'Start the system shell as a POSIX login shell by passing -l.',
	'terminal.settings.profiles.login.label': 'Login shell',
	'terminal.settings.profiles.login.unsupported':
		'Login-shell mode is unavailable on this operating system.',
	'terminal.settings.profiles.name.invalid': 'Enter a profile name.',
	'terminal.settings.profiles.name.label': 'Profile name',
	'terminal.settings.profiles.persistence.invalid':
		'The saved profile file is invalid. The built-in profile is active until you save or reset.',
	'terminal.settings.profiles.persistence.read':
		'The saved profile file could not be read. The built-in profile is active until you save or reset.',
	'terminal.settings.profiles.persistence.write':
		'The last profile change could not be saved. Your previous profiles remain active.',
	'terminal.settings.profiles.section': 'Local shell profiles',
	'terminal.settings.profiles.systemShell': 'System shell',
	'terminal.settings.reset.description': 'Restore the default application and terminal settings.',
	'terminal.settings.remoteDevelopment.baseline.description':
		'Orchbee starts local Desktop terminals with this widely available TERM. A standard SSH PTY can carry it to the remote shell.',
	'terminal.settings.remoteDevelopment.baseline.title': 'Terminal baseline',
	'terminal.settings.remoteDevelopment.description':
		'Check the terminal identity visible to a remote shell without changing the remote host.',
	'terminal.settings.remoteDevelopment.diagnostic.description':
		'Run this command yourself after connecting with SSH. It only reads three environment values and checks whether infocmp recognizes TERM.',
	'terminal.settings.remoteDevelopment.diagnostic.label': 'Read-only diagnostic command',
	'terminal.settings.remoteDevelopment.diagnostic.resultDescription':
		'TERM_MATCH compares TERM with the Orchbee baseline. Unsafe control-bearing values are not printed. TERMINFO distinguishes an unset or unsafe TERM, an unavailable infocmp program, an available entry, and a missing entry.',
	'terminal.settings.remoteDevelopment.diagnostic.title': 'Diagnose the remote shell',
	'terminal.settings.remoteDevelopment.noChanges.description':
		'Orchbee shell integration augments eligible interactive SSH sessions with one private reverse Unix-socket forward for Agent observation. Other SSH modes remain unchanged; Orchbee installs no remote files and keeps no remote cache. Your explicit arguments and OpenSSH configuration remain authoritative.',
	'terminal.settings.remoteDevelopment.noChanges.title': 'No automatic remote changes',
	'terminal.settings.remoteDevelopment.title': 'Remote development',
	'terminal.settings.shellIntegration.bash.title': 'Bash',
	'terminal.settings.shellIntegration.bash.version':
		'Bash 4.4 or newer is required. The Bash 3.2 included with macOS is not supported.',
	'terminal.settings.shellIntegration.description':
		'Expose reliable command boundaries for completion reminders and terminal semantics.',
	'terminal.settings.shellIntegration.fish.title': 'Fish',
	'terminal.settings.shellIntegration.manualNotice':
		'Desktop automatically loads integration for supported new local Zsh, non-login Bash, and Fish terminals. Orchbee never edits shell startup files. Use these source commands as a manual fallback for unsupported launch forms and other clients.',
	'terminal.settings.shellIntegration.notification.description':
		'System notification permission is requested only from this button. Terminal output never requests permission.',
	'terminal.settings.shellIntegration.notification.agentMode.hidden':
		'Notify when the Agent location is hidden',
	'terminal.settings.shellIntegration.notification.agentMode.label': 'Agent status notifications',
	'terminal.settings.shellIntegration.notification.agentMode.never': 'Off',
	'terminal.settings.shellIntegration.notification.request': 'Allow system notifications',
	'terminal.settings.shellIntegration.notification.status.denied':
		'Denied. Change notification access in your browser or operating-system settings.',
	'terminal.settings.shellIntegration.notification.status.granted': 'System notifications allowed.',
	'terminal.settings.shellIntegration.notification.status.prompt':
		'Not requested. Orchbee will not ask until you use the button below.',
	'terminal.settings.shellIntegration.notification.status.unsupported':
		'System notifications are unavailable in this client.',
	'terminal.settings.shellIntegration.notification.title': 'System notifications',
	'terminal.settings.shellIntegration.reminder.always': 'Always',
	'terminal.settings.shellIntegration.reminder.description':
		'Remind only after a semantic command completion from an enabled integration. Command text is never included.',
	'terminal.settings.shellIntegration.reminder.minimum.description':
		'Commands shorter than this many seconds do not produce a reminder.',
	'terminal.settings.shellIntegration.reminder.minimum.invalid':
		'Enter a whole number from {minimum, number} to {maximum, number}.',
	'terminal.settings.shellIntegration.reminder.minimum.label': 'Minimum command duration',
	'terminal.settings.shellIntegration.reminder.mode.label': 'Completion reminders',
	'terminal.settings.shellIntegration.reminder.never': 'Never',
	'terminal.settings.shellIntegration.reminder.title': 'Command completion reminders',
	'terminal.settings.shellIntegration.reminder.unfocused': 'When this terminal is unfocused',
	'terminal.settings.shellIntegration.reset.description':
		'Restore completion reminders to off and the minimum duration to five seconds.',
	'terminal.settings.shellIntegration.setupCommand.label': 'Manual fallback source command',
	'terminal.settings.shellIntegration.title': 'Shell integration',
	'terminal.settings.shellIntegration.zsh.title': 'Zsh',
	'terminal.settings.sections.applicationAppearance': 'Application appearance',
	'terminal.settings.sections.terminalClipboard': 'Selection and clipboard',
	'terminal.settings.sections.terminalColors': 'Terminal colors',
	'terminal.settings.sections.terminalHistory': 'History',
	'terminal.settings.sections.terminalLayout': 'Terminal layout',
	'terminal.settings.sections.terminalTextShaping': 'Text shaping',
	'terminal.settings.sections.terminalTypography': 'Terminal typography',
	'terminal.settings.terminal.scrollbackBytes.busyWarning':
		'Another scrollback update is still in progress. Try again after it finishes.',
	'terminal.settings.terminal.scrollbackBytes.controlUnavailableWarning':
		'Scrollback settings are unavailable until the Desktop control connection is restored.',
	'terminal.settings.terminal.scrollbackBytes.description':
		'Maximum retained terminal history, in bytes. New terminals capture this value.',
	'terminal.settings.terminal.scrollbackBytes.futureNotice':
		'Running terminals keep their current history limit.',
	'terminal.settings.terminal.scrollbackBytes.hostRejectedWarning':
		'The Terminal Host rejected this change. The previous value remains active.',
	'terminal.settings.terminal.scrollbackBytes.invalid':
		'Enter a whole number from {minimum, number} to {maximum, number}.',
	'terminal.settings.terminal.scrollbackBytes.label': 'Scrollback limit',
	'terminal.settings.terminal.scrollbackBytes.persistenceInvalidWarning':
		'The saved scrollback setting is invalid. The runtime default is active for this session.',
	'terminal.settings.terminal.scrollbackBytes.persistenceReadWarning':
		'The saved scrollback setting could not be read. The runtime default is active for this session.',
	'terminal.settings.terminal.scrollbackBytes.persistenceWriteWarning':
		'This scrollback value is active for this session, but could not be saved. Retry the value to save it.',
	'terminal.settings.title': 'Settings',
	'terminal.settings.typography.fontFamily.description': 'Set the font used for terminal text.',
	'terminal.settings.typography.fontFamily.deviceAvailability':
		'Font availability depends on this device. A fallback font is used when needed.',
	'terminal.settings.typography.fontFamily.invalid':
		'Enter a font family between 1 and 256 characters.',
	'terminal.settings.typography.fontFamily.label': 'Terminal font family',
	'terminal.settings.typography.fontLigatures.description':
		'Use discretionary ligatures provided by the terminal font. Existing terminals update immediately.',
	'terminal.settings.typography.fontLigatures.label': 'Font ligatures',
	'terminal.settings.typography.fontSize.description': 'Choose a size for terminal text.',
	'terminal.settings.typography.fontSize.invalid':
		'Enter a whole number from {minimum, number} to {maximum, number}.',
	'terminal.settings.typography.fontSize.label': 'Terminal font size',
	'terminal.settings.typography.fontWeight.description':
		'Choose the numeric weight used for regular terminal text.',
	'terminal.settings.typography.fontWeight.label': 'Regular font weight',
	'terminal.settings.typography.fontWeightBold.description':
		'Choose the numeric weight used for bold terminal text.',
	'terminal.settings.typography.fontWeightBold.label': 'Bold font weight',
	'terminal.settings.search.noResults': 'No settings found.',
	'terminal.settings.search.placeholder': 'Search settings',
} as const;

export type TerminalSettingsMessageId = keyof typeof enUSTerminalSettingsMessages;
