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

const KEYBOARD_SHORTCUT_STORAGE_KEY = 'orchbee:web:keyboard-shortcuts:v1';

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
		.array(
			z.strictObject({
				chord: chordSchema,
				shortcutId: z.string(),
			}),
		)
		.max(KEYBOARD_SHORTCUT_IDS.length),
	version: z.literal(1),
});

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
	if (serialized === null) return defaultReadResult(null);

	try {
		const parsed = storedKeyboardShortcutOverridesSchema.safeParse(JSON.parse(serialized));
		if (!parsed.success) return defaultReadResult('invalid-record');
		const overrides: KeyboardShortcutOverride[] = [];
		let previousShortcutId: string | null = null;
		const defaults = defaultKeyboardShortcutBindings(platform);
		for (const stored of parsed.data.overrides) {
			if (
				!isKeyboardShortcutId(stored.shortcutId) ||
				(previousShortcutId !== null && previousShortcutId >= stored.shortcutId)
			) {
				return defaultReadResult('invalid-record');
			}
			const normalized = normalizeKeyboardShortcutChord(stored.chord, platform);
			if (normalized === null || !chordIsCanonical(stored.chord, normalized)) {
				return defaultReadResult('invalid-record');
			}
			if (
				keyboardShortcutChordKey(normalized, platform) ===
				keyboardShortcutChordKey(defaults[stored.shortcutId], platform)
			) {
				return defaultReadResult('invalid-record');
			}
			overrides.push(Object.freeze({ chord: normalized, shortcutId: stored.shortcutId }));
			previousShortcutId = stored.shortcutId;
		}
		const frozenOverrides = Object.freeze(overrides);
		const bindings = createKeyboardShortcutBindings(platform, frozenOverrides);
		if (findKeyboardShortcutConflict(bindings, platform) !== null) {
			return defaultReadResult('invalid-record');
		}
		return { issue: null, overrides: frozenOverrides };
	} catch {
		return defaultReadResult('invalid-record');
	}
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
