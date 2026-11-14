import { keyboardShortcutMessages, type IntlShape } from '@orchbee/i18n';

import type { KeyboardShortcutDeclaration } from './keyboard-shortcut-catalog';

/** The name the shortcut settings give the action a shortcut runs. */
export function formatKeyboardShortcutActionName(
	intl: IntlShape,
	declaration: KeyboardShortcutDeclaration,
): string {
	switch (declaration.name) {
		case 'actions-search':
			return intl.formatMessage(keyboardShortcutMessages.actionsSearch);
		case 'clear-terminal':
			return intl.formatMessage(keyboardShortcutMessages.clearTerminal);
		case 'equalize-panels':
			return intl.formatMessage(keyboardShortcutMessages.equalizePanels);
		case 'close-terminal':
			return intl.formatMessage(keyboardShortcutMessages.closeTerminal);
		case 'new-terminal':
			return intl.formatMessage(keyboardShortcutMessages.newTerminal);
		case 'new-terminal-current-directory':
			return intl.formatMessage(keyboardShortcutMessages.newTerminalCurrentDirectory);
		case 'new-workspace':
			return intl.formatMessage(keyboardShortcutMessages.newWorkspace);
		case 'next-tab':
			return intl.formatMessage(keyboardShortcutMessages.nextTab);
		case 'move-tab-backward':
			return intl.formatMessage(keyboardShortcutMessages.moveTabBackward);
		case 'move-tab-forward':
			return intl.formatMessage(keyboardShortcutMessages.moveTabForward);
		case 'set-tab-title':
			return intl.formatMessage(keyboardShortcutMessages.setTabTitle);
		case 'start-agent':
			return intl.formatMessage(keyboardShortcutMessages.startAgent);
		case 'next-prompt':
			return intl.formatMessage(keyboardShortcutMessages.nextPrompt);
		case 'previous-prompt':
			return intl.formatMessage(keyboardShortcutMessages.previousPrompt);
		case 'previous-tab':
			return intl.formatMessage(keyboardShortcutMessages.previousTab);
		case 'scroll-line-down':
			return intl.formatMessage(keyboardShortcutMessages.scrollLineDown);
		case 'scroll-line-up':
			return intl.formatMessage(keyboardShortcutMessages.scrollLineUp);
		case 'scroll-page-down':
			return intl.formatMessage(keyboardShortcutMessages.scrollPageDown);
		case 'scroll-page-up':
			return intl.formatMessage(keyboardShortcutMessages.scrollPageUp);
		case 'scroll-to-bottom':
			return intl.formatMessage(keyboardShortcutMessages.scrollToBottom);
		case 'scroll-to-top':
			return intl.formatMessage(keyboardShortcutMessages.scrollToTop);
		case 'global-search':
			return intl.formatMessage(keyboardShortcutMessages.globalSearch);
		case 'hints':
			return intl.formatMessage(keyboardShortcutMessages.hints);
		case 'maximize-panel':
			return intl.formatMessage(keyboardShortcutMessages.maximizePanel);
		case 'move-panel-down':
			return intl.formatMessage(keyboardShortcutMessages.movePanelDown);
		case 'move-panel-left':
			return intl.formatMessage(keyboardShortcutMessages.movePanelLeft);
		case 'move-panel-right':
			return intl.formatMessage(keyboardShortcutMessages.movePanelRight);
		case 'move-panel-up':
			return intl.formatMessage(keyboardShortcutMessages.movePanelUp);
		case 'settings':
			return intl.formatMessage(keyboardShortcutMessages.settings);
		case 'shortcuts':
			return intl.formatMessage(keyboardShortcutMessages.shortcuts);
		case 'split-below':
			return intl.formatMessage(keyboardShortcutMessages.splitBelow);
		case 'split-right':
			return intl.formatMessage(keyboardShortcutMessages.splitRight);
		case 'toggle-sidebar':
			return intl.formatMessage(keyboardShortcutMessages.toggleSidebar);
		case 'find':
			return intl.formatMessage(keyboardShortcutMessages.find);
		case 'focus-next-region':
			return intl.formatMessage(keyboardShortcutMessages.focusNextRegion);
		case 'focus-panel-down':
			return intl.formatMessage(keyboardShortcutMessages.focusPanelDown);
		case 'focus-panel-left':
			return intl.formatMessage(keyboardShortcutMessages.focusPanelLeft);
		case 'focus-panel-position':
			return intl.formatMessage(keyboardShortcutMessages.focusPanelPosition, {
				position: declaration.position,
			});
		case 'focus-panel-next':
			return intl.formatMessage(keyboardShortcutMessages.focusPanelNext);
		case 'focus-panel-previous':
			return intl.formatMessage(keyboardShortcutMessages.focusPanelPrevious);
		case 'focus-panel-last':
			return intl.formatMessage(keyboardShortcutMessages.focusPanelLast);
		case 'focus-panel-right':
			return intl.formatMessage(keyboardShortcutMessages.focusPanelRight);
		case 'focus-panel-up':
			return intl.formatMessage(keyboardShortcutMessages.focusPanelUp);
		case 'focus-panel-visible':
			return intl.formatMessage(keyboardShortcutMessages.focusPanelVisible);
		case 'focus-previous-region':
			return intl.formatMessage(keyboardShortcutMessages.focusPreviousRegion);
		case 'copy-selection':
			return intl.formatMessage(keyboardShortcutMessages.copySelection);
		case 'decrease-terminal-font-size':
			return intl.formatMessage(keyboardShortcutMessages.decreaseTerminalFontSize);
		case 'increase-terminal-font-size':
			return intl.formatMessage(keyboardShortcutMessages.increaseTerminalFontSize);
		case 'paste':
			return intl.formatMessage(keyboardShortcutMessages.paste);
		case 'select-all':
			return intl.formatMessage(keyboardShortcutMessages.selectAll);
		case 'reset-terminal-font-size':
			return intl.formatMessage(keyboardShortcutMessages.resetTerminalFontSize);
		case 'resize-panel':
			return intl.formatMessage(keyboardShortcutMessages.resizePanel);
		case 'resize-panel-down':
			return intl.formatMessage(keyboardShortcutMessages.resizePanelDown);
		case 'resize-panel-left':
			return intl.formatMessage(keyboardShortcutMessages.resizePanelLeft);
		case 'resize-panel-right':
			return intl.formatMessage(keyboardShortcutMessages.resizePanelRight);
		case 'resize-panel-up':
			return intl.formatMessage(keyboardShortcutMessages.resizePanelUp);
		case 'swap-panel-visible':
			return intl.formatMessage(keyboardShortcutMessages.swapPanelVisible);
		case 'select-tab':
			return intl.formatMessage(keyboardShortcutMessages.selectTab, {
				position: declaration.position,
			});
	}
}
