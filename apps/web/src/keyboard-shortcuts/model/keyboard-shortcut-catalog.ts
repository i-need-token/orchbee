import {
	keyboardShortcutChordKey,
	type KeyboardShortcutChord,
	type KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';
import {
	WORKBENCH_SHORTCUTS,
	defaultWorkbenchShortcutBindings,
	type WorkbenchShortcutBindings,
	type WorkbenchShortcutId,
	type WorkbenchTabPosition,
} from '@orchbee/workbench';

import {
	PROJECTED_TERMINAL_SHORTCUTS,
	defaultProjectedTerminalShortcutBindings,
	type ProjectedTerminalShortcutBindings,
	type ProjectedTerminalShortcutId,
} from '../../projected-terminal-keyboard-shortcuts';

export type KeyboardShortcutId = WorkbenchShortcutId | ProjectedTerminalShortcutId;
export type KeyboardShortcutBindings = WorkbenchShortcutBindings &
	ProjectedTerminalShortcutBindings;
type KeyboardShortcutFeature = 'terminal' | 'workbench';
export type KeyboardShortcutScope =
	| 'active-panel'
	| 'active-terminal'
	| 'focused-terminal'
	| 'focused-terminal-or-find'
	| 'workbench';
export type KeyboardShortcutAvailability =
	| 'active-panel'
	| 'active-terminal'
	| 'always-in-workbench'
	| 'focused-terminal'
	| 'focused-terminal-with-selection'
	| 'movable-tab'
	| 'multiple-panels'
	| 'multiple-tabs'
	| 'directional-panel'
	| 'tab-position';

interface KeyboardShortcutDeclarationBase {
	readonly actionId: string;
	readonly actionInput: Readonly<Record<string, unknown>> | undefined;
	readonly availability: KeyboardShortcutAvailability;
	readonly defaults: Readonly<Record<KeyboardShortcutPlatform, KeyboardShortcutChord>>;
	readonly feature: KeyboardShortcutFeature;
	readonly scope: KeyboardShortcutScope;
	readonly shortcutId: KeyboardShortcutId;
}

type KeyboardShortcutName =
	| 'actions-search'
	| 'clear-terminal'
	| 'close-terminal'
	| 'copy-selection'
	| 'decrease-terminal-font-size'
	| 'find'
	| 'focus-panel-down'
	| 'focus-panel-left'
	| 'focus-panel-right'
	| 'focus-panel-up'
	| 'global-search'
	| 'hints'
	| 'increase-terminal-font-size'
	| 'maximize-panel'
	| 'move-panel-down'
	| 'move-panel-left'
	| 'move-panel-right'
	| 'move-panel-up'
	| 'move-tab-backward'
	| 'move-tab-forward'
	| 'new-terminal'
	| 'next-prompt'
	| 'next-tab'
	| 'show-last-command-output'
	| 'show-scrollback'
	| 'paste'
	| 'previous-prompt'
	| 'previous-tab'
	| 'reset-terminal-font-size'
	| 'resize-panel'
	| 'scroll-line-down'
	| 'scroll-line-up'
	| 'scroll-page-down'
	| 'scroll-page-up'
	| 'scroll-to-bottom'
	| 'scroll-to-top'
	| 'select-all'
	| 'set-tab-title'
	| 'settings'
	| 'split-below'
	| 'split-right'
	| 'toggle-sidebar';

export type KeyboardShortcutDeclaration = KeyboardShortcutDeclarationBase &
	(
		| {
				readonly name: 'select-tab';
				readonly position: WorkbenchTabPosition;
		  }
		| {
				readonly name: KeyboardShortcutName;
				readonly position?: undefined;
		  }
	);

const POSITIONAL_TAB_SHORTCUT_DECLARATIONS = Object.freeze(
	[
		WORKBENCH_SHORTCUTS.selectTab1,
		WORKBENCH_SHORTCUTS.selectTab2,
		WORKBENCH_SHORTCUTS.selectTab3,
		WORKBENCH_SHORTCUTS.selectTab4,
		WORKBENCH_SHORTCUTS.selectTab5,
		WORKBENCH_SHORTCUTS.selectTab6,
		WORKBENCH_SHORTCUTS.selectTab7,
		WORKBENCH_SHORTCUTS.selectTab8,
		WORKBENCH_SHORTCUTS.selectTab9,
	].map(positionalTabDeclaration),
);

function positionalTabDeclaration(
	shortcut:
		| typeof WORKBENCH_SHORTCUTS.selectTab1
		| typeof WORKBENCH_SHORTCUTS.selectTab2
		| typeof WORKBENCH_SHORTCUTS.selectTab3
		| typeof WORKBENCH_SHORTCUTS.selectTab4
		| typeof WORKBENCH_SHORTCUTS.selectTab5
		| typeof WORKBENCH_SHORTCUTS.selectTab6
		| typeof WORKBENCH_SHORTCUTS.selectTab7
		| typeof WORKBENCH_SHORTCUTS.selectTab8
		| typeof WORKBENCH_SHORTCUTS.selectTab9,
): KeyboardShortcutDeclaration {
	return Object.freeze({
		actionId: shortcut.id,
		actionInput: shortcut.input,
		availability: 'tab-position',
		defaults: shortcut.defaults,
		feature: 'workbench',
		name: 'select-tab',
		position: shortcut.input.position,
		scope: 'workbench',
		shortcutId: shortcut.shortcutId,
	});
}

export const KEYBOARD_SHORTCUT_CATALOG = Object.freeze([
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.actionsSearch.id,
		actionInput: WORKBENCH_SHORTCUTS.actionsSearch.input,
		availability: 'always-in-workbench',
		defaults: WORKBENCH_SHORTCUTS.actionsSearch.defaults,
		feature: 'workbench',
		name: 'actions-search',
		scope: 'workbench',
		shortcutId: WORKBENCH_SHORTCUTS.actionsSearch.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.newTerminal.id,
		actionInput: WORKBENCH_SHORTCUTS.newTerminal.input,
		availability: 'always-in-workbench',
		defaults: WORKBENCH_SHORTCUTS.newTerminal.defaults,
		feature: 'workbench',
		name: 'new-terminal',
		scope: 'workbench',
		shortcutId: WORKBENCH_SHORTCUTS.newTerminal.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.increaseTerminalFontSize.id,
		actionInput: undefined,
		availability: 'always-in-workbench',
		defaults: WORKBENCH_SHORTCUTS.increaseTerminalFontSize.defaults,
		feature: 'terminal',
		name: 'increase-terminal-font-size',
		scope: 'workbench',
		shortcutId: WORKBENCH_SHORTCUTS.increaseTerminalFontSize.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.decreaseTerminalFontSize.id,
		actionInput: undefined,
		availability: 'always-in-workbench',
		defaults: WORKBENCH_SHORTCUTS.decreaseTerminalFontSize.defaults,
		feature: 'terminal',
		name: 'decrease-terminal-font-size',
		scope: 'workbench',
		shortcutId: WORKBENCH_SHORTCUTS.decreaseTerminalFontSize.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.resetTerminalFontSize.id,
		actionInput: undefined,
		availability: 'always-in-workbench',
		defaults: WORKBENCH_SHORTCUTS.resetTerminalFontSize.defaults,
		feature: 'terminal',
		name: 'reset-terminal-font-size',
		scope: 'workbench',
		shortcutId: WORKBENCH_SHORTCUTS.resetTerminalFontSize.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.moveActiveTabBackward.id,
		actionInput: WORKBENCH_SHORTCUTS.moveActiveTabBackward.input,
		availability: 'movable-tab',
		defaults: WORKBENCH_SHORTCUTS.moveActiveTabBackward.defaults,
		feature: 'workbench',
		name: 'move-tab-backward',
		scope: 'workbench',
		shortcutId: WORKBENCH_SHORTCUTS.moveActiveTabBackward.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.moveActiveTabForward.id,
		actionInput: WORKBENCH_SHORTCUTS.moveActiveTabForward.input,
		availability: 'movable-tab',
		defaults: WORKBENCH_SHORTCUTS.moveActiveTabForward.defaults,
		feature: 'workbench',
		name: 'move-tab-forward',
		scope: 'workbench',
		shortcutId: WORKBENCH_SHORTCUTS.moveActiveTabForward.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.setTabTitle.id,
		actionInput: WORKBENCH_SHORTCUTS.setTabTitle.input,
		availability: 'active-terminal',
		defaults: WORKBENCH_SHORTCUTS.setTabTitle.defaults,
		feature: 'workbench',
		name: 'set-tab-title',
		scope: 'workbench',
		shortcutId: WORKBENCH_SHORTCUTS.setTabTitle.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.selectPreviousTab.id,
		actionInput: WORKBENCH_SHORTCUTS.selectPreviousTab.input,
		availability: 'multiple-tabs',
		defaults: WORKBENCH_SHORTCUTS.selectPreviousTab.defaults,
		feature: 'workbench',
		name: 'previous-tab',
		scope: 'workbench',
		shortcutId: WORKBENCH_SHORTCUTS.selectPreviousTab.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.selectNextTab.id,
		actionInput: WORKBENCH_SHORTCUTS.selectNextTab.input,
		availability: 'multiple-tabs',
		defaults: WORKBENCH_SHORTCUTS.selectNextTab.defaults,
		feature: 'workbench',
		name: 'next-tab',
		scope: 'workbench',
		shortcutId: WORKBENCH_SHORTCUTS.selectNextTab.shortcutId,
	}),
	...POSITIONAL_TAB_SHORTCUT_DECLARATIONS,
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.closeActiveTerminal.id,
		actionInput: WORKBENCH_SHORTCUTS.closeActiveTerminal.input,
		availability: 'active-terminal',
		defaults: WORKBENCH_SHORTCUTS.closeActiveTerminal.defaults,
		feature: 'workbench',
		name: 'close-terminal',
		scope: 'active-terminal',
		shortcutId: WORKBENCH_SHORTCUTS.closeActiveTerminal.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.focusActivePanelDown.id,
		actionInput: WORKBENCH_SHORTCUTS.focusActivePanelDown.input,
		availability: 'directional-panel',
		defaults: WORKBENCH_SHORTCUTS.focusActivePanelDown.defaults,
		feature: 'workbench',
		name: 'focus-panel-down',
		scope: 'active-panel',
		shortcutId: WORKBENCH_SHORTCUTS.focusActivePanelDown.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.focusActivePanelLeft.id,
		actionInput: WORKBENCH_SHORTCUTS.focusActivePanelLeft.input,
		availability: 'directional-panel',
		defaults: WORKBENCH_SHORTCUTS.focusActivePanelLeft.defaults,
		feature: 'workbench',
		name: 'focus-panel-left',
		scope: 'active-panel',
		shortcutId: WORKBENCH_SHORTCUTS.focusActivePanelLeft.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.focusActivePanelRight.id,
		actionInput: WORKBENCH_SHORTCUTS.focusActivePanelRight.input,
		availability: 'directional-panel',
		defaults: WORKBENCH_SHORTCUTS.focusActivePanelRight.defaults,
		feature: 'workbench',
		name: 'focus-panel-right',
		scope: 'active-panel',
		shortcutId: WORKBENCH_SHORTCUTS.focusActivePanelRight.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.focusActivePanelUp.id,
		actionInput: WORKBENCH_SHORTCUTS.focusActivePanelUp.input,
		availability: 'directional-panel',
		defaults: WORKBENCH_SHORTCUTS.focusActivePanelUp.defaults,
		feature: 'workbench',
		name: 'focus-panel-up',
		scope: 'active-panel',
		shortcutId: WORKBENCH_SHORTCUTS.focusActivePanelUp.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.moveActivePanelDown.id,
		actionInput: WORKBENCH_SHORTCUTS.moveActivePanelDown.input,
		availability: 'directional-panel',
		defaults: WORKBENCH_SHORTCUTS.moveActivePanelDown.defaults,
		feature: 'workbench',
		name: 'move-panel-down',
		scope: 'active-panel',
		shortcutId: WORKBENCH_SHORTCUTS.moveActivePanelDown.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.moveActivePanelLeft.id,
		actionInput: WORKBENCH_SHORTCUTS.moveActivePanelLeft.input,
		availability: 'directional-panel',
		defaults: WORKBENCH_SHORTCUTS.moveActivePanelLeft.defaults,
		feature: 'workbench',
		name: 'move-panel-left',
		scope: 'active-panel',
		shortcutId: WORKBENCH_SHORTCUTS.moveActivePanelLeft.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.moveActivePanelRight.id,
		actionInput: WORKBENCH_SHORTCUTS.moveActivePanelRight.input,
		availability: 'directional-panel',
		defaults: WORKBENCH_SHORTCUTS.moveActivePanelRight.defaults,
		feature: 'workbench',
		name: 'move-panel-right',
		scope: 'active-panel',
		shortcutId: WORKBENCH_SHORTCUTS.moveActivePanelRight.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.moveActivePanelUp.id,
		actionInput: WORKBENCH_SHORTCUTS.moveActivePanelUp.input,
		availability: 'directional-panel',
		defaults: WORKBENCH_SHORTCUTS.moveActivePanelUp.defaults,
		feature: 'workbench',
		name: 'move-panel-up',
		scope: 'active-panel',
		shortcutId: WORKBENCH_SHORTCUTS.moveActivePanelUp.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.maximizeActivePanel.id,
		actionInput: WORKBENCH_SHORTCUTS.maximizeActivePanel.input,
		availability: 'multiple-panels',
		defaults: WORKBENCH_SHORTCUTS.maximizeActivePanel.defaults,
		feature: 'workbench',
		name: 'maximize-panel',
		scope: 'active-panel',
		shortcutId: WORKBENCH_SHORTCUTS.maximizeActivePanel.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.resizeActivePanel.id,
		actionInput: WORKBENCH_SHORTCUTS.resizeActivePanel.input,
		availability: 'active-panel',
		defaults: WORKBENCH_SHORTCUTS.resizeActivePanel.defaults,
		feature: 'workbench',
		name: 'resize-panel',
		scope: 'active-panel',
		shortcutId: WORKBENCH_SHORTCUTS.resizeActivePanel.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.splitActivePanelRight.id,
		actionInput: WORKBENCH_SHORTCUTS.splitActivePanelRight.input,
		availability: 'active-panel',
		defaults: WORKBENCH_SHORTCUTS.splitActivePanelRight.defaults,
		feature: 'workbench',
		name: 'split-right',
		scope: 'active-panel',
		shortcutId: WORKBENCH_SHORTCUTS.splitActivePanelRight.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.splitActivePanelBelow.id,
		actionInput: WORKBENCH_SHORTCUTS.splitActivePanelBelow.input,
		availability: 'active-panel',
		defaults: WORKBENCH_SHORTCUTS.splitActivePanelBelow.defaults,
		feature: 'workbench',
		name: 'split-below',
		scope: 'active-panel',
		shortcutId: WORKBENCH_SHORTCUTS.splitActivePanelBelow.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.search.id,
		actionInput: WORKBENCH_SHORTCUTS.search.input,
		availability: 'always-in-workbench',
		defaults: WORKBENCH_SHORTCUTS.search.defaults,
		feature: 'workbench',
		name: 'global-search',
		scope: 'workbench',
		shortcutId: WORKBENCH_SHORTCUTS.search.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.settings.id,
		actionInput: WORKBENCH_SHORTCUTS.settings.input,
		availability: 'always-in-workbench',
		defaults: WORKBENCH_SHORTCUTS.settings.defaults,
		feature: 'workbench',
		name: 'settings',
		scope: 'workbench',
		shortcutId: WORKBENCH_SHORTCUTS.settings.shortcutId,
	}),
	Object.freeze({
		actionId: WORKBENCH_SHORTCUTS.toggleSidebar.id,
		actionInput: WORKBENCH_SHORTCUTS.toggleSidebar.input,
		availability: 'always-in-workbench',
		defaults: WORKBENCH_SHORTCUTS.toggleSidebar.defaults,
		feature: 'workbench',
		name: 'toggle-sidebar',
		scope: 'workbench',
		shortcutId: WORKBENCH_SHORTCUTS.toggleSidebar.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.findOpen.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.findOpen.defaults,
		feature: 'terminal',
		name: 'find',
		scope: 'focused-terminal-or-find',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.findOpen.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.hintsOpen.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.hintsOpen.defaults,
		feature: 'terminal',
		name: 'hints',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.hintsOpen.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.outputShowLast.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.outputShowLast.defaults,
		feature: 'terminal',
		name: 'show-last-command-output',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.outputShowLast.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.historyShow.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.historyShow.defaults,
		feature: 'terminal',
		name: 'show-scrollback',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.historyShow.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.copySelection.actionId,
		actionInput: undefined,
		availability: 'focused-terminal-with-selection',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.copySelection.defaults,
		feature: 'terminal',
		name: 'copy-selection',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.copySelection.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.paste.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.paste.defaults,
		feature: 'terminal',
		name: 'paste',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.paste.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.promptPrevious.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.promptPrevious.defaults,
		feature: 'terminal',
		name: 'previous-prompt',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.promptPrevious.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.promptNext.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.promptNext.defaults,
		feature: 'terminal',
		name: 'next-prompt',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.promptNext.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.scrollLineUp.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.scrollLineUp.defaults,
		feature: 'terminal',
		name: 'scroll-line-up',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.scrollLineUp.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.scrollLineDown.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.scrollLineDown.defaults,
		feature: 'terminal',
		name: 'scroll-line-down',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.scrollLineDown.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.scrollPageUp.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.scrollPageUp.defaults,
		feature: 'terminal',
		name: 'scroll-page-up',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.scrollPageUp.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.scrollPageDown.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.scrollPageDown.defaults,
		feature: 'terminal',
		name: 'scroll-page-down',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.scrollPageDown.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.scrollToTop.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.scrollToTop.defaults,
		feature: 'terminal',
		name: 'scroll-to-top',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.scrollToTop.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.scrollToBottom.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.scrollToBottom.defaults,
		feature: 'terminal',
		name: 'scroll-to-bottom',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.scrollToBottom.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.selectAll.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.selectAll.defaults,
		feature: 'terminal',
		name: 'select-all',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.selectAll.shortcutId,
	}),
	Object.freeze({
		actionId: PROJECTED_TERMINAL_SHORTCUTS.clear.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.clear.defaults,
		feature: 'terminal',
		name: 'clear-terminal',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.clear.shortcutId,
	}),
] as const satisfies readonly KeyboardShortcutDeclaration[]);

export const KEYBOARD_SHORTCUT_IDS = Object.freeze(
	KEYBOARD_SHORTCUT_CATALOG.map((declaration) => declaration.shortcutId),
);

const DECLARATION_BY_ID: ReadonlyMap<string, KeyboardShortcutDeclaration> = new Map(
	KEYBOARD_SHORTCUT_CATALOG.map((declaration) => [declaration.shortcutId, declaration]),
);

interface KeyboardShortcutConflictPair {
	readonly firstShortcutId: KeyboardShortcutId;
	readonly secondShortcutId: KeyboardShortcutId;
}

export function keyboardShortcutDeclaration(
	shortcutId: KeyboardShortcutId,
): KeyboardShortcutDeclaration {
	const declaration = DECLARATION_BY_ID.get(shortcutId);
	if (declaration === undefined) {
		throw new Error(`Missing keyboard shortcut declaration: ${shortcutId}`);
	}
	return declaration;
}

export function isKeyboardShortcutId(value: string): value is KeyboardShortcutId {
	return DECLARATION_BY_ID.has(value);
}

export function defaultKeyboardShortcutBindings(
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutBindings {
	return Object.freeze({
		...defaultWorkbenchShortcutBindings(platform),
		...defaultProjectedTerminalShortcutBindings(platform),
	});
}

export function findKeyboardShortcutConflict(
	bindings: KeyboardShortcutBindings,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutConflictPair | null {
	const firstIdByChord = new Map<string, KeyboardShortcutId>();
	for (const declaration of KEYBOARD_SHORTCUT_CATALOG) {
		const key = keyboardShortcutChordKey(bindings[declaration.shortcutId], platform);
		const firstShortcutId = firstIdByChord.get(key);
		if (firstShortcutId !== undefined) {
			return { firstShortcutId, secondShortcutId: declaration.shortcutId };
		}
		firstIdByChord.set(key, declaration.shortcutId);
	}
	return null;
}
