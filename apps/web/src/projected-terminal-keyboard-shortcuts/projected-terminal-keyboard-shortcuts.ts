import type {
	KeyboardShortcutChord,
	KeyboardShortcutEvent,
	KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';
import { matchesKeyboardShortcutChord } from '@orchbee/keyboard-shortcuts';

export const PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS = Object.freeze({
	copySelection: 'terminal.selection.copy',
	findOpen: 'terminal.search.open',
	selectAll: 'terminal.selection.select-all',
} as const);

export type ProjectedTerminalShortcutPlatform = KeyboardShortcutPlatform;
export type ProjectedTerminalShortcutFocus = 'find' | 'outside' | 'terminal';

const chord = (
	code: string,
	modifiers: KeyboardShortcutChord['modifiers'],
): KeyboardShortcutChord => Object.freeze({ code, modifiers: Object.freeze([...modifiers]) });

export const PROJECTED_TERMINAL_SHORTCUTS = {
	copySelection: {
		actionId: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.copySelection,
		defaults: {
			apple: chord('KeyC', ['mod']),
			standard: chord('KeyC', ['mod', 'shift']),
		},
		focus: 'terminal',
		shortcutId: 'terminal.selection.copy',
	},
	findOpen: {
		actionId: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.findOpen,
		defaults: {
			apple: chord('KeyF', ['mod']),
			standard: chord('KeyF', ['mod', 'shift']),
		},
		focus: 'terminal-or-find',
		shortcutId: 'terminal.search.open',
	},
	selectAll: {
		actionId: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.selectAll,
		defaults: {
			apple: chord('KeyA', ['mod']),
			standard: chord('KeyA', ['mod', 'shift']),
		},
		focus: 'terminal',
		shortcutId: 'terminal.selection.select-all',
	},
} as const;

type ProjectedTerminalShortcutAction = keyof typeof PROJECTED_TERMINAL_SHORTCUTS;
export type ProjectedTerminalShortcutId =
	(typeof PROJECTED_TERMINAL_SHORTCUTS)[ProjectedTerminalShortcutAction]['shortcutId'];
export type ProjectedTerminalShortcutBindings = Readonly<
	Record<ProjectedTerminalShortcutId, KeyboardShortcutChord>
>;

export interface ProjectedTerminalShortcutBindingSource {
	current(): ProjectedTerminalShortcutBindings;
}

export interface ProjectedTerminalShortcutBinding {
	readonly action: ProjectedTerminalShortcutAction;
	readonly actionId:
		| typeof PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.copySelection
		| typeof PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.findOpen
		| typeof PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.selectAll;
	readonly shortcutId: ProjectedTerminalShortcutId;
}

const PROJECTED_TERMINAL_SHORTCUT_ACTIONS = Object.freeze([
	'copySelection',
	'findOpen',
	'selectAll',
] as const satisfies readonly ProjectedTerminalShortcutAction[]);

const DEFAULT_PROJECTED_TERMINAL_SHORTCUT_BINDINGS: Readonly<
	Record<ProjectedTerminalShortcutPlatform, ProjectedTerminalShortcutBindings>
> = Object.freeze({
	apple: defaultBindingsFor('apple'),
	standard: defaultBindingsFor('standard'),
});

export function defaultProjectedTerminalShortcutBindings(
	platform: ProjectedTerminalShortcutPlatform,
): ProjectedTerminalShortcutBindings {
	return DEFAULT_PROJECTED_TERMINAL_SHORTCUT_BINDINGS[platform];
}

export function resolveProjectedTerminalShortcut(
	event: KeyboardShortcutEvent,
	platform: ProjectedTerminalShortcutPlatform,
	focus: ProjectedTerminalShortcutFocus,
	bindings: ProjectedTerminalShortcutBindings = defaultProjectedTerminalShortcutBindings(platform),
): ProjectedTerminalShortcutBinding | null {
	if (focus === 'outside') return null;
	for (const action of PROJECTED_TERMINAL_SHORTCUT_ACTIONS) {
		const definition = PROJECTED_TERMINAL_SHORTCUTS[action];
		if (focus === 'find' && definition.focus !== 'terminal-or-find') continue;
		const shortcutId: ProjectedTerminalShortcutId = definition.shortcutId;
		if (!matchesKeyboardShortcutChord(event, bindings[shortcutId], platform)) continue;
		return {
			action,
			actionId: definition.actionId,
			shortcutId,
		};
	}
	return null;
}

function defaultBindingsFor(
	platform: ProjectedTerminalShortcutPlatform,
): ProjectedTerminalShortcutBindings {
	return Object.freeze({
		[PROJECTED_TERMINAL_SHORTCUTS.copySelection.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.copySelection.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.findOpen.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.findOpen.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.selectAll.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.selectAll.defaults[platform],
	});
}
