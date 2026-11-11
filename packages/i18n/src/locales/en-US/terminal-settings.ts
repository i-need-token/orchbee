export const enUSTerminalSettingsMessages = {
	'terminal.settings.shared.unreadable':
		'The settings file has a problem · open Settings to see it',
	'terminal.settings.shared.unavailable':
		"Settings can't be reached · the values shown may be out of date",
	'terminal.settings.shared.conflict':
		'Settings changed elsewhere · your draft is kept · check the current profiles, then save again',
	'terminal.settings.shared.unknown':
		'The change may not have been saved · check the value before changing it again',
	'terminal.settings.shared.system': 'Follow system',
	'terminal.settings.actions.back': 'Back',
	'terminal.settings.actions.close': 'Close settings',
	'terminal.settings.actions.reset': 'Reset',
	'terminal.settings.applicationTheme.dark': 'Dark',
	'terminal.settings.applicationTheme.label': 'Application theme',
	'terminal.settings.applicationTheme.light': 'Light',
	'terminal.settings.colorTheme.names.catppuccinMocha': 'Catppuccin Mocha',
	'terminal.settings.colorTheme.names.dayfox': 'Dayfox',
	'terminal.settings.colorTheme.names.dracula': 'Dracula',
	'terminal.settings.colorTheme.names.flexokiDark': 'Flexoki Dark',
	'terminal.settings.colorTheme.names.flexokiLight': 'Flexoki Light',
	'terminal.settings.colorTheme.names.gruvboxDark': 'Gruvbox Dark',
	'terminal.settings.colorTheme.names.gruvboxLight': 'Gruvbox Light',
	'terminal.settings.colorTheme.names.kanagawaWave': 'Kanagawa Wave',
	'terminal.settings.colorTheme.names.nightfox': 'Nightfox',
	'terminal.settings.colorTheme.names.orchbeeDark': 'Orchbee Dark',
	'terminal.settings.colorTheme.names.orchbeeLight': 'Orchbee Light',
	'terminal.settings.colorTheme.names.rosePine': 'Rosé Pine',
	'terminal.settings.colorTheme.names.rosePineDawn': 'Rosé Pine Dawn',
	'terminal.settings.colorTheme.names.tangoLight': 'Tango Light',
	'terminal.settings.colorTheme.names.tokyoNight': 'Tokyo Night',
	'terminal.settings.colorTheme.names.tomorrowNight': 'Tomorrow Night',
	'terminal.settings.language.english': 'English',
	'terminal.settings.language.simplifiedChinese': '简体中文',

	'terminal.settings.categories.terminal': 'Terminal',
	'terminal.settings.colors.background.invalid': 'Enter a color in #RRGGBB format.',
	'terminal.settings.colors.foreground.invalid': 'Enter a color in #RRGGBB format.',
	'terminal.settings.description': 'Customize the application and terminal appearance.',
	'terminal.settings.interaction.copyOnSelect.label': 'Copy on select',
	'terminal.settings.profiles.actions.add': 'Add profile',
	'terminal.settings.profiles.actions.cancel': 'Cancel',
	'terminal.settings.profiles.actions.confirmDelete': 'Delete this profile',
	'terminal.settings.profiles.actions.confirmReset': 'Reset all profiles',
	'terminal.settings.profiles.actions.delete': 'Delete',
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
	'terminal.settings.profiles.arguments.label': 'Arguments (optional)',
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
		'The last profile change could not be saved. Your previous profiles remain active.',
	'terminal.settings.profiles.futureTerminalsNotice':
		'The default profile is captured when a new terminal starts. Running and restored terminals keep their original command.',
	'terminal.settings.profiles.login.description':
		'Start the system shell as a POSIX login shell by passing -l.',
	'terminal.settings.profiles.login.label': 'Login shell',
	'terminal.settings.profiles.login.unsupported':
		'Login-shell mode is unavailable on this operating system.',
	'terminal.settings.profiles.name.invalid': 'Enter a profile name.',
	'terminal.settings.profiles.name.label': 'Profile name',
	'terminal.settings.profiles.section': 'Launch profiles',
	'terminal.settings.profiles.systemShell': 'System shell',
	'terminal.settings.terminal.scrollbackBytes.controlUnavailableWarning':
		'Scrollback settings are unavailable until the Desktop control connection is restored.',
	'terminal.settings.title': 'Settings',
	'terminal.settings.typography.fontFamily.invalid':
		'Enter a font family between 1 and 256 characters.',
	'terminal.settings.typography.fontSize.invalid':
		'Enter a whole number from {minimum, number} to {maximum, number}.',
	'terminal.settings.typography.fontSize.label': 'Terminal font size',
	'terminal.settings.search.noResults': 'No settings found.',
	'terminal.settings.search.placeholder': 'Search settings',
	'terminal.settings.page.searchResults': 'Search results',
	'terminal.settings.page.noMatches': 'No matching settings.',
	'terminal.settings.page.restoreDefault': 'Restore default',
	'terminal.settings.page.changedFromDefault': 'Changed from default',
	'terminal.settings.page.effect.newTerminals': 'new terminals',
	'terminal.settings.page.effect.nextRestart': 'next restart',
	'terminal.settings.page.dismiss': 'Dismiss',
	'terminal.settings.page.selected': 'Selected',
	'terminal.settings.page.configFile.label': 'More options in config.toml',
	'terminal.settings.page.configFile.copy': 'Copy path',
	'terminal.settings.page.configFile.copied': 'Copied',
	'terminal.settings.page.categories.general': 'General',
	'terminal.settings.page.categories.appearance': 'Appearance',
	'terminal.settings.page.categories.themes': 'Themes',
	'terminal.settings.page.categories.terminal': 'Terminal',
	'terminal.settings.page.categories.notifications': 'Notifications',
	'terminal.settings.page.categories.keyboard': 'Keyboard',
	'terminal.settings.page.categories.agents': 'Agents',
	'terminal.settings.page.groups.afterRestart': 'After a restart',
	'terminal.settings.page.groups.appAppearance': 'App appearance',
	'terminal.settings.page.groups.lightTheme': 'Light theme',
	'terminal.settings.page.groups.darkTheme': 'Dark theme',
	'terminal.settings.page.groups.terminalColors': 'Terminal colors',
	'terminal.settings.page.groups.terminalText': 'Terminal text',
	'terminal.settings.page.groups.selectionAndKeys': 'Selection and keys',
	'terminal.settings.page.groups.output': 'Output',
	'terminal.settings.page.groups.launchProfiles': 'Launch profiles',
	'terminal.settings.page.groups.launchProfiles.description':
		'Which shell a new terminal runs, plus your own profiles.',
	'terminal.settings.page.groups.whenToNotify': 'When to notify',
	'terminal.settings.page.groups.soundAndDelivery': 'Sound and delivery',
	'terminal.settings.page.groups.shortcuts': 'Window and panel shortcuts',
	'terminal.settings.page.groups.agentIntegrations': 'Agent integrations',
	'terminal.settings.page.groups.agentIntegrations.description':
		'Lets Orchbee show each Agent’s status. Turning one on changes one file of that Agent.',
	'terminal.settings.page.filter.count': '{shown, number}/{total, number}',
	'terminal.settings.page.filter.noMatches': 'Nothing matches the filter.',
	'terminal.settings.page.filter.profiles': 'Filter profiles',
	'terminal.settings.page.filter.agents': 'Filter Agents',
	'terminal.settings.page.filter.lightThemes': 'Filter light themes',
	'terminal.settings.page.filter.darkThemes': 'Filter dark themes',
	'terminal.settings.page.themes.inUse': 'In use now',
	'terminal.settings.page.themes.appliesDark': 'Applies when the app appearance is Dark.',
	'terminal.settings.page.themes.appliesLight': 'Applies when the app appearance is Light.',
	'terminal.settings.page.language.label': 'Language',
	'terminal.settings.page.language.description': 'Language for menus and messages.',
	'terminal.settings.page.restoreOutput.label': 'Restore terminal output after restart',
	'terminal.settings.page.restoreOutput.description':
		'Shows recent output again after a restart. Kept on this computer; may include secrets.',
	'terminal.settings.page.applicationTheme.label': 'Application theme',
	'terminal.settings.page.applicationTheme.description':
		'Light, dark, or follow the system. Pick the light and dark themes under Themes.',
	'terminal.settings.page.terminalColors.label': 'Terminal colors',
	'terminal.settings.page.terminalColors.description':
		'Default text and background colors in terminals; colors a program sets stay as they are.',
	'terminal.settings.page.terminalColors.theme': 'Match theme',
	'terminal.settings.page.terminalColors.host': 'Terminal default',
	'terminal.settings.page.terminalColors.custom': 'Custom',
	'terminal.settings.page.textColor.label': 'Text color',
	'terminal.settings.page.textColor.description': 'Terminal text color for Custom.',
	'terminal.settings.page.backgroundColor.label': 'Background color',
	'terminal.settings.page.backgroundColor.description': 'Terminal background color for Custom.',
	'terminal.settings.page.font.label': 'Font',
	'terminal.settings.page.font.description':
		'Font for terminal text; another font fills in when it is missing.',
	'terminal.settings.page.fontSize.label': 'Font size',
	'terminal.settings.page.fontSize.description': 'Size of terminal text.',
	'terminal.settings.page.unit.pixels': 'px',
	'terminal.settings.page.unit.megabytes': 'MB',
	'terminal.settings.page.copyOnSelect.label': 'Copy on select',
	'terminal.settings.page.copyOnSelect.description':
		'Copies text as soon as you finish selecting it.',
	'terminal.settings.page.terminalHistory.label': 'Terminal history',
	'terminal.settings.page.terminalHistory.description':
		'How much output each terminal keeps for scrolling back.',
	'terminal.settings.page.terminalHistory.invalid':
		'Enter a number from {minimum, number} to {maximum, number}.',
	'terminal.settings.page.optionAsAlt.label': 'Use Option as Alt',
	'terminal.settings.page.optionAsAlt.description':
		'The chosen Option key sends Alt combinations instead of special characters.',
	'terminal.settings.page.optionAsAlt.off': 'Off',
	'terminal.settings.page.optionAsAlt.left': 'Left Option',
	'terminal.settings.page.optionAsAlt.right': 'Right Option',
	'terminal.settings.page.optionAsAlt.both': 'Both Option keys',
	'terminal.settings.page.agentNotifications.label': 'Agent status notifications',
	'terminal.settings.page.agentNotifications.description':
		'Tells you when an Agent you can’t see finishes or needs you.',
	'terminal.settings.page.agentNotifications.never': 'Off',
	'terminal.settings.page.agentNotifications.hidden': 'When the Agent isn’t visible',
	'terminal.settings.page.completionReminders.label': 'Command completion reminders',
	'terminal.settings.page.completionReminders.description':
		'Reminds you when a long command finishes. Command text is never shown.',
	'terminal.settings.page.completionReminders.never': 'Off',
	'terminal.settings.page.completionReminders.unfocused': 'When the terminal isn’t focused',
	'terminal.settings.page.completionReminders.always': 'Always',
	'terminal.settings.page.sound.label': 'Notification sound',
	'terminal.settings.page.sound.description': 'Plays a sound with notifications and reminders.',
	'terminal.settings.page.permission.askTitle': 'System notifications are not allowed yet',
	'terminal.settings.page.permission.askDescription':
		'Until you allow them, Agent and command notifications appear only inside Orchbee.',
	'terminal.settings.page.permission.deniedTitle': 'The system is blocking Orchbee’s notifications',
	'terminal.settings.page.permission.deniedDescription':
		'Allow Orchbee in the system notification settings to see them outside the app.',
	'terminal.settings.page.permission.allow': 'Allow',
	'terminal.settings.page.resumeAgents.label': 'Resume Agent conversations after restart',
	'terminal.settings.page.resumeAgents.description':
		'Reopens Claude Code and Codex conversations in their panels after a restart.',
	'terminal.settings.page.profiles.add': 'Add profile',
	'terminal.settings.page.profiles.edit': 'Edit',
	'terminal.settings.page.profiles.done': 'Done',
	'terminal.settings.page.profiles.useAsDefault': 'Use as default',
	'terminal.settings.refused.denied': "This window can't change settings",
	'terminal.settings.refused.unavailable':
		"Settings can't be reached right now · try again shortly",
	'terminal.settings.refused.sessionMismatch': "This window's session has ended · reopen Orchbee",
	'terminal.settings.refused.busy': 'Settings are busy · try again shortly',
	'terminal.settings.refused.readOnly':
		'The settings file is read-only · make it writable and try again',
	'terminal.settings.refused.fileInUse':
		'Another program has the settings file open · close it and try again',
	'terminal.settings.refused.locked':
		'Another edit is writing the settings file · try again shortly',
	'terminal.settings.refused.changed':
		'The settings file changed meanwhile · check the value and try again',
	'terminal.settings.refused.tooLarge':
		'The settings file would get too large · remove entries you no longer need',
	'terminal.settings.refused.writeFailed':
		"The settings file couldn't be written · check disk space and permissions",
	'terminal.settings.refused.unknownKey': "This Orchbee doesn't know this setting · update Orchbee",
	'terminal.settings.refused.invalidValue': "This value isn't allowed · choose another",
	'terminal.settings.refused.syntaxError': 'The settings file has a syntax error · fix it first',
	'terminal.settings.refused.unusableFile':
		"The settings file's location can't be used · check where it points",
	'terminal.settings.file.syntaxError':
		"{path} has a syntax error at line {line}, column {column} · showing the last valid settings, read-only, until it's fixed",
	'terminal.settings.file.unreadable':
		"{path} can't be read · showing the last valid settings, read-only, until it can",
	'terminal.settings.page.newTerminalFolder.label': 'New terminal folder',
	'terminal.settings.page.newTerminalFolder.description':
		'Where a new terminal starts: follow, home, current, workspace, or a folder. Leave empty for the default.',
} as const;

export type TerminalSettingsMessageId = keyof typeof enUSTerminalSettingsMessages;
