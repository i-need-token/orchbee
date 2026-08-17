export const enUSKeyboardShortcutMessages = {
	'keyboard.shortcuts.actions.actionsSearch': 'Search actions',
	'keyboard.shortcuts.actions.change': 'Change shortcut for {action}',
	'keyboard.shortcuts.actions.copySelection': 'Copy terminal selection',
	'keyboard.shortcuts.actions.find': 'Find in terminal',
	'keyboard.shortcuts.actions.globalSearch': 'Search everything',
	'keyboard.shortcuts.actions.newTerminal': 'New terminal',
	'keyboard.shortcuts.actions.reset': 'Restore default',
	'keyboard.shortcuts.actions.selectAll': 'Select all terminal text',
	'keyboard.shortcuts.actions.settings': 'Open settings',
	'keyboard.shortcuts.actions.toggleSidebar': 'Toggle sidebar',
	'keyboard.shortcuts.availability.focusedTerminal': 'When a terminal is focused',
	'keyboard.shortcuts.availability.focusedTerminalWithSelection':
		'When a focused terminal has a selection',
	'keyboard.shortcuts.availability.workbench': 'While the Workbench is active',
	'keyboard.shortcuts.category': 'Keyboard shortcuts',
	'keyboard.shortcuts.conflict':
		'{shortcut} conflicts with {action} ({scope}, {source}; {availability}).',
	'keyboard.shortcuts.description':
		'Customize frequently used Workbench and terminal shortcuts on this device.',
	'keyboard.shortcuts.invalid': 'Press a supported key with Control, Command, Alt, or Super.',
	'keyboard.shortcuts.noResults': 'No keyboard shortcuts found.',
	'keyboard.shortcuts.persistence.invalidWarning':
		'Saved keyboard shortcuts are invalid. Platform defaults are in use.',
	'keyboard.shortcuts.persistence.readWarning':
		'Saved keyboard shortcuts could not be read. Platform defaults are in use.',
	'keyboard.shortcuts.persistence.writeWarning':
		'Your keyboard shortcut changes apply for this session but could not be saved.',
	'keyboard.shortcuts.recording': 'Press a shortcut. Escape or moving focus away cancels.',
	'keyboard.shortcuts.scope.focusedTerminal': 'Focused terminal',
	'keyboard.shortcuts.scope.focusedTerminalOrFind': 'Focused terminal or Find',
	'keyboard.shortcuts.scope.workbench': 'Workbench',
	'keyboard.shortcuts.source.custom': 'Custom',
	'keyboard.shortcuts.source.default': 'Default',
	'keyboard.shortcuts.title': 'Keyboard shortcuts',
} as const;

export type KeyboardShortcutMessageId = keyof typeof enUSKeyboardShortcutMessages;
