import {
	keyboardShortcutChordKey,
	type KeyboardShortcutChord,
	type KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';
import { WORKBENCH_SHORTCUTS } from '@orchbee/workbench';

import { PROJECTED_TERMINAL_SHORTCUTS } from '../../projected-terminal-keyboard-shortcuts';
import {
	defaultKeyboardShortcutBindings,
	type KeyboardShortcutBindings,
	type KeyboardShortcutId,
} from './keyboard-shortcut-catalog';

export interface KeyboardShortcutOverride {
	readonly chord: KeyboardShortcutChord;
	readonly shortcutId: KeyboardShortcutId;
}

export function createKeyboardShortcutBindings(
	platform: KeyboardShortcutPlatform,
	overrides: readonly KeyboardShortcutOverride[],
): KeyboardShortcutBindings {
	const defaults = defaultKeyboardShortcutBindings(platform);
	const overrideByShortcutId = new Map(
		overrides.map((override) => [override.shortcutId, override.chord]),
	);
	const bindingFor = (shortcutId: KeyboardShortcutId): KeyboardShortcutChord =>
		overrideByShortcutId.get(shortcutId) ?? defaults[shortcutId];

	return Object.freeze({
		[WORKBENCH_SHORTCUTS.actionsSearch.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.actionsSearch.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.closeActiveTerminal.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.closeActiveTerminal.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.decreaseTerminalFontSize.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.decreaseTerminalFontSize.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.focusActivePanelDown.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.focusActivePanelDown.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.focusActivePanelLeft.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.focusActivePanelLeft.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.focusActivePanelRight.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.focusActivePanelRight.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.focusActivePanelUp.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.focusActivePanelUp.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.increaseTerminalFontSize.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.increaseTerminalFontSize.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.maximizeActivePanel.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.maximizeActivePanel.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.moveActivePanelDown.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.moveActivePanelDown.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.moveActivePanelLeft.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.moveActivePanelLeft.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.moveActivePanelRight.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.moveActivePanelRight.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.moveActivePanelUp.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.moveActivePanelUp.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.moveActiveTabBackward.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.moveActiveTabBackward.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.moveActiveTabForward.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.moveActiveTabForward.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.setTabTitle.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.setTabTitle.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.newTerminal.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.newTerminal.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.search.shortcutId]: bindingFor(WORKBENCH_SHORTCUTS.search.shortcutId),
		[WORKBENCH_SHORTCUTS.resetTerminalFontSize.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.resetTerminalFontSize.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.resizeActivePanel.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.resizeActivePanel.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.selectNextTab.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.selectNextTab.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.selectPreviousTab.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.selectPreviousTab.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.selectTab1.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.selectTab1.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.selectTab2.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.selectTab2.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.selectTab3.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.selectTab3.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.selectTab4.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.selectTab4.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.selectTab5.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.selectTab5.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.selectTab6.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.selectTab6.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.selectTab7.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.selectTab7.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.selectTab8.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.selectTab8.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.selectTab9.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.selectTab9.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.settings.shortcutId]: bindingFor(WORKBENCH_SHORTCUTS.settings.shortcutId),
		[WORKBENCH_SHORTCUTS.splitActivePanelBelow.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.splitActivePanelBelow.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.splitActivePanelRight.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.splitActivePanelRight.shortcutId,
		),
		[WORKBENCH_SHORTCUTS.toggleSidebar.shortcutId]: bindingFor(
			WORKBENCH_SHORTCUTS.toggleSidebar.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.clear.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.clear.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.findOpen.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.findOpen.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.hintsOpen.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.hintsOpen.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.historyShow.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.historyShow.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.outputShowLast.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.outputShowLast.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.copySelection.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.copySelection.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.paste.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.paste.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.promptNext.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.promptNext.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.promptPrevious.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.promptPrevious.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.scrollLineDown.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.scrollLineDown.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.scrollLineUp.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.scrollLineUp.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.scrollPageDown.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.scrollPageDown.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.scrollPageUp.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.scrollPageUp.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.scrollToBottom.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.scrollToBottom.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.scrollToTop.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.scrollToTop.shortcutId,
		),
		[PROJECTED_TERMINAL_SHORTCUTS.selectAll.shortcutId]: bindingFor(
			PROJECTED_TERMINAL_SHORTCUTS.selectAll.shortcutId,
		),
	});
}

export function keyboardShortcutOverridesEqual(
	left: readonly KeyboardShortcutOverride[],
	right: readonly KeyboardShortcutOverride[],
	platform: KeyboardShortcutPlatform,
): boolean {
	return (
		left.length === right.length &&
		left.every((override, index) => {
			const other = right[index];
			return (
				other !== undefined &&
				override.shortcutId === other.shortcutId &&
				keyboardShortcutChordKey(override.chord, platform) ===
					keyboardShortcutChordKey(other.chord, platform)
			);
		})
	);
}

export function replaceKeyboardShortcutOverride(
	overrides: readonly KeyboardShortcutOverride[],
	shortcutId: KeyboardShortcutId,
	chord: KeyboardShortcutChord | null,
): readonly KeyboardShortcutOverride[] {
	const next = overrides.filter((override) => override.shortcutId !== shortcutId);
	if (chord !== null) next.push(Object.freeze({ chord, shortcutId }));
	return Object.freeze(
		next.sort((left, right) => {
			if (left.shortcutId < right.shortcutId) return -1;
			if (left.shortcutId > right.shortcutId) return 1;
			return 0;
		}),
	);
}
