import type {
	KeyboardShortcutChord,
	KeyboardShortcutEvent,
	KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';
import {
	formatKeyboardShortcutChord,
	matchesKeyboardShortcutChord,
} from '@orchbee/keyboard-shortcuts';

export const PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS = Object.freeze({
	clear: 'terminal.clear',
	copySelection: 'terminal.selection.copy',
	findOpen: 'terminal.search.open',
	hintsOpen: 'terminal.hints.open',
	paste: 'terminal.clipboard.paste',
	promptNext: 'terminal.history.prompt-next',
	promptPrevious: 'terminal.history.prompt-previous',
	scrollLineDown: 'terminal.history.scroll-line-down',
	scrollLineUp: 'terminal.history.scroll-line-up',
	scrollPageDown: 'terminal.history.scroll-page-down',
	scrollPageUp: 'terminal.history.scroll-page-up',
	scrollToBottom: 'terminal.history.scroll-bottom',
	scrollToTop: 'terminal.history.scroll-top',
	selectAll: 'terminal.selection.select-all',
} as const);

export type ProjectedTerminalShortcutPlatform = KeyboardShortcutPlatform;
export type ProjectedTerminalShortcutFocus = 'find' | 'outside' | 'terminal';

const chord = (
	code: string,
	modifiers: KeyboardShortcutChord['modifiers'],
): KeyboardShortcutChord => Object.freeze({ code, modifiers: Object.freeze([...modifiers]) });

export const PROJECTED_TERMINAL_SHORTCUTS = {
	clear: {
		actionId: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.clear,
		defaults: {
			apple: chord('KeyK', ['mod']),
			standard: chord('KeyL', ['mod', 'shift']),
		},
		focus: 'terminal',
		shortcutId: 'terminal.clear',
	},
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
	hintsOpen: {
		actionId: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.hintsOpen,
		defaults: {
			apple: chord('KeyO', ['mod', 'shift']),
			standard: chord('KeyO', ['mod', 'shift']),
		},
		focus: 'terminal',
		shortcutId: 'terminal.hints.open',
	},
	paste: {
		actionId: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.paste,
		defaults: {
			apple: chord('KeyV', ['mod']),
			standard: chord('KeyV', ['mod', 'shift']),
		},
		focus: 'terminal',
		shortcutId: 'terminal.clipboard.paste',
	},
	promptNext: {
		actionId: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.promptNext,
		defaults: {
			apple: chord('ArrowDown', ['mod']),
			standard: chord('ArrowDown', ['mod']),
		},
		focus: 'terminal',
		shortcutId: 'terminal.history.prompt-next',
	},
	promptPrevious: {
		actionId: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.promptPrevious,
		defaults: {
			apple: chord('ArrowUp', ['mod']),
			standard: chord('ArrowUp', ['mod']),
		},
		focus: 'terminal',
		shortcutId: 'terminal.history.prompt-previous',
	},
	scrollLineDown: {
		actionId: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.scrollLineDown,
		defaults: {
			apple: chord('ArrowDown', ['control', 'shift']),
			standard: chord('ArrowDown', ['control', 'shift']),
		},
		focus: 'terminal',
		shortcutId: 'terminal.history.scroll-line-down',
	},
	scrollLineUp: {
		actionId: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.scrollLineUp,
		defaults: {
			apple: chord('ArrowUp', ['control', 'shift']),
			standard: chord('ArrowUp', ['control', 'shift']),
		},
		focus: 'terminal',
		shortcutId: 'terminal.history.scroll-line-up',
	},
	scrollPageDown: {
		actionId: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.scrollPageDown,
		defaults: {
			apple: chord('PageDown', ['control', 'shift']),
			standard: chord('PageDown', ['control', 'shift']),
		},
		focus: 'terminal',
		shortcutId: 'terminal.history.scroll-page-down',
	},
	scrollPageUp: {
		actionId: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.scrollPageUp,
		defaults: {
			apple: chord('PageUp', ['control', 'shift']),
			standard: chord('PageUp', ['control', 'shift']),
		},
		focus: 'terminal',
		shortcutId: 'terminal.history.scroll-page-up',
	},
	scrollToBottom: {
		actionId: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.scrollToBottom,
		defaults: {
			apple: chord('End', ['control', 'shift']),
			standard: chord('End', ['control', 'shift']),
		},
		focus: 'terminal',
		shortcutId: 'terminal.history.scroll-bottom',
	},
	scrollToTop: {
		actionId: PROJECTED_TERMINAL_SHORTCUT_ACTION_IDS.scrollToTop,
		defaults: {
			apple: chord('Home', ['control', 'shift']),
			standard: chord('Home', ['control', 'shift']),
		},
		focus: 'terminal',
		shortcutId: 'terminal.history.scroll-top',
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
type ProjectedTerminalShortcutActionId =
	(typeof PROJECTED_TERMINAL_SHORTCUTS)[ProjectedTerminalShortcutAction]['actionId'];
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
	readonly actionId: ProjectedTerminalShortcutActionId;
	readonly nativePasteFallback: boolean;
	readonly shortcutId: ProjectedTerminalShortcutId;
}

const PROJECTED_TERMINAL_SHORTCUT_ACTIONS = Object.freeze([
	'clear',
	'copySelection',
	'findOpen',
	'hintsOpen',
	'paste',
	'promptPrevious',
	'promptNext',
	'scrollLineUp',
	'scrollLineDown',
	'scrollPageUp',
	'scrollPageDown',
	'scrollToTop',
	'scrollToBottom',
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
			nativePasteFallback:
				action === 'paste' &&
				matchesKeyboardShortcutChord(
					event,
					PROJECTED_TERMINAL_SHORTCUTS.paste.defaults[platform],
					platform,
				),
			shortcutId,
		};
	}
	return null;
}

function defaultBindingsFor(
	platform: ProjectedTerminalShortcutPlatform,
): ProjectedTerminalShortcutBindings {
	return Object.freeze({
		[PROJECTED_TERMINAL_SHORTCUTS.clear.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.clear.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.copySelection.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.copySelection.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.findOpen.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.findOpen.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.hintsOpen.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.hintsOpen.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.paste.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.paste.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.promptNext.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.promptNext.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.promptPrevious.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.promptPrevious.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.scrollLineDown.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.scrollLineDown.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.scrollLineUp.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.scrollLineUp.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.scrollPageDown.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.scrollPageDown.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.scrollPageUp.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.scrollPageUp.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.scrollToBottom.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.scrollToBottom.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.scrollToTop.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.scrollToTop.defaults[platform],
		[PROJECTED_TERMINAL_SHORTCUTS.selectAll.shortcutId]:
			PROJECTED_TERMINAL_SHORTCUTS.selectAll.defaults[platform],
	});
}

export function formatProjectedTerminalActionShortcut(
	actionId: ProjectedTerminalShortcutActionId,
	platform: ProjectedTerminalShortcutPlatform,
	bindings?: ProjectedTerminalShortcutBindings,
): string;
export function formatProjectedTerminalActionShortcut(
	actionId: string,
	platform: ProjectedTerminalShortcutPlatform,
	bindings?: ProjectedTerminalShortcutBindings,
): string | null;
export function formatProjectedTerminalActionShortcut(
	actionId: string,
	platform: ProjectedTerminalShortcutPlatform,
	bindings: ProjectedTerminalShortcutBindings = defaultProjectedTerminalShortcutBindings(platform),
): string | null {
	for (const action of PROJECTED_TERMINAL_SHORTCUT_ACTIONS) {
		const definition = PROJECTED_TERMINAL_SHORTCUTS[action];
		if (definition.actionId !== actionId) continue;
		return formatKeyboardShortcutChord(bindings[definition.shortcutId], platform);
	}
	return null;
}
