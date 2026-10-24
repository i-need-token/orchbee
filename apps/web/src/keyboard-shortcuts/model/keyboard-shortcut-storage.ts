import {
	keyboardShortcutChordKey,
	normalizeKeyboardShortcutChord,
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

const KEYBOARD_SHORTCUT_STORAGE_KEY = 'orchbee:web:keyboard-shortcut-overrides:v1';

export type KeyboardShortcutPersistenceIssue = 'invalid-record' | 'read-failed' | 'write-failed';

export interface KeyboardShortcutStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

export interface KeyboardShortcutStorageReadResult {
	readonly issue: Exclude<KeyboardShortcutPersistenceIssue, 'write-failed'> | null;
	readonly overrides: readonly KeyboardShortcutOverride[];
}

const chordSchema = z.strictObject({
	code: z.string(),
	modifiers: z
		.array(z.enum(['alt', 'control', 'meta', 'mod', 'shift']))
		.min(1)
		.max(5),
});
const storedKeyboardShortcutOverridesSchema = z.strictObject({
	overrides: z
		.array(z.strictObject({ chord: chordSchema, shortcutId: z.string() }))
		.max(KEYBOARD_SHORTCUT_IDS.length),
	version: z.literal(1),
});

interface StoredKeyboardShortcutOverride {
	readonly chord: { readonly code: string; readonly modifiers: readonly string[] };
	readonly shortcutId: string;
}

export function readKeyboardShortcutOverrides(
	storage: KeyboardShortcutStorage,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutStorageReadResult {
	let serialized: string | null;
	try {
		serialized = storage.getItem(KEYBOARD_SHORTCUT_STORAGE_KEY);
	} catch {
		return defaultReadResult('read-failed');
	}
	return serialized === null ? defaultReadResult(null) : readStoredOverrides(serialized, platform);
}

export function writeKeyboardShortcutOverrides(
	storage: KeyboardShortcutStorage,
	overrides: readonly KeyboardShortcutOverride[],
): boolean {
	try {
		storage.setItem(
			KEYBOARD_SHORTCUT_STORAGE_KEY,
			JSON.stringify({
				overrides: overrides.map((override) => ({
					chord: override.chord,
					shortcutId: override.shortcutId,
				})),
				version: 1,
			}),
		);
		return true;
	} catch {
		return false;
	}
}

function readStoredOverrides(
	serialized: string,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutStorageReadResult {
	try {
		const parsed = storedKeyboardShortcutOverridesSchema.safeParse(JSON.parse(serialized));
		if (!parsed.success) return defaultReadResult('invalid-record');
		const overrides = validateCurrentOverrides(parsed.data.overrides, platform);
		return overrides === null ? defaultReadResult('invalid-record') : { issue: null, overrides };
	} catch {
		return defaultReadResult('invalid-record');
	}
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
	return { issue, overrides: Object.freeze([]) };
}
