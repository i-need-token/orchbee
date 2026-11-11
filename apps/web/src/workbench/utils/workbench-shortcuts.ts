import {
	formatKeyboardShortcutChord,
	matchesKeyboardShortcutChord,
	type KeyboardShortcutChord,
	type KeyboardShortcutEvent,
} from '@orchbee/keyboard-shortcuts';

import { WORKBENCH_ACTION_IDS } from '../actions/workbench-action-ids';
import type {
	WorkbenchPanelPositionInput,
	WorkbenchTabPositionInput,
} from '../actions/workbench-action-ids';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';

interface WorkbenchShortcutDefinition {
	readonly defaults: Readonly<Record<WorkbenchShortcutPlatform, KeyboardShortcutChord>>;
	readonly id:
		| typeof WORKBENCH_ACTION_IDS.closeActiveTerminal
		| typeof WORKBENCH_ACTION_IDS.createTerminal
		| typeof WORKBENCH_ACTION_IDS.createWorkspace
		| typeof WORKBENCH_ACTION_IDS.createTerminalInCurrentDirectory
		| typeof WORKBENCH_ACTION_IDS.decreaseTerminalFontSize
		| typeof WORKBENCH_ACTION_IDS.equalizeActivePanels
		| typeof WORKBENCH_ACTION_IDS.focusActivePanelDown
		| typeof WORKBENCH_ACTION_IDS.focusActivePanelLeft
		| typeof WORKBENCH_ACTION_IDS.focusActivePanelByPosition
		| typeof WORKBENCH_ACTION_IDS.focusActivePanelNext
		| typeof WORKBENCH_ACTION_IDS.focusActivePanelPrevious
		| typeof WORKBENCH_ACTION_IDS.focusActivePanelRight
		| typeof WORKBENCH_ACTION_IDS.focusActivePanelUp
		| typeof WORKBENCH_ACTION_IDS.focusActivePanelVisible
		| typeof WORKBENCH_ACTION_IDS.focusLastPanel
		| typeof WORKBENCH_ACTION_IDS.focusNextRegion
		| typeof WORKBENCH_ACTION_IDS.focusPreviousRegion
		| typeof WORKBENCH_ACTION_IDS.increaseTerminalFontSize
		| typeof WORKBENCH_ACTION_IDS.moveActivePanelDown
		| typeof WORKBENCH_ACTION_IDS.moveActivePanelLeft
		| typeof WORKBENCH_ACTION_IDS.moveActivePanelRight
		| typeof WORKBENCH_ACTION_IDS.moveActivePanelUp
		| typeof WORKBENCH_ACTION_IDS.moveActiveTabBackward
		| typeof WORKBENCH_ACTION_IDS.moveActiveTabForward
		| typeof WORKBENCH_ACTION_IDS.setTabTitle
		| typeof WORKBENCH_ACTION_IDS.openSearch
		| typeof WORKBENCH_ACTION_IDS.openSettings
		| typeof WORKBENCH_ACTION_IDS.openShortcuts
		| typeof WORKBENCH_ACTION_IDS.openAgentStart
		| typeof WORKBENCH_ACTION_IDS.resetTerminalFontSize
		| typeof WORKBENCH_ACTION_IDS.resizeActivePanel
		| typeof WORKBENCH_ACTION_IDS.selectNextTab
		| typeof WORKBENCH_ACTION_IDS.selectPreviousTab
		| typeof WORKBENCH_ACTION_IDS.selectTabByPosition
		| typeof WORKBENCH_ACTION_IDS.splitActivePanelBelow
		| typeof WORKBENCH_ACTION_IDS.splitActivePanelRight
		| typeof WORKBENCH_ACTION_IDS.swapActivePanelVisible
		| typeof WORKBENCH_ACTION_IDS.toggleActivePanelMaximization
		| typeof WORKBENCH_ACTION_IDS.toggleSidebar;
	readonly input:
		| { readonly filter: 'actions' | 'all' }
		| { readonly groupId: null }
		| WorkbenchPanelPositionInput
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
	decreaseTerminalFontSize: {
		defaults: {
			apple: chord('Minus', ['mod']),
			standard: chord('Minus', ['mod']),
		},
		id: WORKBENCH_ACTION_IDS.decreaseTerminalFontSize,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.decreaseTerminalFontSize,
	},
	equalizePanels: {
		defaults: {
			apple: chord('Equal', ['control', 'mod']),
			standard: chord('Equal', ['alt', 'mod']),
		},
		id: WORKBENCH_ACTION_IDS.equalizeActivePanels,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.equalizeActivePanels,
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
	focusPanel1: positionalPanelShortcut(1),
	focusPanel2: positionalPanelShortcut(2),
	focusPanel3: positionalPanelShortcut(3),
	focusPanel4: positionalPanelShortcut(4),
	focusPanel5: positionalPanelShortcut(5),
	focusPanel6: positionalPanelShortcut(6),
	focusPanel7: positionalPanelShortcut(7),
	focusPanel8: positionalPanelShortcut(8),
	focusPanel9: positionalPanelShortcut(9),
	focusPanel10: positionalPanelShortcut(10),
	focusActivePanelNext: {
		defaults: {
			apple: chord('BracketRight', ['control', 'shift']),
			standard: chord('BracketRight', ['control', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.focusActivePanelNext,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.focusActivePanelNext,
	},
	focusActivePanelPrevious: {
		defaults: {
			apple: chord('BracketLeft', ['control', 'shift']),
			standard: chord('BracketLeft', ['control', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.focusActivePanelPrevious,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.focusActivePanelPrevious,
	},
	// Last Panel carries the TUI's Ctrl+B ; with the Panel-focus family's Ctrl+Shift on both
	// platforms, as cyclic focus does.
	focusLastPanel: {
		defaults: {
			apple: chord('Semicolon', ['control', 'shift']),
			standard: chord('Semicolon', ['control', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.focusLastPanel,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.focusLastPanel,
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
	focusActivePanelVisible: {
		defaults: {
			apple: chord('F7', ['control', 'shift']),
			standard: chord('F7', ['control', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.focusActivePanelVisible,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.focusActivePanelVisible,
	},
	focusNextRegion: {
		defaults: {
			apple: chord('F6', ['mod']),
			standard: chord('F6', ['mod']),
		},
		id: WORKBENCH_ACTION_IDS.focusNextRegion,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.focusNextRegion,
	},
	focusPreviousRegion: {
		defaults: {
			apple: chord('F6', ['mod', 'shift']),
			standard: chord('F6', ['mod', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.focusPreviousRegion,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.focusPreviousRegion,
	},
	increaseTerminalFontSize: {
		defaults: {
			apple: chord('Equal', ['mod']),
			standard: chord('Equal', ['mod']),
		},
		id: WORKBENCH_ACTION_IDS.increaseTerminalFontSize,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.increaseTerminalFontSize,
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
	moveActivePanelDown: {
		defaults: {
			apple: chord('ArrowDown', ['mod', 'alt', 'shift']),
			standard: chord('ArrowDown', ['mod', 'alt', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.moveActivePanelDown,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.moveActivePanelDown,
	},
	moveActivePanelLeft: {
		defaults: {
			apple: chord('ArrowLeft', ['mod', 'alt', 'shift']),
			standard: chord('ArrowLeft', ['mod', 'alt', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.moveActivePanelLeft,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.moveActivePanelLeft,
	},
	moveActivePanelRight: {
		defaults: {
			apple: chord('ArrowRight', ['mod', 'alt', 'shift']),
			standard: chord('ArrowRight', ['mod', 'alt', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.moveActivePanelRight,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.moveActivePanelRight,
	},
	moveActivePanelUp: {
		defaults: {
			apple: chord('ArrowUp', ['mod', 'alt', 'shift']),
			standard: chord('ArrowUp', ['mod', 'alt', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.moveActivePanelUp,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.moveActivePanelUp,
	},
	moveActiveTabBackward: {
		defaults: {
			apple: chord('Comma', ['control', 'shift']),
			standard: chord('Comma', ['control', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.moveActiveTabBackward,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.moveActiveTabBackward,
	},
	moveActiveTabForward: {
		defaults: {
			apple: chord('Period', ['control', 'shift']),
			standard: chord('Period', ['control', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.moveActiveTabForward,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.moveActiveTabForward,
	},
	// New Workspace and the shortcut reference carry the TUI's Ctrl+B Shift+N and Ctrl+B ? with the
	// platform modifier, as the sidebar canvas proposes; Apple takes Cmd+/ because Cmd+? opens the
	// system's Help search.
	newWorkspace: {
		defaults: {
			apple: chord('KeyN', ['mod', 'shift']),
			standard: chord('KeyN', ['mod', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.createWorkspace,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.createWorkspace,
	},
	setTabTitle: {
		defaults: {
			apple: chord('KeyI', ['mod', 'shift']),
			standard: chord('KeyT', ['mod', 'alt', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.setTabTitle,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.setTabTitle,
	},
	// Start Agent takes G with Rename Tab's chord shape: Apple Cmd+Shift+G, and the standard
	// platforms add Alt because Ctrl+Shift+G shows a terminal's last output.
	startAgent: {
		defaults: {
			apple: chord('KeyG', ['mod', 'shift']),
			standard: chord('KeyG', ['mod', 'alt', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.openAgentStart,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.openAgentStart,
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
	newTerminalInCurrentDirectory: {
		defaults: {
			apple: chord('Enter', ['alt', 'mod']),
			standard: chord('Enter', ['alt', 'mod']),
		},
		id: WORKBENCH_ACTION_IDS.createTerminalInCurrentDirectory,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.createTerminalInCurrentDirectory,
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
	resetTerminalFontSize: {
		defaults: {
			apple: chord('Digit0', ['mod']),
			standard: chord('Digit0', ['mod']),
		},
		id: WORKBENCH_ACTION_IDS.resetTerminalFontSize,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.resetTerminalFontSize,
	},
	resizeActivePanel: {
		defaults: {
			apple: chord('KeyR', ['mod']),
			standard: chord('KeyR', ['mod', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.resizeActivePanel,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.resizeActivePanel,
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
	shortcuts: {
		defaults: {
			apple: chord('Slash', ['mod']),
			standard: chord('Slash', ['mod', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.openShortcuts,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.openShortcuts,
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
	swapActivePanelVisible: {
		defaults: {
			apple: chord('F8', ['control', 'shift']),
			standard: chord('F8', ['control', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.swapActivePanelVisible,
		input: undefined,
		shortcutId: WORKBENCH_ACTION_IDS.swapActivePanelVisible,
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

type WorkbenchShortcutAction = keyof typeof WORKBENCH_SHORTCUTS;
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
	'decreaseTerminalFontSize',
	'equalizePanels',
	'focusActivePanelDown',
	'focusActivePanelLeft',
	'focusPanel1',
	'focusPanel2',
	'focusPanel3',
	'focusPanel4',
	'focusPanel5',
	'focusPanel6',
	'focusPanel7',
	'focusPanel8',
	'focusPanel9',
	'focusPanel10',
	'focusActivePanelNext',
	'focusActivePanelPrevious',
	'focusLastPanel',
	'focusActivePanelRight',
	'focusActivePanelUp',
	'focusActivePanelVisible',
	'focusNextRegion',
	'focusPreviousRegion',
	'increaseTerminalFontSize',
	'maximizeActivePanel',
	'moveActivePanelDown',
	'moveActivePanelLeft',
	'moveActivePanelRight',
	'moveActivePanelUp',
	'moveActiveTabBackward',
	'moveActiveTabForward',
	'setTabTitle',
	'newTerminal',
	'newTerminalInCurrentDirectory',
	'newWorkspace',
	'search',
	'resetTerminalFontSize',
	'resizeActivePanel',
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
	'shortcuts',
	'splitActivePanelBelow',
	'splitActivePanelRight',
	'startAgent',
	'swapActivePanelVisible',
	'toggleSidebar',
] as const satisfies readonly WorkbenchShortcutAction[]);

function positionalPanelShortcut<const Position extends WorkbenchPanelPositionInput['position']>(
	position: Position,
) {
	const code = position === 10 ? 'Digit0' : `Digit${position}`;
	return {
		defaults: {
			apple: chord(code, ['control', 'shift']),
			standard: chord(code, ['control', 'shift']),
		},
		id: WORKBENCH_ACTION_IDS.focusActivePanelByPosition,
		input: { position },
		shortcutId: `workbench.panel.focus.${position}`,
	} as const;
}

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
		case WORKBENCH_ACTION_IDS.createTerminalInCurrentDirectory:
			return formatWorkbenchShortcut('newTerminalInCurrentDirectory', platform, bindings);
		case WORKBENCH_ACTION_IDS.closeActiveTerminal:
			return formatWorkbenchShortcut('closeActiveTerminal', platform, bindings);
		case WORKBENCH_ACTION_IDS.equalizeActivePanels:
			return formatWorkbenchShortcut('equalizePanels', platform, bindings);
		case WORKBENCH_ACTION_IDS.focusActivePanelDown:
			return formatWorkbenchShortcut('focusActivePanelDown', platform, bindings);
		case WORKBENCH_ACTION_IDS.focusActivePanelLeft:
			return formatWorkbenchShortcut('focusActivePanelLeft', platform, bindings);
		case WORKBENCH_ACTION_IDS.focusActivePanelNext:
			return formatWorkbenchShortcut('focusActivePanelNext', platform, bindings);
		case WORKBENCH_ACTION_IDS.focusActivePanelPrevious:
			return formatWorkbenchShortcut('focusActivePanelPrevious', platform, bindings);
		case WORKBENCH_ACTION_IDS.focusLastPanel:
			return formatWorkbenchShortcut('focusLastPanel', platform, bindings);
		case WORKBENCH_ACTION_IDS.focusActivePanelRight:
			return formatWorkbenchShortcut('focusActivePanelRight', platform, bindings);
		case WORKBENCH_ACTION_IDS.focusActivePanelUp:
			return formatWorkbenchShortcut('focusActivePanelUp', platform, bindings);
		case WORKBENCH_ACTION_IDS.focusActivePanelVisible:
			return formatWorkbenchShortcut('focusActivePanelVisible', platform, bindings);
		case WORKBENCH_ACTION_IDS.focusNextRegion:
			return formatWorkbenchShortcut('focusNextRegion', platform, bindings);
		case WORKBENCH_ACTION_IDS.focusPreviousRegion:
			return formatWorkbenchShortcut('focusPreviousRegion', platform, bindings);
		case WORKBENCH_ACTION_IDS.increaseTerminalFontSize:
			return formatWorkbenchShortcut('increaseTerminalFontSize', platform, bindings);
		case WORKBENCH_ACTION_IDS.moveActivePanelDown:
			return formatWorkbenchShortcut('moveActivePanelDown', platform, bindings);
		case WORKBENCH_ACTION_IDS.moveActivePanelLeft:
			return formatWorkbenchShortcut('moveActivePanelLeft', platform, bindings);
		case WORKBENCH_ACTION_IDS.moveActivePanelRight:
			return formatWorkbenchShortcut('moveActivePanelRight', platform, bindings);
		case WORKBENCH_ACTION_IDS.moveActivePanelUp:
			return formatWorkbenchShortcut('moveActivePanelUp', platform, bindings);
		case WORKBENCH_ACTION_IDS.moveActiveTabBackward:
			return formatWorkbenchShortcut('moveActiveTabBackward', platform, bindings);
		case WORKBENCH_ACTION_IDS.moveActiveTabForward:
			return formatWorkbenchShortcut('moveActiveTabForward', platform, bindings);
		case WORKBENCH_ACTION_IDS.setTabTitle:
			return formatWorkbenchShortcut('setTabTitle', platform, bindings);
		case WORKBENCH_ACTION_IDS.decreaseTerminalFontSize:
			return formatWorkbenchShortcut('decreaseTerminalFontSize', platform, bindings);
		case WORKBENCH_ACTION_IDS.resetTerminalFontSize:
			return formatWorkbenchShortcut('resetTerminalFontSize', platform, bindings);
		case WORKBENCH_ACTION_IDS.resizeActivePanel:
			return formatWorkbenchShortcut('resizeActivePanel', platform, bindings);
		case WORKBENCH_ACTION_IDS.openSearch:
			return formatWorkbenchShortcut('search', platform, bindings);
		case WORKBENCH_ACTION_IDS.openSettings:
			return formatWorkbenchShortcut('settings', platform, bindings);
		case WORKBENCH_ACTION_IDS.openShortcuts:
			return formatWorkbenchShortcut('shortcuts', platform, bindings);
		case WORKBENCH_ACTION_IDS.openAgentStart:
			return formatWorkbenchShortcut('startAgent', platform, bindings);
		case WORKBENCH_ACTION_IDS.createWorkspace:
			return formatWorkbenchShortcut('newWorkspace', platform, bindings);
		case WORKBENCH_ACTION_IDS.selectNextTab:
			return formatWorkbenchShortcut('selectNextTab', platform, bindings);
		case WORKBENCH_ACTION_IDS.selectPreviousTab:
			return formatWorkbenchShortcut('selectPreviousTab', platform, bindings);
		case WORKBENCH_ACTION_IDS.splitActivePanelBelow:
			return formatWorkbenchShortcut('splitActivePanelBelow', platform, bindings);
		case WORKBENCH_ACTION_IDS.splitActivePanelRight:
			return formatWorkbenchShortcut('splitActivePanelRight', platform, bindings);
		case WORKBENCH_ACTION_IDS.swapActivePanelVisible:
			return formatWorkbenchShortcut('swapActivePanelVisible', platform, bindings);
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
		[WORKBENCH_SHORTCUTS.decreaseTerminalFontSize.shortcutId]:
			WORKBENCH_SHORTCUTS.decreaseTerminalFontSize.defaults[platform],
		[WORKBENCH_SHORTCUTS.equalizePanels.shortcutId]:
			WORKBENCH_SHORTCUTS.equalizePanels.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusActivePanelDown.shortcutId]:
			WORKBENCH_SHORTCUTS.focusActivePanelDown.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusActivePanelLeft.shortcutId]:
			WORKBENCH_SHORTCUTS.focusActivePanelLeft.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusPanel1.shortcutId]:
			WORKBENCH_SHORTCUTS.focusPanel1.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusPanel2.shortcutId]:
			WORKBENCH_SHORTCUTS.focusPanel2.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusPanel3.shortcutId]:
			WORKBENCH_SHORTCUTS.focusPanel3.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusPanel4.shortcutId]:
			WORKBENCH_SHORTCUTS.focusPanel4.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusPanel5.shortcutId]:
			WORKBENCH_SHORTCUTS.focusPanel5.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusPanel6.shortcutId]:
			WORKBENCH_SHORTCUTS.focusPanel6.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusPanel7.shortcutId]:
			WORKBENCH_SHORTCUTS.focusPanel7.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusPanel8.shortcutId]:
			WORKBENCH_SHORTCUTS.focusPanel8.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusPanel9.shortcutId]:
			WORKBENCH_SHORTCUTS.focusPanel9.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusPanel10.shortcutId]:
			WORKBENCH_SHORTCUTS.focusPanel10.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusActivePanelNext.shortcutId]:
			WORKBENCH_SHORTCUTS.focusActivePanelNext.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusActivePanelPrevious.shortcutId]:
			WORKBENCH_SHORTCUTS.focusActivePanelPrevious.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusLastPanel.shortcutId]:
			WORKBENCH_SHORTCUTS.focusLastPanel.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusActivePanelRight.shortcutId]:
			WORKBENCH_SHORTCUTS.focusActivePanelRight.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusActivePanelUp.shortcutId]:
			WORKBENCH_SHORTCUTS.focusActivePanelUp.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusActivePanelVisible.shortcutId]:
			WORKBENCH_SHORTCUTS.focusActivePanelVisible.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusNextRegion.shortcutId]:
			WORKBENCH_SHORTCUTS.focusNextRegion.defaults[platform],
		[WORKBENCH_SHORTCUTS.focusPreviousRegion.shortcutId]:
			WORKBENCH_SHORTCUTS.focusPreviousRegion.defaults[platform],
		[WORKBENCH_SHORTCUTS.increaseTerminalFontSize.shortcutId]:
			WORKBENCH_SHORTCUTS.increaseTerminalFontSize.defaults[platform],
		[WORKBENCH_SHORTCUTS.maximizeActivePanel.shortcutId]:
			WORKBENCH_SHORTCUTS.maximizeActivePanel.defaults[platform],
		[WORKBENCH_SHORTCUTS.moveActivePanelDown.shortcutId]:
			WORKBENCH_SHORTCUTS.moveActivePanelDown.defaults[platform],
		[WORKBENCH_SHORTCUTS.moveActivePanelLeft.shortcutId]:
			WORKBENCH_SHORTCUTS.moveActivePanelLeft.defaults[platform],
		[WORKBENCH_SHORTCUTS.moveActivePanelRight.shortcutId]:
			WORKBENCH_SHORTCUTS.moveActivePanelRight.defaults[platform],
		[WORKBENCH_SHORTCUTS.moveActivePanelUp.shortcutId]:
			WORKBENCH_SHORTCUTS.moveActivePanelUp.defaults[platform],
		[WORKBENCH_SHORTCUTS.moveActiveTabBackward.shortcutId]:
			WORKBENCH_SHORTCUTS.moveActiveTabBackward.defaults[platform],
		[WORKBENCH_SHORTCUTS.moveActiveTabForward.shortcutId]:
			WORKBENCH_SHORTCUTS.moveActiveTabForward.defaults[platform],
		[WORKBENCH_SHORTCUTS.setTabTitle.shortcutId]:
			WORKBENCH_SHORTCUTS.setTabTitle.defaults[platform],
		[WORKBENCH_SHORTCUTS.startAgent.shortcutId]: WORKBENCH_SHORTCUTS.startAgent.defaults[platform],
		[WORKBENCH_SHORTCUTS.newTerminal.shortcutId]:
			WORKBENCH_SHORTCUTS.newTerminal.defaults[platform],
		[WORKBENCH_SHORTCUTS.newTerminalInCurrentDirectory.shortcutId]:
			WORKBENCH_SHORTCUTS.newTerminalInCurrentDirectory.defaults[platform],
		[WORKBENCH_SHORTCUTS.newWorkspace.shortcutId]:
			WORKBENCH_SHORTCUTS.newWorkspace.defaults[platform],
		[WORKBENCH_SHORTCUTS.search.shortcutId]: WORKBENCH_SHORTCUTS.search.defaults[platform],
		[WORKBENCH_SHORTCUTS.resetTerminalFontSize.shortcutId]:
			WORKBENCH_SHORTCUTS.resetTerminalFontSize.defaults[platform],
		[WORKBENCH_SHORTCUTS.resizeActivePanel.shortcutId]:
			WORKBENCH_SHORTCUTS.resizeActivePanel.defaults[platform],
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
		[WORKBENCH_SHORTCUTS.shortcuts.shortcutId]: WORKBENCH_SHORTCUTS.shortcuts.defaults[platform],
		[WORKBENCH_SHORTCUTS.splitActivePanelBelow.shortcutId]:
			WORKBENCH_SHORTCUTS.splitActivePanelBelow.defaults[platform],
		[WORKBENCH_SHORTCUTS.splitActivePanelRight.shortcutId]:
			WORKBENCH_SHORTCUTS.splitActivePanelRight.defaults[platform],
		[WORKBENCH_SHORTCUTS.swapActivePanelVisible.shortcutId]:
			WORKBENCH_SHORTCUTS.swapActivePanelVisible.defaults[platform],
		[WORKBENCH_SHORTCUTS.toggleSidebar.shortcutId]:
			WORKBENCH_SHORTCUTS.toggleSidebar.defaults[platform],
	});
}
