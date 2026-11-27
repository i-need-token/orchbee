import {
	keyboardShortcutChordKey,
	keyboardShortcutSequenceKey,
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
import {
	KEYBOARD_SHORTCUT_SCOPED_COUNT,
	findKeyboardShortcutScopedConflict,
	keyboardShortcutScopedDeclaration,
	normalizeKeyboardShortcutScopedSequence,
	type KeyboardShortcutScopedOverride,
} from './keyboard-shortcut-scoped-catalog';

const KEYBOARD_SHORTCUT_STORAGE_KEY = 'orchbee:web:keyboard-shortcut-overrides:v1';
/**
 * Where the File Bar is offered, every override as steps. Until the release adopts it, the v1
 * record is still written beside it with the window shortcuts, so v1 stays complete for a window
 * that reads only v1, and nothing is migrated in place.
 */
const KEYBOARD_SHORTCUT_STEPS_STORAGE_KEY = 'orchbee:web:keyboard-shortcut-overrides:v2';

export type KeyboardShortcutPersistenceIssue = 'invalid-record' | 'read-failed' | 'write-failed';

export interface KeyboardShortcutStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

export interface KeyboardShortcutStorageReadResult {
	readonly issue: Exclude<KeyboardShortcutPersistenceIssue, 'write-failed'> | null;
	readonly overrides: readonly KeyboardShortcutOverride[];
}

/** What a window that offers the File Bar restores: its window and its scoped overrides. */
export interface KeyboardShortcutStepsReadResult extends KeyboardShortcutStorageReadResult {
	readonly scopedOverrides: readonly KeyboardShortcutScopedOverride[];
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

const stepSchema = z.strictObject({
	code: z.string(),
	modifiers: z.array(z.enum(['alt', 'control', 'meta', 'mod', 'shift'])).max(5),
});
const storedKeyboardShortcutStepsSchema = z.strictObject({
	overrides: z
		.array(z.strictObject({ shortcutId: z.string(), steps: z.array(stepSchema).min(1).max(2) }))
		.max(KEYBOARD_SHORTCUT_IDS.length + KEYBOARD_SHORTCUT_SCOPED_COUNT),
	version: z.literal(2),
});

interface StoredKeyboardShortcutOverride {
	readonly chord: { readonly code: string; readonly modifiers: readonly string[] };
	readonly shortcutId: string;
}

interface StoredKeyboardShortcutSteps {
	readonly shortcutId: string;
	readonly steps: readonly { readonly code: string; readonly modifiers: readonly string[] }[];
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

/**
 * Restores the overrides of a window that offers the File Bar: the v2 record when one exists,
 * otherwise the v1 record read as one-step bindings. Reading never writes, so v1 stays as it was.
 */
export function readKeyboardShortcutSteps(
	storage: KeyboardShortcutStorage,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutStepsReadResult {
	let serialized: string | null;
	try {
		serialized = storage.getItem(KEYBOARD_SHORTCUT_STEPS_STORAGE_KEY);
	} catch {
		return { ...defaultReadResult('read-failed'), scopedOverrides: Object.freeze([]) };
	}
	if (serialized === null) {
		return {
			...readKeyboardShortcutOverrides(storage, platform),
			scopedOverrides: Object.freeze([]),
		};
	}
	return readStoredSteps(serialized, platform);
}

/** Writes the v2 record, then the v1 record of the window shortcuts alone. */
export function writeKeyboardShortcutSteps(
	storage: KeyboardShortcutStorage,
	overrides: readonly KeyboardShortcutOverride[],
	scopedOverrides: readonly KeyboardShortcutScopedOverride[],
): boolean {
	const entries: StoredKeyboardShortcutSteps[] = [
		...overrides.map((override) => ({ shortcutId: override.shortcutId, steps: [override.chord] })),
		...scopedOverrides.map((override) => ({
			shortcutId: override.shortcutId,
			steps: [...override.sequence],
		})),
	].sort((left, right) => (left.shortcutId < right.shortcutId ? -1 : 1));
	try {
		storage.setItem(
			KEYBOARD_SHORTCUT_STEPS_STORAGE_KEY,
			JSON.stringify({ overrides: entries, version: 2 }),
		);
	} catch {
		return false;
	}
	return writeKeyboardShortcutOverrides(storage, overrides);
}

function readStoredSteps(
	serialized: string,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutStepsReadResult {
	const invalid = { ...defaultReadResult('invalid-record'), scopedOverrides: Object.freeze([]) };
	try {
		const parsed = storedKeyboardShortcutStepsSchema.safeParse(JSON.parse(serialized));
		if (!parsed.success) return invalid;
		return validateCurrentSteps(parsed.data.overrides, platform) ?? invalid;
	} catch {
		return invalid;
	}
}

function validateCurrentSteps(
	stored: readonly StoredKeyboardShortcutSteps[],
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutStepsReadResult | null {
	const windowOverrides: StoredKeyboardShortcutOverride[] = [];
	const scopedOverrides: KeyboardShortcutScopedOverride[] = [];
	let previousShortcutId: string | null = null;
	for (const entry of stored) {
		if (previousShortcutId !== null && previousShortcutId >= entry.shortcutId) return null;
		previousShortcutId = entry.shortcutId;
		const [step, secondStep] = entry.steps;
		if (isKeyboardShortcutId(entry.shortcutId)) {
			if (step === undefined || secondStep !== undefined) return null;
			windowOverrides.push({ chord: step, shortcutId: entry.shortcutId });
			continue;
		}
		const declaration = keyboardShortcutScopedDeclaration(entry.shortcutId, platform);
		if (declaration === undefined) return null;
		const sequence = normalizeKeyboardShortcutScopedSequence(declaration, entry.steps, platform);
		if (
			sequence === null ||
			!sequence.every((normalized, index) => {
				const original = entry.steps[index];
				return original !== undefined && chordIsCanonical(original, normalized);
			}) ||
			keyboardShortcutSequenceKey(sequence, platform) ===
				keyboardShortcutSequenceKey(declaration.defaultSequence, platform)
		) {
			return null;
		}
		scopedOverrides.push(Object.freeze({ sequence, shortcutId: declaration.shortcutId }));
	}
	const overrides = validateCurrentOverrides(windowOverrides, platform);
	if (overrides === null) return null;
	const frozenScoped = Object.freeze(scopedOverrides);
	return findKeyboardShortcutScopedConflict(
		createKeyboardShortcutBindings(platform, overrides),
		frozenScoped,
		platform,
	) === null
		? { issue: null, overrides, scopedOverrides: frozenScoped }
		: null;
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
	normalized: KeyboardShortcutChord,
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
