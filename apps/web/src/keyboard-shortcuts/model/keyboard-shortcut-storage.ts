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

export type KeyboardShortcutPersistenceIssue = 'invalid-record' | 'read-failed' | 'write-failed';

export interface KeyboardShortcutStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

export interface KeyboardShortcutStorageReadResult {
	readonly issue: Exclude<KeyboardShortcutPersistenceIssue, 'write-failed'> | null;
	readonly migration: 'v1' | 'v2' | 'v3' | 'v4' | null;
	readonly overrides: readonly KeyboardShortcutOverride[];
}

const chordSchema = z.strictObject({
	code: z.string(),
	modifiers: z
		.array(z.enum(['alt', 'control', 'meta', 'mod', 'shift']))
		.min(1)
		.max(5),
});
const storedKeyboardShortcutOverridesV1Schema = z.strictObject({
	overrides: z
		.array(
			z.strictObject({
				chord: chordSchema,
				shortcutId: z.string(),
			}),
		)
		.max(LEGACY_V1_KEYBOARD_SHORTCUT_IDS.length),
	version: z.literal(1),
});
const storedKeyboardShortcutOverridesV2Schema = z.strictObject({
	overrides: z
		.array(
			z.strictObject({
				chord: chordSchema,
				shortcutId: z.string(),
			}),
		)
		.max(LEGACY_V2_KEYBOARD_SHORTCUT_IDS.length),
	version: z.literal(2),
});
const storedKeyboardShortcutOverridesV3Schema = z.strictObject({
	overrides: z
		.array(
			z.strictObject({
				chord: chordSchema,
				shortcutId: z.string(),
			}),
		)
		.max(LEGACY_V3_KEYBOARD_SHORTCUT_IDS.length),
	version: z.literal(3),
});
const storedKeyboardShortcutOverridesV4Schema = z.strictObject({
	overrides: z
		.array(
			z.strictObject({
				chord: chordSchema,
				shortcutId: z.string(),
			}),
		)
		.max(LEGACY_V4_KEYBOARD_SHORTCUT_IDS.length),
	version: z.literal(4),
});
const storedKeyboardShortcutOverridesV5Schema = z.strictObject({
	overrides: z
		.array(
			z.strictObject({
				chord: chordSchema,
				shortcutId: z.string(),
			}),
		)
		.max(KEYBOARD_SHORTCUT_IDS.length),
	version: z.literal(5),
});

interface StoredKeyboardShortcutOverride {
	readonly chord: { readonly code: string; readonly modifiers: readonly string[] };
	readonly shortcutId: string;
}

export function readKeyboardShortcutOverrides(
	storage: KeyboardShortcutStorage,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutStorageReadResult {
	let versionFive: string | null;
	try {
		versionFive = storage.getItem(KEYBOARD_SHORTCUT_STORAGE_V5_KEY);
	} catch {
		return defaultReadResult('read-failed');
	}
	if (versionFive !== null) return readVersionFive(versionFive, platform);

	let versionFour: string | null;
	try {
		versionFour = storage.getItem(KEYBOARD_SHORTCUT_STORAGE_V4_KEY);
	} catch {
		return defaultReadResult('read-failed');
	}
	if (versionFour !== null) return migrateVersionFour(versionFour, platform);

	let versionThree: string | null;
	try {
		versionThree = storage.getItem(KEYBOARD_SHORTCUT_STORAGE_V3_KEY);
	} catch {
		return defaultReadResult('read-failed');
	}
	if (versionThree !== null) return migrateVersionThree(versionThree, platform);

	let versionTwo: string | null;
	try {
		versionTwo = storage.getItem(KEYBOARD_SHORTCUT_STORAGE_V2_KEY);
	} catch {
		return defaultReadResult('read-failed');
	}
	if (versionTwo !== null) return migrateVersionTwo(versionTwo, platform);

	let versionOne: string | null;
	try {
		versionOne = storage.getItem(KEYBOARD_SHORTCUT_STORAGE_V1_KEY);
	} catch {
		return defaultReadResult('read-failed');
	}
	if (versionOne === null) return defaultReadResult(null);
	return readVersionOne(versionOne, platform);
}

export function writeKeyboardShortcutOverrides(
	storage: KeyboardShortcutStorage,
	overrides: readonly KeyboardShortcutOverride[],
): boolean {
	try {
		storage.setItem(
			KEYBOARD_SHORTCUT_STORAGE_V5_KEY,
			JSON.stringify({
				overrides: overrides.map((override) => ({
					chord: override.chord,
					shortcutId: override.shortcutId,
				})),
				version: 5,
			}),
		);
		return true;
	} catch {
		return false;
	}
}

function readVersionFive(
	serialized: string,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutStorageReadResult {
	try {
		const parsed = storedKeyboardShortcutOverridesV5Schema.safeParse(JSON.parse(serialized));
		if (!parsed.success) return defaultReadResult('invalid-record');
		const overrides = validateCurrentOverrides(parsed.data.overrides, platform);
		return overrides === null
			? defaultReadResult('invalid-record')
			: { issue: null, migration: null, overrides };
	} catch {
		return defaultReadResult('invalid-record');
	}
}

function migrateVersionFour(
	serialized: string,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutStorageReadResult {
	try {
		const parsed = storedKeyboardShortcutOverridesV4Schema.safeParse(JSON.parse(serialized));
		if (!parsed.success) return defaultReadResult('invalid-record');
		const legacyOverrides = validateLegacyOverrides(
			parsed.data.overrides,
			platform,
			LEGACY_V4_KEYBOARD_SHORTCUT_IDS,
			LEGACY_V4_DEFAULT_BINDINGS,
		);
		return legacyOverrides === null
			? defaultReadResult('invalid-record')
			: migrateLegacyOverrides(legacyOverrides, platform, 'v4');
	} catch {
		return defaultReadResult('invalid-record');
	}
}

function migrateVersionThree(
	serialized: string,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutStorageReadResult {
	try {
		const parsed = storedKeyboardShortcutOverridesV3Schema.safeParse(JSON.parse(serialized));
		if (!parsed.success) return defaultReadResult('invalid-record');
		const legacyOverrides = validateLegacyOverrides(
			parsed.data.overrides,
			platform,
			LEGACY_V3_KEYBOARD_SHORTCUT_IDS,
			LEGACY_V3_DEFAULT_BINDINGS,
		);
		return legacyOverrides === null
			? defaultReadResult('invalid-record')
			: migrateLegacyOverrides(legacyOverrides, platform, 'v3');
	} catch {
		return defaultReadResult('invalid-record');
	}
}

function migrateVersionTwo(
	serialized: string,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutStorageReadResult {
	try {
		const parsed = storedKeyboardShortcutOverridesV2Schema.safeParse(JSON.parse(serialized));
		if (!parsed.success) return defaultReadResult('invalid-record');
		const legacyOverrides = validateLegacyOverrides(
			parsed.data.overrides,
			platform,
			LEGACY_V2_KEYBOARD_SHORTCUT_IDS,
			LEGACY_V2_DEFAULT_BINDINGS,
		);
		return legacyOverrides === null
			? defaultReadResult('invalid-record')
			: migrateLegacyOverrides(legacyOverrides, platform, 'v2');
	} catch {
		return defaultReadResult('invalid-record');
	}
}

function readVersionOne(
	serialized: string,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutStorageReadResult {
	try {
		const parsed = storedKeyboardShortcutOverridesV1Schema.safeParse(JSON.parse(serialized));
		if (!parsed.success) return defaultReadResult('invalid-record');
		const legacyOverrides = validateLegacyOverrides(
			parsed.data.overrides,
			platform,
			LEGACY_V1_KEYBOARD_SHORTCUT_IDS,
			LEGACY_V1_DEFAULT_BINDINGS,
		);
		return legacyOverrides === null
			? defaultReadResult('invalid-record')
			: migrateLegacyOverrides(legacyOverrides, platform, 'v1');
	} catch {
		return defaultReadResult('invalid-record');
	}
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
