import {
	formatKeyboardShortcutChord,
	matchesKeyboardShortcutChord,
	type KeyboardShortcutChord,
	type KeyboardShortcutEvent,
} from '@orchbee/keyboard-shortcuts';

import { WORKBENCH_ACTION_IDS } from '../actions/workbench-action-ids';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';

interface WorkbenchShortcutDefinition {
	readonly defaults: Readonly<Record<WorkbenchShortcutPlatform, KeyboardShortcutChord>>;
	readonly id:
		| typeof WORKBENCH_ACTION_IDS.createTerminal
		| typeof WORKBENCH_ACTION_IDS.openSearch
		| typeof WORKBENCH_ACTION_IDS.openSettings
		| typeof WORKBENCH_ACTION_IDS.toggleSidebar;
	readonly input: { readonly filter: 'actions' | 'all' } | { readonly groupId: null } | undefined;
	readonly shortcutId: string;
}

const chord = (
	code: string,
	modifiers: KeyboardShortcutChord['modifiers'],
): KeyboardShortcutChord => Object.freeze({ code, modifiers: Object.freeze([...modifiers]) });

export const WORKBENCH_SHORTCUTS = {
	actionsSearch: {
		defaults: {
			apple: chord('KeyP', ['mod', 'shift']),
			standard: chord('KeyP', ['mod', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.openSearch,
		input: { filter: 'actions' },
		shortcutId: 'workbench.search.actions.open',
	},
	newTerminal: {
		defaults: {
			apple: chord('KeyN', ['mod']),
			standard: chord('KeyN', ['mod', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.createTerminal,
		input: { groupId: null },
		shortcutId: 'workbench.terminal.create',
	},
	search: {
		defaults: {
			apple: chord('KeyK', ['mod']),
			standard: chord('KeyK', ['mod', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.openSearch,
		input: { filter: 'all' },
		shortcutId: 'workbench.search.all.open',
	},
	settings: {
		defaults: {
			apple: chord('Comma', ['mod']),
			standard: chord('Comma', ['mod']),
		},
		id: WORKBENCH_ACTION_IDS.openSettings,
		input: undefined,
		shortcutId: 'workbench.settings.open',
	},
	toggleSidebar: {
		defaults: {
			apple: chord('KeyB', ['mod']),
			standard: chord('KeyB', ['mod', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.toggleSidebar,
		input: undefined,
		shortcutId: 'workbench.sidebar.toggle',
	},
} as const satisfies Readonly<Record<string, WorkbenchShortcutDefinition>>;

export type WorkbenchShortcutAction = keyof typeof WORKBENCH_SHORTCUTS;
export type WorkbenchShortcutId =
	(typeof WORKBENCH_SHORTCUTS)[WorkbenchShortcutAction]['shortcutId'];
export type WorkbenchShortcutBindings = Readonly<
	Record<WorkbenchShortcutId, KeyboardShortcutChord>
>;

export interface WorkbenchShortcutBinding {
	readonly action: WorkbenchShortcutAction;
	readonly chord: KeyboardShortcutChord;
	readonly shortcutId: WorkbenchShortcutId;
}

const WORKBENCH_SHORTCUT_ACTIONS = Object.freeze([
	'actionsSearch',
	'newTerminal',
	'search',
	'settings',
	'toggleSidebar',
] as const satisfies readonly WorkbenchShortcutAction[]);

const DEFAULT_WORKBENCH_SHORTCUT_BINDINGS: Readonly<
	Record<WorkbenchShortcutPlatform, WorkbenchShortcutBindings>
> = Object.freeze({
	apple: defaultBindingsFor('apple'),
	standard: defaultBindingsFor('standard'),
});

export function defaultWorkbenchShortcutBindings(
	platform: WorkbenchShortcutPlatform,
): WorkbenchShortcutBindings {
	return DEFAULT_WORKBENCH_SHORTCUT_BINDINGS[platform];
}

export function formatWorkbenchShortcut(
	action: WorkbenchShortcutAction,
	platform: WorkbenchShortcutPlatform,
	bindings: WorkbenchShortcutBindings = defaultWorkbenchShortcutBindings(platform),
): string {
	const shortcut = WORKBENCH_SHORTCUTS[action];
	return formatKeyboardShortcutChord(bindings[shortcut.shortcutId], platform);
}

export function formatWorkbenchActionShortcut(
	actionId: string,
	platform: WorkbenchShortcutPlatform,
	bindings: WorkbenchShortcutBindings = defaultWorkbenchShortcutBindings(platform),
): string | null {
	switch (actionId) {
		case WORKBENCH_ACTION_IDS.createTerminal:
			return formatWorkbenchShortcut('newTerminal', platform, bindings);
		case WORKBENCH_ACTION_IDS.openSearch:
			return formatWorkbenchShortcut('search', platform, bindings);
		case WORKBENCH_ACTION_IDS.openSettings:
			return formatWorkbenchShortcut('settings', platform, bindings);
		case WORKBENCH_ACTION_IDS.toggleSidebar:
			return formatWorkbenchShortcut('toggleSidebar', platform, bindings);
		default:
			return null;
	}
}

export function resolveWorkbenchShortcut(
	event: KeyboardShortcutEvent,
	platform: WorkbenchShortcutPlatform,
	bindings: WorkbenchShortcutBindings = defaultWorkbenchShortcutBindings(platform),
): WorkbenchShortcutBinding | null {
	for (const action of WORKBENCH_SHORTCUT_ACTIONS) {
		const definition = WORKBENCH_SHORTCUTS[action];
		const shortcutId: WorkbenchShortcutId = definition.shortcutId;
		const currentChord = bindings[shortcutId];
		if (matchesKeyboardShortcutChord(event, currentChord, platform)) {
			return {
				action,
				chord: currentChord,
				shortcutId,
			};
		}
	}
	return null;
}

function defaultBindingsFor(platform: WorkbenchShortcutPlatform): WorkbenchShortcutBindings {
	return Object.freeze({
		[WORKBENCH_SHORTCUTS.actionsSearch.shortcutId]:
			WORKBENCH_SHORTCUTS.actionsSearch.defaults[platform],
		[WORKBENCH_SHORTCUTS.newTerminal.shortcutId]:
			WORKBENCH_SHORTCUTS.newTerminal.defaults[platform],
		[WORKBENCH_SHORTCUTS.search.shortcutId]: WORKBENCH_SHORTCUTS.search.defaults[platform],
		[WORKBENCH_SHORTCUTS.settings.shortcutId]: WORKBENCH_SHORTCUTS.settings.defaults[platform],
		[WORKBENCH_SHORTCUTS.toggleSidebar.shortcutId]:
			WORKBENCH_SHORTCUTS.toggleSidebar.defaults[platform],
	});
}
