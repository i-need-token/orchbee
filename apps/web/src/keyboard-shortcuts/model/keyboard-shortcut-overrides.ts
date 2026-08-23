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
	let actionsSearch = defaults[WORKBENCH_SHORTCUTS.actionsSearch.shortcutId];
	let closeActiveTerminal = defaults[WORKBENCH_SHORTCUTS.closeActiveTerminal.shortcutId];
	let maximizeActivePanel = defaults[WORKBENCH_SHORTCUTS.maximizeActivePanel.shortcutId];
	let newTerminal = defaults[WORKBENCH_SHORTCUTS.newTerminal.shortcutId];
	let search = defaults[WORKBENCH_SHORTCUTS.search.shortcutId];
	let settings = defaults[WORKBENCH_SHORTCUTS.settings.shortcutId];
	let splitActivePanelBelow = defaults[WORKBENCH_SHORTCUTS.splitActivePanelBelow.shortcutId];
	let splitActivePanelRight = defaults[WORKBENCH_SHORTCUTS.splitActivePanelRight.shortcutId];
	let toggleSidebar = defaults[WORKBENCH_SHORTCUTS.toggleSidebar.shortcutId];
	let clear = defaults[PROJECTED_TERMINAL_SHORTCUTS.clear.shortcutId];
	let findOpen = defaults[PROJECTED_TERMINAL_SHORTCUTS.findOpen.shortcutId];
	let copySelection = defaults[PROJECTED_TERMINAL_SHORTCUTS.copySelection.shortcutId];
	let paste = defaults[PROJECTED_TERMINAL_SHORTCUTS.paste.shortcutId];
	let selectAll = defaults[PROJECTED_TERMINAL_SHORTCUTS.selectAll.shortcutId];

	for (const override of overrides) {
		switch (override.shortcutId) {
			case WORKBENCH_SHORTCUTS.actionsSearch.shortcutId:
				actionsSearch = override.chord;
				break;
			case WORKBENCH_SHORTCUTS.closeActiveTerminal.shortcutId:
				closeActiveTerminal = override.chord;
				break;
			case WORKBENCH_SHORTCUTS.maximizeActivePanel.shortcutId:
				maximizeActivePanel = override.chord;
				break;
			case WORKBENCH_SHORTCUTS.newTerminal.shortcutId:
				newTerminal = override.chord;
				break;
			case WORKBENCH_SHORTCUTS.search.shortcutId:
				search = override.chord;
				break;
			case WORKBENCH_SHORTCUTS.settings.shortcutId:
				settings = override.chord;
				break;
			case WORKBENCH_SHORTCUTS.splitActivePanelBelow.shortcutId:
				splitActivePanelBelow = override.chord;
				break;
			case WORKBENCH_SHORTCUTS.splitActivePanelRight.shortcutId:
				splitActivePanelRight = override.chord;
				break;
			case WORKBENCH_SHORTCUTS.toggleSidebar.shortcutId:
				toggleSidebar = override.chord;
				break;
			case PROJECTED_TERMINAL_SHORTCUTS.findOpen.shortcutId:
				findOpen = override.chord;
				break;
			case PROJECTED_TERMINAL_SHORTCUTS.clear.shortcutId:
				clear = override.chord;
				break;
			case PROJECTED_TERMINAL_SHORTCUTS.copySelection.shortcutId:
				copySelection = override.chord;
				break;
			case PROJECTED_TERMINAL_SHORTCUTS.paste.shortcutId:
				paste = override.chord;
				break;
			case PROJECTED_TERMINAL_SHORTCUTS.selectAll.shortcutId:
				selectAll = override.chord;
				break;
		}
	}

	return Object.freeze({
		[WORKBENCH_SHORTCUTS.actionsSearch.shortcutId]: actionsSearch,
		[WORKBENCH_SHORTCUTS.closeActiveTerminal.shortcutId]: closeActiveTerminal,
		[WORKBENCH_SHORTCUTS.maximizeActivePanel.shortcutId]: maximizeActivePanel,
		[WORKBENCH_SHORTCUTS.newTerminal.shortcutId]: newTerminal,
		[WORKBENCH_SHORTCUTS.search.shortcutId]: search,
		[WORKBENCH_SHORTCUTS.settings.shortcutId]: settings,
		[WORKBENCH_SHORTCUTS.splitActivePanelBelow.shortcutId]: splitActivePanelBelow,
		[WORKBENCH_SHORTCUTS.splitActivePanelRight.shortcutId]: splitActivePanelRight,
		[WORKBENCH_SHORTCUTS.toggleSidebar.shortcutId]: toggleSidebar,
		[PROJECTED_TERMINAL_SHORTCUTS.clear.shortcutId]: clear,
		[PROJECTED_TERMINAL_SHORTCUTS.findOpen.shortcutId]: findOpen,
		[PROJECTED_TERMINAL_SHORTCUTS.copySelection.shortcutId]: copySelection,
		[PROJECTED_TERMINAL_SHORTCUTS.paste.shortcutId]: paste,
		[PROJECTED_TERMINAL_SHORTCUTS.selectAll.shortcutId]: selectAll,
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
