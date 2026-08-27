import {
	formatKeyboardShortcutChord,
	matchesKeyboardShortcutChord,
	type KeyboardShortcutChord,
	type KeyboardShortcutEvent,
} from '@orchbee/keyboard-shortcuts';

import { WORKBENCH_ACTION_IDS } from '../actions/workbench-action-ids';
import type { WorkbenchTabPositionInput } from '../actions/workbench-action-ids';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';

interface WorkbenchShortcutDefinition {
	readonly defaults: Readonly<Record<WorkbenchShortcutPlatform, KeyboardShortcutChord>>;
	readonly id:
		| typeof WORKBENCH_ACTION_IDS.closeActiveTerminal
		| typeof WORKBENCH_ACTION_IDS.createTerminal
		| typeof WORKBENCH_ACTION_IDS.focusActivePanelDown
		| typeof WORKBENCH_ACTION_IDS.focusActivePanelLeft
		| typeof WORKBENCH_ACTION_IDS.focusActivePanelRight
		| typeof WORKBENCH_ACTION_IDS.focusActivePanelUp
		| typeof WORKBENCH_ACTION_IDS.openSearch
		| typeof WORKBENCH_ACTION_IDS.openSettings
		| typeof WORKBENCH_ACTION_IDS.selectNextTab
		| typeof WORKBENCH_ACTION_IDS.selectPreviousTab
		| typeof WORKBENCH_ACTION_IDS.selectTabByPosition
		| typeof WORKBENCH_ACTION_IDS.splitActivePanelBelow
		| typeof WORKBENCH_ACTION_IDS.splitActivePanelRight
		| typeof WORKBENCH_ACTION_IDS.toggleActivePanelMaximization
		| typeof WORKBENCH_ACTION_IDS.toggleSidebar;
	readonly input:
		| { readonly filter: 'actions' | 'all' }
		| { readonly groupId: null }
		| WorkbenchTabPositionInput
		| undefined;
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
	focusActivePanelDown: {
		defaults: {
			apple: chord('ArrowDown', ['alt', 'mod']),
			standard: chord('ArrowDown', ['alt', 'mod']),
		},
		id: WORKBENCH_ACTION_IDS.focusActivePanelDown,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.focusActivePanelDown,
	},
	focusActivePanelLeft: {
		defaults: {
			apple: chord('ArrowLeft', ['alt', 'mod']),
			standard: chord('ArrowLeft', ['alt', 'mod']),
		},
		id: WORKBENCH_ACTION_IDS.focusActivePanelLeft,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.focusActivePanelLeft,
	},
	focusActivePanelRight: {
		defaults: {
			apple: chord('ArrowRight', ['alt', 'mod']),
			standard: chord('ArrowRight', ['alt', 'mod']),
		},
		id: WORKBENCH_ACTION_IDS.focusActivePanelRight,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.focusActivePanelRight,
	},
	focusActivePanelUp: {
		defaults: {
			apple: chord('ArrowUp', ['alt', 'mod']),
			standard: chord('ArrowUp', ['alt', 'mod']),
		},
		id: WORKBENCH_ACTION_IDS.focusActivePanelUp,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.focusActivePanelUp,
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
	selectNextTab: {
		defaults: {
			apple: chord('BracketRight', ['mod', 'shift']),
			standard: chord('PageDown', ['mod']),
		},
		id: WORKBENCH_ACTION_IDS.selectNextTab,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.selectNextTab,
	},
	selectPreviousTab: {
		defaults: {
			apple: chord('BracketLeft', ['mod', 'shift']),
			standard: chord('PageUp', ['mod']),
		},
		id: WORKBENCH_ACTION_IDS.selectPreviousTab,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.selectPreviousTab,
	},
	selectTab1: positionalTabShortcut(1),
	selectTab2: positionalTabShortcut(2),
	selectTab3: positionalTabShortcut(3),
	selectTab4: positionalTabShortcut(4),
	selectTab5: positionalTabShortcut(5),
	selectTab6: positionalTabShortcut(6),
	selectTab7: positionalTabShortcut(7),
	selectTab8: positionalTabShortcut(8),
	selectTab9: positionalTabShortcut(9),
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
	'focusActivePanelDown',
	'focusActivePanelLeft',
	'focusActivePanelRight',
	'focusActivePanelUp',
	'maximizeActivePanel',
	'newTerminal',
	'search',
	'selectNextTab',
	'selectPreviousTab',
	'selectTab1',
	'selectTab2',
	'selectTab3',
	'selectTab4',
	'selectTab5',
	'selectTab6',
	'selectTab7',
	'selectTab8',
	'selectTab9',
	'settings',
	'splitActivePanelBelow',
	'splitActivePanelRight',
	'toggleSidebar',
] as const satisfies readonly WorkbenchShortcutAction[]);

function positionalTabShortcut<const Position extends WorkbenchTabPositionInput['position']>(
	position: Position,
) {
	return {
		defaults: {
			apple: chord(`Digit${position}`, ['mod']),
			standard: chord(`Digit${position}`, ['mod']),
		},
		id: WORKBENCH_ACTION_IDS.selectTabByPosition,
		input: { position },
		shortcutId: `workbench.tab.select.${position}`,
	} as const;
}

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
		case WORKBENCH_ACTION_IDS.focusActivePanelDown:
			return formatWorkbenchShortcut('focusActivePanelDown', platform, bindings);
		case WORKBENCH_ACTION_IDS.focusActivePanelLeft:
			return formatWorkbenchShortcut('focusActivePanelLeft', platform, bindings);
		case WORKBENCH_ACTION_IDS.focusActivePanelRight:
			return formatWorkbenchShortcut('focusActivePanelRight', platform, bindings);
		case WORKBENCH_ACTION_IDS.focusActivePanelUp:
			return formatWorkbenchShortcut('focusActivePanelUp', platform, bindings);
		case WORKBENCH_ACTION_IDS.openSearch:
			return formatWorkbenchShortcut('search', platform, bindings);
		case WORKBENCH_ACTION_IDS.openSettings:
			return formatWorkbenchShortcut('settings', platform, bindings);
		case WORKBENCH_ACTION_IDS.selectNextTab:
			return formatWorkbenchShortcut('selectNextTab', platform, bindings);
		case WORKBENCH_ACTION_IDS.selectPreviousTab:
			return formatWorkbenchShortcut('selectPreviousTab', platform, bindings);
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
		[WORKBENCH_SHORTCUTS.focusActivePanelDown.shortcutId]:
			WORKBENCH_SHORTCUTS.focusActivePanelDown.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusActivePanelLeft.shortcutId]:
			WORKBENCH_SHORTCUTS.focusActivePanelLeft.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusActivePanelRight.shortcutId]:
			WORKBENCH_SHORTCUTS.focusActivePanelRight.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusActivePanelUp.shortcutId]:
			WORKBENCH_SHORTCUTS.focusActivePanelUp.defaults[platform],
		[WORKBENCH_SHORTCUTS.maximizeActivePanel.shortcutId]:
			WORKBENCH_SHORTCUTS.maximizeActivePanel.defaults[platform],
		[WORKBENCH_SHORTCUTS.newTerminal.shortcutId]:
			WORKBENCH_SHORTCUTS.newTerminal.defaults[platform],
		[WORKBENCH_SHORTCUTS.search.shortcutId]: WORKBENCH_SHORTCUTS.search.defaults[platform],
		[WORKBENCH_SHORTCUTS.selectNextTab.shortcutId]:
			WORKBENCH_SHORTCUTS.selectNextTab.defaults[platform],
		[WORKBENCH_SHORTCUTS.selectPreviousTab.shortcutId]:
			WORKBENCH_SHORTCUTS.selectPreviousTab.defaults[platform],
		[WORKBENCH_SHORTCUTS.selectTab1.shortcutId]: WORKBENCH_SHORTCUTS.selectTab1.defaults[platform],
		[WORKBENCH_SHORTCUTS.selectTab2.shortcutId]: WORKBENCH_SHORTCUTS.selectTab2.defaults[platform],
		[WORKBENCH_SHORTCUTS.selectTab3.shortcutId]: WORKBENCH_SHORTCUTS.selectTab3.defaults[platform],
		[WORKBENCH_SHORTCUTS.selectTab4.shortcutId]: WORKBENCH_SHORTCUTS.selectTab4.defaults[platform],
		[WORKBENCH_SHORTCUTS.selectTab5.shortcutId]: WORKBENCH_SHORTCUTS.selectTab5.defaults[platform],
		[WORKBENCH_SHORTCUTS.selectTab6.shortcutId]: WORKBENCH_SHORTCUTS.selectTab6.defaults[platform],
		[WORKBENCH_SHORTCUTS.selectTab7.shortcutId]: WORKBENCH_SHORTCUTS.selectTab7.defaults[platform],
		[WORKBENCH_SHORTCUTS.selectTab8.shortcutId]: WORKBENCH_SHORTCUTS.selectTab8.defaults[platform],
		[WORKBENCH_SHORTCUTS.selectTab9.shortcutId]: WORKBENCH_SHORTCUTS.selectTab9.defaults[platform],
		[WORKBENCH_SHORTCUTS.settings.shortcutId]: WORKBENCH_SHORTCUTS.settings.defaults[platform],
		[WORKBENCH_SHORTCUTS.splitActivePanelBelow.shortcutId]:
			WORKBENCH_SHORTCUTS.splitActivePanelBelow.defaults[platform],
		[WORKBENCH_SHORTCUTS.splitActivePanelRight.shortcutId]:
			WORKBENCH_SHORTCUTS.splitActivePanelRight.defaults[platform],
		[WORKBENCH_SHORTCUTS.toggleSidebar.shortcutId]:
			WORKBENCH_SHORTCUTS.toggleSidebar.defaults[platform],
	});
}
