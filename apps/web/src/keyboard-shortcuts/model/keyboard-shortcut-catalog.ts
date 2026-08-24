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
	| 'multiple-panels'
	| 'directional-panel';

export interface KeyboardShortcutDeclaration {
	readonly actionId: string;
	readonly actionInput: Readonly<Record<string, unknown>> | undefined;
	readonly availability: KeyboardShortcutAvailability;
	readonly defaults: Readonly<Record<KeyboardShortcutPlatform, KeyboardShortcutChord>>;
	readonly feature: KeyboardShortcutFeature;
	readonly name:
		| 'actions-search'
		| 'clear-terminal'
		| 'close-terminal'
		| 'copy-selection'
		| 'find'
		| 'focus-panel-down'
		| 'focus-panel-left'
		| 'focus-panel-right'
		| 'focus-panel-up'
		| 'global-search'
		| 'maximize-panel'
		| 'new-terminal'
		| 'paste'
		| 'select-all'
		| 'settings'
		| 'split-below'
		| 'split-right'
		| 'toggle-sidebar';
	readonly scope: KeyboardShortcutScope;
	readonly shortcutId: KeyboardShortcutId;
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
