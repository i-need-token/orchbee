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
		| typeof WORKBENCH_ACTION_IDS.closeActiveTerminal
		| typeof WORKBENCH_ACTION_IDS.createTerminal
		| typeof WORKBENCH_ACTION_IDS.openSearch
		| typeof WORKBENCH_ACTION_IDS.openSettings
		| typeof WORKBENCH_ACTION_IDS.splitActivePanelBelow
		| typeof WORKBENCH_ACTION_IDS.splitActivePanelRight
		| typeof WORKBENCH_ACTION_IDS.toggleActivePanelMaximization
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
	closeActiveTerminal: {
		defaults: {
			apple: chord('KeyW', ['mod']),
			standard: chord('KeyW', ['mod', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.closeActiveTerminal,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.closeActiveTerminal,
	},
	maximizeActivePanel: {
		defaults: {
			apple: chord('Enter', ['mod', 'shift']),
			standard: chord('Enter', ['mod', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.toggleActivePanelMaximization,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.toggleActivePanelMaximization,
	},
	newTerminal: {
		defaults: {
			apple: chord('KeyT', ['mod']),
			standard: chord('KeyT', ['mod', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.createTerminal,
		input: { groupId: null },
		shortcutId: 'workbench.terminal.create',
	},
	search: {
		defaults: {
			apple: chord('KeyF', ['mod', 'shift']),
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
	splitActivePanelBelow: {
		defaults: {
			apple: chord('KeyD', ['mod', 'shift']),
			standard: chord('KeyE', ['mod', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.splitActivePanelBelow,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.splitActivePanelBelow,
	},
	splitActivePanelRight: {
		defaults: {
			apple: chord('KeyD', ['mod']),
			standard: chord('KeyD', ['mod', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.splitActivePanelRight,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.splitActivePanelRight,
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
	'closeActiveTerminal',
	'maximizeActivePanel',
	'newTerminal',
	'search',
	'settings',
	'splitActivePanelBelow',
	'splitActivePanelRight',
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
		case WORKBENCH_ACTION_IDS.closeActiveTerminal:
			return formatWorkbenchShortcut('closeActiveTerminal', platform, bindings);
		case WORKBENCH_ACTION_IDS.openSearch:
			return formatWorkbenchShortcut('search', platform, bindings);
		case WORKBENCH_ACTION_IDS.openSettings:
			return formatWorkbenchShortcut('settings', platform, bindings);
		case WORKBENCH_ACTION_IDS.splitActivePanelBelow:
			return formatWorkbenchShortcut('splitActivePanelBelow', platform, bindings);
		case WORKBENCH_ACTION_IDS.splitActivePanelRight:
			return formatWorkbenchShortcut('splitActivePanelRight', platform, bindings);
		case WORKBENCH_ACTION_IDS.toggleActivePanelMaximization:
			return formatWorkbenchShortcut('maximizeActivePanel', platform, bindings);
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
		[WORKBENCH_SHORTCUTS.closeActiveTerminal.shortcutId]:
			WORKBENCH_SHORTCUTS.closeActiveTerminal.defaults[platform],
		[WORKBENCH_SHORTCUTS.maximizeActivePanel.shortcutId]:
			WORKBENCH_SHORTCUTS.maximizeActivePanel.defaults[platform],
		[WORKBENCH_SHORTCUTS.newTerminal.shortcutId]:
			WORKBENCH_SHORTCUTS.newTerminal.defaults[platform],
		[WORKBENCH_SHORTCUTS.search.shortcutId]: WORKBENCH_SHORTCUTS.search.defaults[platform],
		[WORKBENCH_SHORTCUTS.settings.shortcutId]: WORKBENCH_SHORTCUTS.settings.defaults[platform],
		[WORKBENCH_SHORTCUTS.splitActivePanelBelow.shortcutId]:
			WORKBENCH_SHORTCUTS.splitActivePanelBelow.defaults[platform],
		[WORKBENCH_SHORTCUTS.splitActivePanelRight.shortcutId]:
			WORKBENCH_SHORTCUTS.splitActivePanelRight.defaults[platform],
		[WORKBENCH_SHORTCUTS.toggleSidebar.shortcutId]:
			WORKBENCH_SHORTCUTS.toggleSidebar.defaults[platform],
	});
}
