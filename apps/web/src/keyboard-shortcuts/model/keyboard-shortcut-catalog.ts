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
export type KeyboardShortcutScope = 'focused-terminal' | 'focused-terminal-or-find' | 'workbench';
export type KeyboardShortcutAvailability =
	| 'always-in-workbench'
	| 'focused-terminal'
	| 'focused-terminal-with-selection';

export interface KeyboardShortcutDeclaration {
	readonly actionId: string;
	readonly actionInput: Readonly<Record<string, unknown>> | undefined;
	readonly availability: KeyboardShortcutAvailability;
	readonly defaults: Readonly<Record<KeyboardShortcutPlatform, KeyboardShortcutChord>>;
	readonly feature: KeyboardShortcutFeature;
	readonly name:
		| 'actions-search'
		| 'copy-selection'
		| 'find'
		| 'global-search'
		| 'new-terminal'
		| 'select-all'
		| 'settings'
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
		actionId: PROJECTED_TERMINAL_SHORTCUTS.selectAll.actionId,
		actionInput: undefined,
		availability: 'focused-terminal',
		defaults: PROJECTED_TERMINAL_SHORTCUTS.selectAll.defaults,
		feature: 'terminal',
		name: 'select-all',
		scope: 'focused-terminal',
		shortcutId: PROJECTED_TERMINAL_SHORTCUTS.selectAll.shortcutId,
	}),
] as const satisfies readonly KeyboardShortcutDeclaration[]);

export const KEYBOARD_SHORTCUT_IDS = Object.freeze(
	KEYBOARD_SHORTCUT_CATALOG.map((declaration) => declaration.shortcutId),
);

const DECLARATION_BY_ID: Readonly<Record<KeyboardShortcutId, KeyboardShortcutDeclaration>> =
	Object.freeze({
		[WORKBENCH_SHORTCUTS.actionsSearch.shortcutId]: KEYBOARD_SHORTCUT_CATALOG[0],
		[WORKBENCH_SHORTCUTS.newTerminal.shortcutId]: KEYBOARD_SHORTCUT_CATALOG[1],
		[WORKBENCH_SHORTCUTS.search.shortcutId]: KEYBOARD_SHORTCUT_CATALOG[2],
		[WORKBENCH_SHORTCUTS.settings.shortcutId]: KEYBOARD_SHORTCUT_CATALOG[3],
		[WORKBENCH_SHORTCUTS.toggleSidebar.shortcutId]: KEYBOARD_SHORTCUT_CATALOG[4],
		[PROJECTED_TERMINAL_SHORTCUTS.findOpen.shortcutId]: KEYBOARD_SHORTCUT_CATALOG[5],
		[PROJECTED_TERMINAL_SHORTCUTS.copySelection.shortcutId]: KEYBOARD_SHORTCUT_CATALOG[6],
		[PROJECTED_TERMINAL_SHORTCUTS.selectAll.shortcutId]: KEYBOARD_SHORTCUT_CATALOG[7],
	});

interface KeyboardShortcutConflictPair {
	readonly firstShortcutId: KeyboardShortcutId;
	readonly secondShortcutId: KeyboardShortcutId;
}

export function keyboardShortcutDeclaration(
	shortcutId: KeyboardShortcutId,
): KeyboardShortcutDeclaration {
	return DECLARATION_BY_ID[shortcutId];
}

export function isKeyboardShortcutId(value: string): value is KeyboardShortcutId {
	return Object.hasOwn(DECLARATION_BY_ID, value);
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
