import {
	codeEditorMessages,
	fileBarMessages,
	fileExplorerMessages,
	fileOverlayMessages,
	keyboardShortcutMessages,
	type IntlShape,
} from '@orchbee/i18n';

import type { KeyboardShortcutDeclaration } from './keyboard-shortcut-catalog';
import type { KeyboardShortcutScopedDeclaration } from './keyboard-shortcut-scoped-catalog';

/** The name the shortcut settings give the action a shortcut runs. */
export function formatKeyboardShortcutActionName(
	intl: IntlShape,
	declaration: KeyboardShortcutDeclaration | KeyboardShortcutScopedDeclaration,
): string {
	switch (declaration.name) {
		case 'toggle-file-bar':
			return intl.formatMessage(fileBarMessages.toggle);
		case 'toggle-file-overlay-full':
			return intl.formatMessage(fileOverlayMessages.toggleFull);
		case 'close-all-editors':
			return intl.formatMessage(fileOverlayMessages.closeAllEditors);
		case 'close-editor':
			return intl.formatMessage(fileOverlayMessages.closeEditor);
		case 'close-file-overlay':
			return intl.formatMessage(fileOverlayMessages.closeOverlay);
		case 'close-other-editors':
			return intl.formatMessage(fileOverlayMessages.closeOtherEditors);
		case 'close-saved-editors':
			return intl.formatMessage(fileOverlayMessages.closeSavedEditors);
		case 'copy-path':
			return intl.formatMessage(fileExplorerMessages.copyPath);
		case 'copy-relative-path':
			return intl.formatMessage(fileExplorerMessages.copyRelativePath);
		case 'keep-editor':
			return intl.formatMessage(fileOverlayMessages.keepEditor);
		case 'last-editor':
			return intl.formatMessage(fileOverlayMessages.lastEditor);
		case 'move-editor-left':
			return intl.formatMessage(fileOverlayMessages.moveEditorLeft);
		case 'move-editor-right':
			return intl.formatMessage(fileOverlayMessages.moveEditorRight);
		case 'navigate-next-in-quick-open':
			return intl.formatMessage(fileOverlayMessages.navigateNextInQuickOpen);
		case 'navigate-previous-in-quick-open':
			return intl.formatMessage(fileOverlayMessages.navigatePreviousInQuickOpen);
		case 'next-editor':
			return intl.formatMessage(fileOverlayMessages.nextEditor);
		case 'open-to-side':
			return intl.formatMessage(fileExplorerMessages.openToSide);
		case 'pin-editor':
			return intl.formatMessage(fileOverlayMessages.pinEditor);
		case 'position-editor':
			return intl.formatMessage(fileOverlayMessages.positionEditorNumbered, {
				position: declaration.position,
			});
		case 'previous-editor':
			return intl.formatMessage(fileOverlayMessages.previousEditor);
		case 'quick-open-least-recently-used-editor':
			return intl.formatMessage(fileOverlayMessages.quickOpenLeastRecentlyUsedEditor);
		case 'quick-open-previous-recently-used-editor':
			return intl.formatMessage(fileOverlayMessages.quickOpenPreviousRecentlyUsedEditor);
		case 'reopen-closed-editor':
			return intl.formatMessage(fileOverlayMessages.reopenClosedEditor);
		case 'unpin-editor':
			return intl.formatMessage(fileOverlayMessages.unpinEditor);
		case 'increase-editor-font-size':
			return intl.formatMessage(codeEditorMessages.fontZoomIn);
		case 'decrease-editor-font-size':
			return intl.formatMessage(codeEditorMessages.fontZoomOut);
		case 'add-line-comment':
			return intl.formatMessage(codeEditorMessages.addLineComment);
		case 'remove-line-comment':
			return intl.formatMessage(codeEditorMessages.removeLineComment);
		case 'move-selection-to-next-find-match':
			return intl.formatMessage(codeEditorMessages.moveSelectionToNextFindMatch);
		case 'set-selection-anchor':
			return intl.formatMessage(codeEditorMessages.setSelectionAnchor);
		case 'select-from-anchor-to-cursor':
			return intl.formatMessage(codeEditorMessages.selectFromAnchorToCursor);
		case 'show-hover':
			return intl.formatMessage(codeEditorMessages.showHover);
		case 'trim-trailing-whitespace':
			return intl.formatMessage(codeEditorMessages.trimTrailingWhitespace);
		case 'toggle-fold':
			return intl.formatMessage(codeEditorMessages.toggleFold);
		case 'toggle-fold-recursively':
			return intl.formatMessage(codeEditorMessages.toggleFoldRecursively);
		case 'fold-recursively':
			return intl.formatMessage(codeEditorMessages.foldRecursively);
		case 'unfold-recursively':
			return intl.formatMessage(codeEditorMessages.unfoldRecursively);
		case 'fold-all':
			return intl.formatMessage(codeEditorMessages.foldAll);
		case 'unfold-all':
			return intl.formatMessage(codeEditorMessages.unfoldAll);
		case 'fold-all-block-comments':
			return intl.formatMessage(codeEditorMessages.foldAllBlockComments);
		case 'fold-all-regions':
			return intl.formatMessage(codeEditorMessages.foldAllRegions);
		case 'unfold-all-regions':
			return intl.formatMessage(codeEditorMessages.unfoldAllRegions);
		case 'fold-all-except':
			return intl.formatMessage(codeEditorMessages.foldAllExcept);
		case 'unfold-all-except':
			return intl.formatMessage(codeEditorMessages.unfoldAllExcept);
		case 'fold-level-1':
			return intl.formatMessage(codeEditorMessages.foldLevel, { level: 1 });
		case 'fold-level-2':
			return intl.formatMessage(codeEditorMessages.foldLevel, { level: 2 });
		case 'fold-level-3':
			return intl.formatMessage(codeEditorMessages.foldLevel, { level: 3 });
		case 'fold-level-4':
			return intl.formatMessage(codeEditorMessages.foldLevel, { level: 4 });
		case 'fold-level-5':
			return intl.formatMessage(codeEditorMessages.foldLevel, { level: 5 });
		case 'fold-level-6':
			return intl.formatMessage(codeEditorMessages.foldLevel, { level: 6 });
		case 'fold-level-7':
			return intl.formatMessage(codeEditorMessages.foldLevel, { level: 7 });
		case 'create-folding-range-from-selection':
			return intl.formatMessage(codeEditorMessages.createFoldingRangeFromSelection);
		case 'remove-manual-folding-ranges':
			return intl.formatMessage(codeEditorMessages.removeManualFoldingRanges);
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
