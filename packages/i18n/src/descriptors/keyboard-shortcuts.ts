import { defineMessages } from 'react-intl';

import { enUSKeyboardShortcutMessages } from '../locales';

export const keyboardShortcutMessages = defineMessages({
	actionsSearch: message('keyboard.shortcuts.actions.actionsSearch'),
	change: message(
		'keyboard.shortcuts.actions.change',
		'Accessible label for the shortcut recorder. action is a localized action name.',
	),
	clearTerminal: message('keyboard.shortcuts.actions.clearTerminal'),
	closeTerminal: message('keyboard.shortcuts.actions.closeTerminal'),
	copySelection: message('keyboard.shortcuts.actions.copySelection'),
	decreaseTerminalFontSize: message('keyboard.shortcuts.actions.decreaseTerminalFontSize'),
	equalizePanels: message('keyboard.shortcuts.actions.equalizePanels'),
	find: message('keyboard.shortcuts.actions.find'),
	focusPanelDown: message('keyboard.shortcuts.actions.focusPanelDown'),
	focusPanelLeft: message('keyboard.shortcuts.actions.focusPanelLeft'),
	focusPanelRight: message('keyboard.shortcuts.actions.focusPanelRight'),
	focusPanelUp: message('keyboard.shortcuts.actions.focusPanelUp'),
	globalSearch: message('keyboard.shortcuts.actions.globalSearch'),
	hints: message('keyboard.shortcuts.actions.hints'),
	increaseTerminalFontSize: message('keyboard.shortcuts.actions.increaseTerminalFontSize'),
	maximizePanel: message('keyboard.shortcuts.actions.maximizePanel'),
	movePanelDown: message('keyboard.shortcuts.actions.movePanelDown'),
	movePanelLeft: message('keyboard.shortcuts.actions.movePanelLeft'),
	movePanelRight: message('keyboard.shortcuts.actions.movePanelRight'),
	movePanelUp: message('keyboard.shortcuts.actions.movePanelUp'),
	moveTabBackward: message('keyboard.shortcuts.actions.moveTabBackward'),
	moveTabForward: message('keyboard.shortcuts.actions.moveTabForward'),
	setTabTitle: message('keyboard.shortcuts.actions.setTabTitle'),
	newTerminal: message('keyboard.shortcuts.actions.newTerminal'),
	newTerminalCurrentDirectory: message('keyboard.shortcuts.actions.newTerminalCurrentDirectory'),
	nextTab: message('keyboard.shortcuts.actions.nextTab'),
	nextPrompt: message('keyboard.shortcuts.actions.nextPrompt'),
	outputShowLast: message('keyboard.shortcuts.actions.outputShowLast'),
	scrollbackShow: message('keyboard.shortcuts.actions.scrollbackShow'),
	paste: message('keyboard.shortcuts.actions.paste'),
	previousPrompt: message('keyboard.shortcuts.actions.previousPrompt'),
	previousTab: message('keyboard.shortcuts.actions.previousTab'),
	reset: message('keyboard.shortcuts.actions.reset'),
	resetTerminalFontSize: message('keyboard.shortcuts.actions.resetTerminalFontSize'),
	resizePanel: message('keyboard.shortcuts.actions.resizePanel'),
	scrollLineDown: message('keyboard.shortcuts.actions.scrollLineDown'),
	scrollLineUp: message('keyboard.shortcuts.actions.scrollLineUp'),
	scrollPageDown: message('keyboard.shortcuts.actions.scrollPageDown'),
	scrollPageUp: message('keyboard.shortcuts.actions.scrollPageUp'),
	scrollToBottom: message('keyboard.shortcuts.actions.scrollToBottom'),
	scrollToTop: message('keyboard.shortcuts.actions.scrollToTop'),
	selectAll: message('keyboard.shortcuts.actions.selectAll'),
	selectTab: message(
		'keyboard.shortcuts.actions.selectTab',
		'Numbered Tab-selection action. position is the one-based Tab position.',
	),
	settings: message('keyboard.shortcuts.actions.settings'),
	splitBelow: message('keyboard.shortcuts.actions.splitBelow'),
	splitRight: message('keyboard.shortcuts.actions.splitRight'),
	toggleSidebar: message('keyboard.shortcuts.actions.toggleSidebar'),
	availabilityActivePanel: message('keyboard.shortcuts.availability.activePanel'),
	availabilityActiveTerminal: message('keyboard.shortcuts.availability.activeTerminal'),
	availabilityFocusedTerminal: message('keyboard.shortcuts.availability.focusedTerminal'),
	availabilityFocusedTerminalWithSelection: message(
		'keyboard.shortcuts.availability.focusedTerminalWithSelection',
	),
	availabilityDirectionalPanel: message('keyboard.shortcuts.availability.directionalPanel'),
	availabilityWorkbench: message('keyboard.shortcuts.availability.workbench'),
	availabilityMultiplePanels: message('keyboard.shortcuts.availability.multiplePanels'),
	availabilityMultipleTabs: message('keyboard.shortcuts.availability.multipleTabs'),
	availabilityMovableTab: message('keyboard.shortcuts.availability.movableTab'),
	availabilityTabPosition: message('keyboard.shortcuts.availability.tabPosition'),
	category: message('keyboard.shortcuts.category'),
	conflict: message(
		'keyboard.shortcuts.conflict',
		'Inline conflict diagnostic. shortcut, action, scope, source, and availability are localized display strings.',
	),
	description: message('keyboard.shortcuts.description'),
	invalid: message('keyboard.shortcuts.invalid'),
	noResults: message('keyboard.shortcuts.noResults'),
	persistenceInvalidWarning: message('keyboard.shortcuts.persistence.invalidWarning'),
	persistenceReadWarning: message('keyboard.shortcuts.persistence.readWarning'),
	persistenceWriteWarning: message('keyboard.shortcuts.persistence.writeWarning'),
	recording: message('keyboard.shortcuts.recording'),
	scopeActivePanel: message('keyboard.shortcuts.scope.activePanel'),
	scopeActiveTerminal: message('keyboard.shortcuts.scope.activeTerminal'),
	scopeFocusedTerminal: message('keyboard.shortcuts.scope.focusedTerminal'),
	scopeFocusedTerminalOrFind: message('keyboard.shortcuts.scope.focusedTerminalOrFind'),
	scopeWorkbench: message('keyboard.shortcuts.scope.workbench'),
	sourceCustom: message('keyboard.shortcuts.source.custom'),
	sourceDefault: message('keyboard.shortcuts.source.default'),
	title: message('keyboard.shortcuts.title'),
});

function message(
	id: keyof typeof enUSKeyboardShortcutMessages,
	description?: string,
): {
	readonly defaultMessage: string;
	readonly description?: string;
	readonly id: string;
} {
	return description === undefined
		? { defaultMessage: enUSKeyboardShortcutMessages[id], id }
		: { defaultMessage: enUSKeyboardShortcutMessages[id], description, id };
}
