import {
	keyboardShortcutChordKey,
	normalizeKeyboardShortcutChord,
	type KeyboardShortcutChord,
	type KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';
import { z } from 'zod';

import {
	KEYBOARD_SHORTCUT_IDS,
	defaultKeyboardShortcutBindings,
	findKeyboardShortcutConflict,
	isKeyboardShortcutId,
} from './keyboard-shortcut-catalog';
import {
	createKeyboardShortcutBindings,
	type KeyboardShortcutOverride,
} from './keyboard-shortcut-overrides';

const KEYBOARD_SHORTCUT_STORAGE_V1_KEY = 'orchbee:web:keyboard-shortcuts:v1';
const KEYBOARD_SHORTCUT_STORAGE_V2_KEY = 'orchbee:web:keyboard-shortcuts:v2';
const KEYBOARD_SHORTCUT_STORAGE_V3_KEY = 'orchbee:web:keyboard-shortcuts:v3';
const KEYBOARD_SHORTCUT_STORAGE_V4_KEY = 'orchbee:web:keyboard-shortcuts:v4';
const KEYBOARD_SHORTCUT_STORAGE_V5_KEY = 'orchbee:web:keyboard-shortcuts:v5';
const KEYBOARD_SHORTCUT_STORAGE_V6_KEY = 'orchbee:web:keyboard-shortcuts:v6';
const KEYBOARD_SHORTCUT_STORAGE_V7_KEY = 'orchbee:web:keyboard-shortcuts:v7';
const KEYBOARD_SHORTCUT_STORAGE_V8_KEY = 'orchbee:web:keyboard-shortcuts:v8';
const KEYBOARD_SHORTCUT_STORAGE_V9_KEY = 'orchbee:web:keyboard-shortcuts:v9';
const KEYBOARD_SHORTCUT_STORAGE_V10_KEY = 'orchbee:web:keyboard-shortcuts:v10';
const KEYBOARD_SHORTCUT_STORAGE_V11_KEY = 'orchbee:web:keyboard-shortcuts:v11';
const KEYBOARD_SHORTCUT_STORAGE_V12_KEY = 'orchbee:web:keyboard-shortcuts:v12';
const KEYBOARD_SHORTCUT_STORAGE_V13_KEY = 'orchbee:web:keyboard-shortcuts:v13';
const KEYBOARD_SHORTCUT_STORAGE_V14_KEY = 'orchbee:web:keyboard-shortcuts:v14';
const KEYBOARD_SHORTCUT_STORAGE_V15_KEY = 'orchbee:web:keyboard-shortcuts:v15';
const KEYBOARD_SHORTCUT_STORAGE_V16_KEY = 'orchbee:web:keyboard-shortcuts:v16';
const KEYBOARD_SHORTCUT_STORAGE_V17_KEY = 'orchbee:web:keyboard-shortcuts:v17';
const KEYBOARD_SHORTCUT_STORAGE_V18_KEY = 'orchbee:web:keyboard-shortcuts:v18';
const KEYBOARD_SHORTCUT_STORAGE_V19_KEY = 'orchbee:web:keyboard-shortcuts:v19';
const KEYBOARD_SHORTCUT_STORAGE_V20_KEY = 'orchbee:web:keyboard-shortcuts:v20';
const KEYBOARD_SHORTCUT_STORAGE_V21_KEY = 'orchbee:web:keyboard-shortcuts:v21';

const CURRENT_DIRECTORY_TERMINAL_SHORTCUT_ID = 'workbench.terminal.create-current-directory';
const PRE_WORKBENCH_REGION_FOCUS_SHORTCUT_IDS = Object.freeze(
	KEYBOARD_SHORTCUT_IDS.filter(
		(shortcutId) =>
			shortcutId !== 'workbench.focus.next-region' &&
			shortcutId !== 'workbench.focus.previous-region',
	),
);
const PRE_POSITIONAL_PANEL_FOCUS_SHORTCUT_IDS = Object.freeze(
	PRE_WORKBENCH_REGION_FOCUS_SHORTCUT_IDS.filter(
		(shortcutId) => !shortcutId.startsWith('workbench.panel.focus.'),
	),
);
const PRE_VISUAL_PANEL_SELECTION_SHORTCUT_IDS = Object.freeze(
	PRE_POSITIONAL_PANEL_FOCUS_SHORTCUT_IDS.filter(
		(shortcutId) =>
			shortcutId !== 'workbench.panel.active.focus-visible' &&
			shortcutId !== 'workbench.panel.active.swap-visible',
	),
);
const PRE_CYCLIC_PANEL_FOCUS_SHORTCUT_IDS = Object.freeze(
	PRE_VISUAL_PANEL_SELECTION_SHORTCUT_IDS.filter(
		(shortcutId) =>
			shortcutId !== 'workbench.panel.active.focus-next' &&
			shortcutId !== 'workbench.panel.active.focus-previous',
	),
);

const LEGACY_V1_KEYBOARD_SHORTCUT_IDS = Object.freeze([
	'terminal.search.open',
	'terminal.selection.copy',
	'terminal.selection.select-all',
	'workbench.search.actions.open',
	'workbench.search.all.open',
	'workbench.settings.open',
	'workbench.sidebar.toggle',
	'workbench.terminal.create',
] as const);

type LegacyV1KeyboardShortcutId = (typeof LEGACY_V1_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V2_KEYBOARD_SHORTCUT_IDS = Object.freeze([
	'terminal.clipboard.paste',
	'terminal.search.open',
	'terminal.selection.copy',
	'terminal.selection.select-all',
	'workbench.panel.active.maximization.toggle',
	'workbench.panel.active.split-below',
	'workbench.panel.active.split-right',
	'workbench.search.actions.open',
	'workbench.search.all.open',
	'workbench.settings.open',
	'workbench.sidebar.toggle',
	'workbench.terminal.active.close',
	'workbench.terminal.create',
] as const satisfies readonly (typeof KEYBOARD_SHORTCUT_IDS)[number][]);

type LegacyV2KeyboardShortcutId = (typeof LEGACY_V2_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V3_KEYBOARD_SHORTCUT_IDS = Object.freeze([
	'terminal.clear',
	'terminal.clipboard.paste',
	'terminal.search.open',
	'terminal.selection.copy',
	'terminal.selection.select-all',
	'workbench.panel.active.focus-down',
	'workbench.panel.active.focus-left',
	'workbench.panel.active.focus-right',
	'workbench.panel.active.focus-up',
	'workbench.panel.active.maximization.toggle',
	'workbench.panel.active.split-below',
	'workbench.panel.active.split-right',
	'workbench.search.actions.open',
	'workbench.search.all.open',
	'workbench.settings.open',
	'workbench.sidebar.toggle',
	'workbench.terminal.active.close',
	'workbench.terminal.create',
] as const satisfies readonly (typeof KEYBOARD_SHORTCUT_IDS)[number][]);

type LegacyV3KeyboardShortcutId = (typeof LEGACY_V3_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V4_KEYBOARD_SHORTCUT_IDS = Object.freeze([
	'terminal.clear',
	'terminal.clipboard.paste',
	'terminal.search.open',
	'terminal.selection.copy',
	'terminal.selection.select-all',
	'workbench.panel.active.focus-down',
	'workbench.panel.active.focus-left',
	'workbench.panel.active.focus-right',
	'workbench.panel.active.focus-up',
	'workbench.panel.active.maximization.toggle',
	'workbench.panel.active.split-below',
	'workbench.panel.active.split-right',
	'workbench.search.actions.open',
	'workbench.search.all.open',
	'workbench.settings.open',
	'workbench.sidebar.toggle',
	'workbench.tab.select.1',
	'workbench.tab.select.2',
	'workbench.tab.select.3',
	'workbench.tab.select.4',
	'workbench.tab.select.5',
	'workbench.tab.select.6',
	'workbench.tab.select.7',
	'workbench.tab.select.8',
	'workbench.tab.select.9',
	'workbench.terminal.active.close',
	'workbench.terminal.create',
] as const satisfies readonly (typeof KEYBOARD_SHORTCUT_IDS)[number][]);

type LegacyV4KeyboardShortcutId = (typeof LEGACY_V4_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V5_KEYBOARD_SHORTCUT_IDS = Object.freeze([
	...LEGACY_V4_KEYBOARD_SHORTCUT_IDS,
	'workbench.tab.select-next',
	'workbench.tab.select-previous',
] as const satisfies readonly (typeof KEYBOARD_SHORTCUT_IDS)[number][]);

type LegacyV5KeyboardShortcutId = (typeof LEGACY_V5_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V6_KEYBOARD_SHORTCUT_IDS = Object.freeze([
	...LEGACY_V5_KEYBOARD_SHORTCUT_IDS,
	'terminal.history.prompt-next',
	'terminal.history.prompt-previous',
] as const satisfies readonly (typeof KEYBOARD_SHORTCUT_IDS)[number][]);

type LegacyV6KeyboardShortcutId = (typeof LEGACY_V6_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V7_KEYBOARD_SHORTCUT_IDS = Object.freeze([
	...LEGACY_V6_KEYBOARD_SHORTCUT_IDS,
	'terminal.history.scroll-bottom',
	'terminal.history.scroll-line-down',
	'terminal.history.scroll-line-up',
	'terminal.history.scroll-page-down',
	'terminal.history.scroll-page-up',
	'terminal.history.scroll-top',
] as const satisfies readonly (typeof KEYBOARD_SHORTCUT_IDS)[number][]);

type LegacyV7KeyboardShortcutId = (typeof LEGACY_V7_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V8_KEYBOARD_SHORTCUT_IDS = Object.freeze([
	...LEGACY_V7_KEYBOARD_SHORTCUT_IDS,
	'workbench.terminal.font-size.decrease',
	'workbench.terminal.font-size.increase',
	'workbench.terminal.font-size.reset',
] as const satisfies readonly (typeof KEYBOARD_SHORTCUT_IDS)[number][]);

type LegacyV8KeyboardShortcutId = (typeof LEGACY_V8_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V9_KEYBOARD_SHORTCUT_IDS = Object.freeze(
	PRE_CYCLIC_PANEL_FOCUS_SHORTCUT_IDS.filter(
		(shortcutId) =>
			shortcutId !== 'terminal.output.show-last' &&
			shortcutId !== 'terminal.history.show' &&
			shortcutId !== 'workbench.panel.active.equalize' &&
			shortcutId !== 'workbench.panel.active.move-down' &&
			shortcutId !== 'workbench.panel.active.move-left' &&
			shortcutId !== 'workbench.panel.active.move-right' &&
			shortcutId !== 'workbench.panel.active.move-up' &&
			shortcutId !== 'workbench.tab.active.move-backward' &&
			shortcutId !== 'workbench.tab.active.move-forward' &&
			shortcutId !== 'workbench.tab.title.set' &&
			shortcutId !== CURRENT_DIRECTORY_TERMINAL_SHORTCUT_ID,
	),
);

type LegacyV9KeyboardShortcutId = (typeof LEGACY_V9_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V10_KEYBOARD_SHORTCUT_IDS = Object.freeze(
	PRE_CYCLIC_PANEL_FOCUS_SHORTCUT_IDS.filter(
		(shortcutId) =>
			shortcutId !== 'terminal.history.show' &&
			shortcutId !== 'workbench.panel.active.equalize' &&
			shortcutId !== 'workbench.panel.active.move-down' &&
			shortcutId !== 'workbench.panel.active.move-left' &&
			shortcutId !== 'workbench.panel.active.move-right' &&
			shortcutId !== 'workbench.panel.active.move-up' &&
			shortcutId !== 'workbench.tab.active.move-backward' &&
			shortcutId !== 'workbench.tab.active.move-forward' &&
			shortcutId !== 'workbench.tab.title.set' &&
			shortcutId !== CURRENT_DIRECTORY_TERMINAL_SHORTCUT_ID,
	),
);

type LegacyV10KeyboardShortcutId = (typeof LEGACY_V10_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V11_KEYBOARD_SHORTCUT_IDS = Object.freeze(
	PRE_CYCLIC_PANEL_FOCUS_SHORTCUT_IDS.filter(
		(shortcutId) =>
			shortcutId !== 'workbench.panel.active.equalize' &&
			shortcutId !== 'workbench.panel.active.move-down' &&
			shortcutId !== 'workbench.panel.active.move-left' &&
			shortcutId !== 'workbench.panel.active.move-right' &&
			shortcutId !== 'workbench.panel.active.move-up' &&
			shortcutId !== 'workbench.tab.active.move-backward' &&
			shortcutId !== 'workbench.tab.active.move-forward' &&
			shortcutId !== 'workbench.tab.title.set' &&
			shortcutId !== CURRENT_DIRECTORY_TERMINAL_SHORTCUT_ID,
	),
);

type LegacyV11KeyboardShortcutId = (typeof LEGACY_V11_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V12_KEYBOARD_SHORTCUT_IDS = Object.freeze(
	PRE_CYCLIC_PANEL_FOCUS_SHORTCUT_IDS.filter(
		(shortcutId) =>
			shortcutId !== 'workbench.panel.active.equalize' &&
			shortcutId !== 'workbench.panel.active.move-down' &&
			shortcutId !== 'workbench.panel.active.move-left' &&
			shortcutId !== 'workbench.panel.active.move-right' &&
			shortcutId !== 'workbench.panel.active.move-up' &&
			shortcutId !== 'workbench.tab.title.set' &&
			shortcutId !== CURRENT_DIRECTORY_TERMINAL_SHORTCUT_ID,
	),
);

type LegacyV12KeyboardShortcutId = (typeof LEGACY_V12_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V13_KEYBOARD_SHORTCUT_IDS = Object.freeze(
	PRE_CYCLIC_PANEL_FOCUS_SHORTCUT_IDS.filter(
		(shortcutId) =>
			shortcutId !== 'workbench.panel.active.equalize' &&
			shortcutId !== 'workbench.panel.active.move-down' &&
			shortcutId !== 'workbench.panel.active.move-left' &&
			shortcutId !== 'workbench.panel.active.move-right' &&
			shortcutId !== 'workbench.panel.active.move-up' &&
			shortcutId !== 'workbench.panel.active.resize' &&
			shortcutId !== CURRENT_DIRECTORY_TERMINAL_SHORTCUT_ID,
	),
);

type LegacyV13KeyboardShortcutId = (typeof LEGACY_V13_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V14_KEYBOARD_SHORTCUT_IDS = Object.freeze(
	PRE_CYCLIC_PANEL_FOCUS_SHORTCUT_IDS.filter(
		(shortcutId) =>
			shortcutId !== 'workbench.panel.active.equalize' &&
			shortcutId !== 'workbench.panel.active.move-down' &&
			shortcutId !== 'workbench.panel.active.move-left' &&
			shortcutId !== 'workbench.panel.active.move-right' &&
			shortcutId !== 'workbench.panel.active.move-up' &&
			shortcutId !== CURRENT_DIRECTORY_TERMINAL_SHORTCUT_ID,
	),
);

type LegacyV14KeyboardShortcutId = (typeof LEGACY_V14_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V15_KEYBOARD_SHORTCUT_IDS = Object.freeze(
	PRE_CYCLIC_PANEL_FOCUS_SHORTCUT_IDS.filter(
		(shortcutId) =>
			shortcutId !== 'workbench.panel.active.equalize' &&
			shortcutId !== CURRENT_DIRECTORY_TERMINAL_SHORTCUT_ID,
	),
);

type LegacyV15KeyboardShortcutId = (typeof LEGACY_V15_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V16_KEYBOARD_SHORTCUT_IDS = Object.freeze(
	PRE_CYCLIC_PANEL_FOCUS_SHORTCUT_IDS.filter(
		(shortcutId) => shortcutId !== CURRENT_DIRECTORY_TERMINAL_SHORTCUT_ID,
	),
);

type LegacyV16KeyboardShortcutId = (typeof LEGACY_V16_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V17_KEYBOARD_SHORTCUT_IDS = PRE_CYCLIC_PANEL_FOCUS_SHORTCUT_IDS;

type LegacyV17KeyboardShortcutId = (typeof LEGACY_V17_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V18_KEYBOARD_SHORTCUT_IDS = PRE_VISUAL_PANEL_SELECTION_SHORTCUT_IDS;

type LegacyV18KeyboardShortcutId = (typeof LEGACY_V18_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V19_KEYBOARD_SHORTCUT_IDS = PRE_POSITIONAL_PANEL_FOCUS_SHORTCUT_IDS;

type LegacyV19KeyboardShortcutId = (typeof LEGACY_V19_KEYBOARD_SHORTCUT_IDS)[number];

const LEGACY_V20_KEYBOARD_SHORTCUT_IDS = PRE_WORKBENCH_REGION_FOCUS_SHORTCUT_IDS;

type LegacyV20KeyboardShortcutId = (typeof LEGACY_V20_KEYBOARD_SHORTCUT_IDS)[number];

const legacyChord = (
	code: string,
	modifiers: KeyboardShortcutChord['modifiers'],
): KeyboardShortcutChord => Object.freeze({ code, modifiers: Object.freeze([...modifiers]) });

const LEGACY_V1_DEFAULT_BINDINGS = Object.freeze({
	apple: Object.freeze({
		'terminal.search.open': legacyChord('KeyF', ['mod']),
		'terminal.selection.copy': legacyChord('KeyC', ['mod']),
		'terminal.selection.select-all': legacyChord('KeyA', ['mod']),
		'workbench.search.actions.open': legacyChord('KeyP', ['mod', 'shift']),
		'workbench.search.all.open': legacyChord('KeyK', ['mod']),
		'workbench.settings.open': legacyChord('Comma', ['mod']),
		'workbench.sidebar.toggle': legacyChord('KeyB', ['mod']),
		'workbench.terminal.create': legacyChord('KeyN', ['mod']),
	}),
	standard: Object.freeze({
		'terminal.search.open': legacyChord('KeyF', ['mod', 'shift']),
		'terminal.selection.copy': legacyChord('KeyC', ['mod', 'shift']),
		'terminal.selection.select-all': legacyChord('KeyA', ['mod', 'shift']),
		'workbench.search.actions.open': legacyChord('KeyP', ['mod', 'shift']),
		'workbench.search.all.open': legacyChord('KeyK', ['mod', 'shift']),
		'workbench.settings.open': legacyChord('Comma', ['mod']),
		'workbench.sidebar.toggle': legacyChord('KeyB', ['mod', 'shift']),
		'workbench.terminal.create': legacyChord('KeyN', ['mod', 'shift']),
	}),
} satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV1KeyboardShortcutId, KeyboardShortcutChord>>
	>
>);

const LEGACY_V2_DEFAULT_BINDINGS = Object.freeze({
	apple: Object.freeze({
		'terminal.clipboard.paste': legacyChord('KeyV', ['mod']),
		'terminal.search.open': legacyChord('KeyF', ['mod']),
		'terminal.selection.copy': legacyChord('KeyC', ['mod']),
		'terminal.selection.select-all': legacyChord('KeyA', ['mod']),
		'workbench.panel.active.maximization.toggle': legacyChord('Enter', ['mod', 'shift']),
		'workbench.panel.active.split-below': legacyChord('KeyD', ['mod', 'shift']),
		'workbench.panel.active.split-right': legacyChord('KeyD', ['mod']),
		'workbench.search.actions.open': legacyChord('KeyP', ['mod', 'shift']),
		'workbench.search.all.open': legacyChord('KeyK', ['mod']),
		'workbench.settings.open': legacyChord('Comma', ['mod']),
		'workbench.sidebar.toggle': legacyChord('KeyB', ['mod']),
		'workbench.terminal.active.close': legacyChord('KeyW', ['mod']),
		'workbench.terminal.create': legacyChord('KeyT', ['mod']),
	}),
	standard: Object.freeze({
		'terminal.clipboard.paste': legacyChord('KeyV', ['mod', 'shift']),
		'terminal.search.open': legacyChord('KeyF', ['mod', 'shift']),
		'terminal.selection.copy': legacyChord('KeyC', ['mod', 'shift']),
		'terminal.selection.select-all': legacyChord('KeyA', ['mod', 'shift']),
		'workbench.panel.active.maximization.toggle': legacyChord('Enter', ['mod', 'shift']),
		'workbench.panel.active.split-below': legacyChord('KeyE', ['mod', 'shift']),
		'workbench.panel.active.split-right': legacyChord('KeyD', ['mod', 'shift']),
		'workbench.search.actions.open': legacyChord('KeyP', ['mod', 'shift']),
		'workbench.search.all.open': legacyChord('KeyK', ['mod', 'shift']),
		'workbench.settings.open': legacyChord('Comma', ['mod']),
		'workbench.sidebar.toggle': legacyChord('KeyB', ['mod', 'shift']),
		'workbench.terminal.active.close': legacyChord('KeyW', ['mod', 'shift']),
		'workbench.terminal.create': legacyChord('KeyT', ['mod', 'shift']),
	}),
} satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV2KeyboardShortcutId, KeyboardShortcutChord>>
	>
>);

const LEGACY_V3_DEFAULT_BINDINGS = Object.freeze({
	apple: Object.freeze({
		'terminal.clear': legacyChord('KeyK', ['mod']),
		'terminal.clipboard.paste': legacyChord('KeyV', ['mod']),
		'terminal.search.open': legacyChord('KeyF', ['mod']),
		'terminal.selection.copy': legacyChord('KeyC', ['mod']),
		'terminal.selection.select-all': legacyChord('KeyA', ['mod']),
		'workbench.panel.active.focus-down': legacyChord('ArrowDown', ['alt', 'mod']),
		'workbench.panel.active.focus-left': legacyChord('ArrowLeft', ['alt', 'mod']),
		'workbench.panel.active.focus-right': legacyChord('ArrowRight', ['alt', 'mod']),
		'workbench.panel.active.focus-up': legacyChord('ArrowUp', ['alt', 'mod']),
		'workbench.panel.active.maximization.toggle': legacyChord('Enter', ['mod', 'shift']),
		'workbench.panel.active.split-below': legacyChord('KeyD', ['mod', 'shift']),
		'workbench.panel.active.split-right': legacyChord('KeyD', ['mod']),
		'workbench.search.actions.open': legacyChord('KeyP', ['mod', 'shift']),
		'workbench.search.all.open': legacyChord('KeyF', ['mod', 'shift']),
		'workbench.settings.open': legacyChord('Comma', ['mod']),
		'workbench.sidebar.toggle': legacyChord('KeyB', ['mod']),
		'workbench.terminal.active.close': legacyChord('KeyW', ['mod']),
		'workbench.terminal.create': legacyChord('KeyT', ['mod']),
	}),
	standard: Object.freeze({
		'terminal.clear': legacyChord('KeyL', ['mod', 'shift']),
		'terminal.clipboard.paste': legacyChord('KeyV', ['mod', 'shift']),
		'terminal.search.open': legacyChord('KeyF', ['mod', 'shift']),
		'terminal.selection.copy': legacyChord('KeyC', ['mod', 'shift']),
		'terminal.selection.select-all': legacyChord('KeyA', ['mod', 'shift']),
		'workbench.panel.active.focus-down': legacyChord('ArrowDown', ['alt', 'mod']),
		'workbench.panel.active.focus-left': legacyChord('ArrowLeft', ['alt', 'mod']),
		'workbench.panel.active.focus-right': legacyChord('ArrowRight', ['alt', 'mod']),
		'workbench.panel.active.focus-up': legacyChord('ArrowUp', ['alt', 'mod']),
		'workbench.panel.active.maximization.toggle': legacyChord('Enter', ['mod', 'shift']),
		'workbench.panel.active.split-below': legacyChord('KeyE', ['mod', 'shift']),
		'workbench.panel.active.split-right': legacyChord('KeyD', ['mod', 'shift']),
		'workbench.search.actions.open': legacyChord('KeyP', ['mod', 'shift']),
		'workbench.search.all.open': legacyChord('KeyK', ['mod', 'shift']),
		'workbench.settings.open': legacyChord('Comma', ['mod']),
		'workbench.sidebar.toggle': legacyChord('KeyB', ['mod', 'shift']),
		'workbench.terminal.active.close': legacyChord('KeyW', ['mod', 'shift']),
		'workbench.terminal.create': legacyChord('KeyT', ['mod', 'shift']),
	}),
} satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV3KeyboardShortcutId, KeyboardShortcutChord>>
	>
>);

const LEGACY_V4_DEFAULT_BINDINGS = Object.freeze({
	apple: Object.freeze({
		...LEGACY_V3_DEFAULT_BINDINGS.apple,
		'workbench.tab.select.1': legacyChord('Digit1', ['mod']),
		'workbench.tab.select.2': legacyChord('Digit2', ['mod']),
		'workbench.tab.select.3': legacyChord('Digit3', ['mod']),
		'workbench.tab.select.4': legacyChord('Digit4', ['mod']),
		'workbench.tab.select.5': legacyChord('Digit5', ['mod']),
		'workbench.tab.select.6': legacyChord('Digit6', ['mod']),
		'workbench.tab.select.7': legacyChord('Digit7', ['mod']),
		'workbench.tab.select.8': legacyChord('Digit8', ['mod']),
		'workbench.tab.select.9': legacyChord('Digit9', ['mod']),
	}),
	standard: Object.freeze({
		...LEGACY_V3_DEFAULT_BINDINGS.standard,
		'workbench.tab.select.1': legacyChord('Digit1', ['mod']),
		'workbench.tab.select.2': legacyChord('Digit2', ['mod']),
		'workbench.tab.select.3': legacyChord('Digit3', ['mod']),
		'workbench.tab.select.4': legacyChord('Digit4', ['mod']),
		'workbench.tab.select.5': legacyChord('Digit5', ['mod']),
		'workbench.tab.select.6': legacyChord('Digit6', ['mod']),
		'workbench.tab.select.7': legacyChord('Digit7', ['mod']),
		'workbench.tab.select.8': legacyChord('Digit8', ['mod']),
		'workbench.tab.select.9': legacyChord('Digit9', ['mod']),
	}),
} satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV4KeyboardShortcutId, KeyboardShortcutChord>>
	>
>);

const LEGACY_V5_DEFAULT_BINDINGS = Object.freeze({
	apple: Object.freeze({
		...LEGACY_V4_DEFAULT_BINDINGS.apple,
		'workbench.tab.select-next': legacyChord('BracketRight', ['mod', 'shift']),
		'workbench.tab.select-previous': legacyChord('BracketLeft', ['mod', 'shift']),
	}),
	standard: Object.freeze({
		...LEGACY_V4_DEFAULT_BINDINGS.standard,
		'workbench.tab.select-next': legacyChord('PageDown', ['mod']),
		'workbench.tab.select-previous': legacyChord('PageUp', ['mod']),
	}),
} satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV5KeyboardShortcutId, KeyboardShortcutChord>>
	>
>);

const LEGACY_V6_DEFAULT_BINDINGS = Object.freeze({
	apple: Object.freeze({
		...LEGACY_V5_DEFAULT_BINDINGS.apple,
		'terminal.history.prompt-next': legacyChord('KeyX', ['control', 'shift']),
		'terminal.history.prompt-previous': legacyChord('KeyZ', ['control', 'shift']),
	}),
	standard: Object.freeze({
		...LEGACY_V5_DEFAULT_BINDINGS.standard,
		'terminal.history.prompt-next': legacyChord('KeyX', ['control', 'shift']),
		'terminal.history.prompt-previous': legacyChord('KeyZ', ['control', 'shift']),
	}),
} satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV6KeyboardShortcutId, KeyboardShortcutChord>>
	>
>);

const LEGACY_V7_DEFAULT_BINDINGS = Object.freeze({
	apple: Object.freeze({
		...LEGACY_V6_DEFAULT_BINDINGS.apple,
		'terminal.history.scroll-bottom': legacyChord('End', ['control', 'shift']),
		'terminal.history.scroll-line-down': legacyChord('ArrowDown', ['control', 'shift']),
		'terminal.history.scroll-line-up': legacyChord('ArrowUp', ['control', 'shift']),
		'terminal.history.scroll-page-down': legacyChord('PageDown', ['control', 'shift']),
		'terminal.history.scroll-page-up': legacyChord('PageUp', ['control', 'shift']),
		'terminal.history.scroll-top': legacyChord('Home', ['control', 'shift']),
	}),
	standard: Object.freeze({
		...LEGACY_V6_DEFAULT_BINDINGS.standard,
		'terminal.history.scroll-bottom': legacyChord('End', ['control', 'shift']),
		'terminal.history.scroll-line-down': legacyChord('ArrowDown', ['control', 'shift']),
		'terminal.history.scroll-line-up': legacyChord('ArrowUp', ['control', 'shift']),
		'terminal.history.scroll-page-down': legacyChord('PageDown', ['control', 'shift']),
		'terminal.history.scroll-page-up': legacyChord('PageUp', ['control', 'shift']),
		'terminal.history.scroll-top': legacyChord('Home', ['control', 'shift']),
	}),
} satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV7KeyboardShortcutId, KeyboardShortcutChord>>
	>
>);

const LEGACY_V8_DEFAULT_BINDINGS = Object.freeze({
	apple: Object.freeze({
		...LEGACY_V7_DEFAULT_BINDINGS.apple,
		'workbench.terminal.font-size.decrease': legacyChord('Minus', ['mod']),
		'workbench.terminal.font-size.increase': legacyChord('Equal', ['mod']),
		'workbench.terminal.font-size.reset': legacyChord('Digit0', ['mod']),
	}),
	standard: Object.freeze({
		...LEGACY_V7_DEFAULT_BINDINGS.standard,
		'workbench.terminal.font-size.decrease': legacyChord('Minus', ['mod']),
		'workbench.terminal.font-size.increase': legacyChord('Equal', ['mod']),
		'workbench.terminal.font-size.reset': legacyChord('Digit0', ['mod']),
	}),
} satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV8KeyboardShortcutId, KeyboardShortcutChord>>
	>
>);

const LEGACY_V9_DEFAULT_BINDINGS = Object.freeze({
	apple: defaultKeyboardShortcutBindings('apple'),
	standard: defaultKeyboardShortcutBindings('standard'),
}) satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV9KeyboardShortcutId, KeyboardShortcutChord>>
	>
>;

const LEGACY_V10_DEFAULT_BINDINGS = Object.freeze({
	apple: defaultKeyboardShortcutBindings('apple'),
	standard: defaultKeyboardShortcutBindings('standard'),
}) satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV10KeyboardShortcutId, KeyboardShortcutChord>>
	>
>;

const LEGACY_V11_DEFAULT_BINDINGS = Object.freeze({
	apple: defaultKeyboardShortcutBindings('apple'),
	standard: defaultKeyboardShortcutBindings('standard'),
}) satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV11KeyboardShortcutId, KeyboardShortcutChord>>
	>
>;

const LEGACY_V12_DEFAULT_BINDINGS = Object.freeze({
	apple: defaultKeyboardShortcutBindings('apple'),
	standard: defaultKeyboardShortcutBindings('standard'),
}) satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV12KeyboardShortcutId, KeyboardShortcutChord>>
	>
>;

const LEGACY_V13_DEFAULT_BINDINGS = Object.freeze({
	apple: defaultKeyboardShortcutBindings('apple'),
	standard: defaultKeyboardShortcutBindings('standard'),
}) satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV13KeyboardShortcutId, KeyboardShortcutChord>>
	>
>;

const LEGACY_V14_DEFAULT_BINDINGS = Object.freeze({
	apple: defaultKeyboardShortcutBindings('apple'),
	standard: defaultKeyboardShortcutBindings('standard'),
}) satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV14KeyboardShortcutId, KeyboardShortcutChord>>
	>
>;

const LEGACY_V15_DEFAULT_BINDINGS = Object.freeze({
	apple: defaultKeyboardShortcutBindings('apple'),
	standard: defaultKeyboardShortcutBindings('standard'),
}) satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV15KeyboardShortcutId, KeyboardShortcutChord>>
	>
>;

const LEGACY_V16_DEFAULT_BINDINGS = Object.freeze({
	apple: defaultKeyboardShortcutBindings('apple'),
	standard: defaultKeyboardShortcutBindings('standard'),
}) satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV16KeyboardShortcutId, KeyboardShortcutChord>>
	>
>;

const LEGACY_V17_DEFAULT_BINDINGS = Object.freeze({
	apple: defaultKeyboardShortcutBindings('apple'),
	standard: defaultKeyboardShortcutBindings('standard'),
}) satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV17KeyboardShortcutId, KeyboardShortcutChord>>
	>
>;

const LEGACY_V18_DEFAULT_BINDINGS = Object.freeze({
	apple: defaultKeyboardShortcutBindings('apple'),
	standard: defaultKeyboardShortcutBindings('standard'),
}) satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV18KeyboardShortcutId, KeyboardShortcutChord>>
	>
>;

const LEGACY_V19_DEFAULT_BINDINGS = Object.freeze({
	apple: defaultKeyboardShortcutBindings('apple'),
	standard: defaultKeyboardShortcutBindings('standard'),
}) satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV19KeyboardShortcutId, KeyboardShortcutChord>>
	>
>;

const LEGACY_V20_DEFAULT_BINDINGS = Object.freeze({
	apple: defaultKeyboardShortcutBindings('apple'),
	standard: defaultKeyboardShortcutBindings('standard'),
}) satisfies Readonly<
	Record<
		KeyboardShortcutPlatform,
		Readonly<Record<LegacyV20KeyboardShortcutId, KeyboardShortcutChord>>
	>
>;

export type KeyboardShortcutPersistenceIssue = 'invalid-record' | 'read-failed' | 'write-failed';

export interface KeyboardShortcutStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

type LegacyKeyboardShortcutStorageVersion =
	| 1
	| 2
	| 3
	| 4
	| 5
	| 6
	| 7
	| 8
	| 9
	| 10
	| 11
	| 12
	| 13
	| 14
	| 15
	| 16
	| 17
	| 18
	| 19
	| 20;

export interface KeyboardShortcutStorageReadResult {
	readonly issue: Exclude<KeyboardShortcutPersistenceIssue, 'write-failed'> | null;
	readonly migration: `v${LegacyKeyboardShortcutStorageVersion}` | null;
	readonly overrides: readonly KeyboardShortcutOverride[];
}

const chordSchema = z.strictObject({
	code: z.string(),
	modifiers: z
		.array(z.enum(['alt', 'control', 'meta', 'mod', 'shift']))
		.min(1)
		.max(5),
});
const storedKeyboardShortcutOverridesV21Schema = createStoredOverridesSchema(
	21,
	KEYBOARD_SHORTCUT_IDS.length,
);

const KEYBOARD_SHORTCUT_STORAGE_READERS: readonly KeyboardShortcutStorageReader[] = Object.freeze([
	{ key: KEYBOARD_SHORTCUT_STORAGE_V21_KEY, read: readVersionTwentyOne },
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V20_KEY,
		20,
		LEGACY_V20_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V20_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V19_KEY,
		19,
		LEGACY_V19_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V19_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V18_KEY,
		18,
		LEGACY_V18_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V18_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V17_KEY,
		17,
		LEGACY_V17_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V17_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V16_KEY,
		16,
		LEGACY_V16_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V16_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V15_KEY,
		15,
		LEGACY_V15_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V15_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V14_KEY,
		14,
		LEGACY_V14_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V14_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V13_KEY,
		13,
		LEGACY_V13_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V13_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V12_KEY,
		12,
		LEGACY_V12_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V12_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V11_KEY,
		11,
		LEGACY_V11_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V11_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V10_KEY,
		10,
		LEGACY_V10_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V10_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V9_KEY,
		9,
		LEGACY_V9_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V9_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V8_KEY,
		8,
		LEGACY_V8_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V8_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V7_KEY,
		7,
		LEGACY_V7_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V7_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V6_KEY,
		6,
		LEGACY_V6_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V6_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V5_KEY,
		5,
		LEGACY_V5_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V5_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V4_KEY,
		4,
		LEGACY_V4_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V4_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V3_KEY,
		3,
		LEGACY_V3_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V3_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V2_KEY,
		2,
		LEGACY_V2_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V2_DEFAULT_BINDINGS,
	),
	createLegacyStorageReader(
		KEYBOARD_SHORTCUT_STORAGE_V1_KEY,
		1,
		LEGACY_V1_KEYBOARD_SHORTCUT_IDS,
		LEGACY_V1_DEFAULT_BINDINGS,
	),
]);

interface StoredKeyboardShortcutOverride {
	readonly chord: { readonly code: string; readonly modifiers: readonly string[] };
	readonly shortcutId: string;
}

export function readKeyboardShortcutOverrides(
	storage: KeyboardShortcutStorage,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutStorageReadResult {
	for (const reader of KEYBOARD_SHORTCUT_STORAGE_READERS) {
		let serialized: string | null;
		try {
			serialized = storage.getItem(reader.key);
		} catch {
			return defaultReadResult('read-failed');
		}
		// The first present version owns the outcome, including invalid data.
		if (serialized !== null) return reader.read(serialized, platform);
	}
	return defaultReadResult(null);
}

export function writeKeyboardShortcutOverrides(
	storage: KeyboardShortcutStorage,
	overrides: readonly KeyboardShortcutOverride[],
): boolean {
	try {
		storage.setItem(
			KEYBOARD_SHORTCUT_STORAGE_V21_KEY,
			JSON.stringify({
				overrides: overrides.map((override) => ({
					chord: override.chord,
					shortcutId: override.shortcutId,
				})),
				version: 21,
			}),
		);
		return true;
	} catch {
		return false;
	}
}

function readVersionTwentyOne(
	serialized: string,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutStorageReadResult {
	try {
		const parsed = storedKeyboardShortcutOverridesV21Schema.safeParse(JSON.parse(serialized));
		if (!parsed.success) return defaultReadResult('invalid-record');
		const overrides = validateCurrentOverrides(parsed.data.overrides, platform);
		return overrides === null
			? defaultReadResult('invalid-record')
			: { issue: null, migration: null, overrides };
	} catch {
		return defaultReadResult('invalid-record');
	}
}

interface StoredKeyboardShortcutRecord {
	readonly overrides: readonly StoredKeyboardShortcutOverride[];
	readonly version: number;
}

interface KeyboardShortcutStorageReader {
	readonly key: string;
	read(serialized: string, platform: KeyboardShortcutPlatform): KeyboardShortcutStorageReadResult;
}

function createStoredOverridesSchema(
	version: number,
	maximumOverrides: number,
): z.ZodType<StoredKeyboardShortcutRecord> {
	return z.strictObject({
		overrides: z
			.array(z.strictObject({ chord: chordSchema, shortcutId: z.string() }))
			.max(maximumOverrides),
		version: z.literal(version),
	});
}

function createLegacyStorageReader<ShortcutId extends KeyboardShortcutOverride['shortcutId']>(
	key: string,
	version: LegacyKeyboardShortcutStorageVersion,
	shortcutIds: readonly ShortcutId[],
	defaultBindings: Readonly<
		Record<KeyboardShortcutPlatform, Readonly<Record<ShortcutId, KeyboardShortcutChord>>>
	>,
): KeyboardShortcutStorageReader {
	const storedOverridesSchema = createStoredOverridesSchema(version, shortcutIds.length);
	return Object.freeze({
		key,
		read(
			serialized: string,
			platform: KeyboardShortcutPlatform,
		): KeyboardShortcutStorageReadResult {
			try {
				const parsed = storedOverridesSchema.safeParse(JSON.parse(serialized));
				if (!parsed.success) return defaultReadResult('invalid-record');
				const legacyOverrides = validateLegacyOverrides(
					parsed.data.overrides,
					platform,
					shortcutIds,
					defaultBindings,
				);
				return legacyOverrides === null
					? defaultReadResult('invalid-record')
					: migrateLegacyOverrides(legacyOverrides, platform, `v${version}`);
			} catch {
				return defaultReadResult('invalid-record');
			}
		},
	});
}

function migrateLegacyOverrides(
	legacyOverrides: readonly KeyboardShortcutOverride[],
	platform: KeyboardShortcutPlatform,
	migration: Exclude<KeyboardShortcutStorageReadResult['migration'], null>,
): KeyboardShortcutStorageReadResult {
	const currentDefaults = defaultKeyboardShortcutBindings(platform);
	const overrides = Object.freeze(
		legacyOverrides.filter(
			(override) =>
				keyboardShortcutChordKey(override.chord, platform) !==
				keyboardShortcutChordKey(currentDefaults[override.shortcutId], platform),
		),
	);
	if (
		findKeyboardShortcutConflict(createKeyboardShortcutBindings(platform, overrides), platform) !==
		null
	) {
		return defaultReadResult('invalid-record');
	}
	return { issue: null, migration, overrides };
}

function validateCurrentOverrides(
	storedOverrides: readonly StoredKeyboardShortcutOverride[],
	platform: KeyboardShortcutPlatform,
): readonly KeyboardShortcutOverride[] | null {
	const overrides: KeyboardShortcutOverride[] = [];
	let previousShortcutId: string | null = null;
	const defaults = defaultKeyboardShortcutBindings(platform);
	for (const stored of storedOverrides) {
		if (
			!isKeyboardShortcutId(stored.shortcutId) ||
			(previousShortcutId !== null && previousShortcutId >= stored.shortcutId)
		) {
			return null;
		}
		const normalized = normalizeKeyboardShortcutChord(stored.chord, platform);
		if (normalized === null || !chordIsCanonical(stored.chord, normalized)) return null;
		if (
			keyboardShortcutChordKey(normalized, platform) ===
			keyboardShortcutChordKey(defaults[stored.shortcutId], platform)
		) {
			return null;
		}
		overrides.push(Object.freeze({ chord: normalized, shortcutId: stored.shortcutId }));
		previousShortcutId = stored.shortcutId;
	}
	const frozenOverrides = Object.freeze(overrides);
	return findKeyboardShortcutConflict(
		createKeyboardShortcutBindings(platform, frozenOverrides),
		platform,
	) === null
		? frozenOverrides
		: null;
}

function validateLegacyOverrides<LegacyShortcutId extends KeyboardShortcutOverride['shortcutId']>(
	storedOverrides: readonly StoredKeyboardShortcutOverride[],
	platform: KeyboardShortcutPlatform,
	shortcutIds: readonly LegacyShortcutId[],
	defaultBindings: Readonly<
		Record<KeyboardShortcutPlatform, Readonly<Record<LegacyShortcutId, KeyboardShortcutChord>>>
	>,
): readonly KeyboardShortcutOverride[] | null {
	const overrides: KeyboardShortcutOverride[] = [];
	const overrideById = new Map<LegacyShortcutId, KeyboardShortcutChord>();
	let previousShortcutId: string | null = null;
	const defaults = defaultBindings[platform];
	for (const stored of storedOverrides) {
		if (
			!isLegacyKeyboardShortcutId(stored.shortcutId, shortcutIds) ||
			(previousShortcutId !== null && previousShortcutId >= stored.shortcutId)
		) {
			return null;
		}
		const normalized = normalizeKeyboardShortcutChord(stored.chord, platform);
		if (normalized === null || !chordIsCanonical(stored.chord, normalized)) return null;
		if (
			keyboardShortcutChordKey(normalized, platform) ===
			keyboardShortcutChordKey(defaults[stored.shortcutId], platform)
		) {
			return null;
		}
		overrides.push(Object.freeze({ chord: normalized, shortcutId: stored.shortcutId }));
		overrideById.set(stored.shortcutId, normalized);
		previousShortcutId = stored.shortcutId;
	}

	const firstIdByChord = new Map<string, LegacyShortcutId>();
	for (const shortcutId of shortcutIds) {
		const key = keyboardShortcutChordKey(
			overrideById.get(shortcutId) ?? defaults[shortcutId],
			platform,
		);
		if (firstIdByChord.has(key)) return null;
		firstIdByChord.set(key, shortcutId);
	}
	return Object.freeze(overrides);
}

function isLegacyKeyboardShortcutId<LegacyShortcutId extends string>(
	value: string,
	shortcutIds: readonly LegacyShortcutId[],
): value is LegacyShortcutId {
	return shortcutIds.some((shortcutId) => shortcutId === value);
}

function chordIsCanonical(
	stored: { readonly code: string; readonly modifiers: readonly string[] },
	normalized: { readonly code: string; readonly modifiers: readonly string[] },
): boolean {
	return (
		stored.code === normalized.code &&
		stored.modifiers.length === normalized.modifiers.length &&
		stored.modifiers.every((modifier, index) => modifier === normalized.modifiers[index])
	);
}

function defaultReadResult(
	issue: KeyboardShortcutStorageReadResult['issue'],
): KeyboardShortcutStorageReadResult {
	return { issue, migration: null, overrides: Object.freeze([]) };
}
