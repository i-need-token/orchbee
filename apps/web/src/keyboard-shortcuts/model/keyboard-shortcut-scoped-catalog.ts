import {
	KEYBOARD_SHORTCUT_SEQUENCE_LEADER,
	keyboardShortcutChordKey,
	keyboardShortcutSequenceKey,
	normalizeKeyboardShortcutSequence,
	type KeyboardShortcutChordRule,
	type KeyboardShortcutPlatform,
	type KeyboardShortcutSequence,
} from '@orchbee/keyboard-shortcuts';
import {
	WORKBENCH_SCOPED_SHORTCUTS,
	widerWorkbenchShortcutScopes,
	workbenchScopeAdmitsSequences,
	workbenchScopedShortcutRule,
	type WorkbenchScopedShortcutBinding,
	type WorkbenchScopedShortcutDefinition,
	type WorkbenchScopedShortcutId,
	type WorkbenchScopedShortcutScope,
} from '../../workbench';

import {
	KEYBOARD_SHORTCUT_CATALOG,
	type KeyboardShortcutBindings,
	type KeyboardShortcutId,
} from './keyboard-shortcut-catalog';

type KeyboardShortcutScopedName = 'toggle-file-bar';

/**
 * A File Bar family shortcut as Settings lists it on one platform. It exists only where the File
 * Bar is offered, and only on a platform its definition gives a default.
 */
export interface KeyboardShortcutScopedDeclaration {
	readonly defaultSequence: KeyboardShortcutSequence;
	readonly definition: WorkbenchScopedShortcutDefinition;
	readonly name: KeyboardShortcutScopedName;
	readonly shortcutId: WorkbenchScopedShortcutId;
}

/** The person's own keys for one scoped shortcut; a default is never stored. */
export interface KeyboardShortcutScopedOverride {
	readonly sequence: KeyboardShortcutSequence;
	readonly shortcutId: WorkbenchScopedShortcutId;
}

/** Any shortcut a conflict or a shadow can name: a window shortcut or a scoped one. */
export type KeyboardShortcutAnyId = KeyboardShortcutId | WorkbenchScopedShortcutId;

const SCOPED_SHORTCUT_NAMES: Readonly<
	Record<WorkbenchScopedShortcutId, KeyboardShortcutScopedName>
> = Object.freeze({
	'workbench.file-bar.toggle': 'toggle-file-bar',
	'workbench.file-bar.toggle.in-file-bar': 'toggle-file-bar',
});

/** How many scoped shortcuts exist on any platform, which bounds a stored record. */
export const KEYBOARD_SHORTCUT_SCOPED_COUNT = Object.keys(WORKBENCH_SCOPED_SHORTCUTS).length;

function platformDeclarations(
	platform: KeyboardShortcutPlatform,
): readonly KeyboardShortcutScopedDeclaration[] {
	return Object.freeze(
		Object.values(WORKBENCH_SCOPED_SHORTCUTS).flatMap((definition) => {
			const defaults: WorkbenchScopedShortcutDefinition['defaults'] = definition.defaults;
			const defaultSequence = defaults[platform];
			return defaultSequence === undefined
				? []
				: [
						Object.freeze({
							defaultSequence,
							definition,
							name: SCOPED_SHORTCUT_NAMES[definition.shortcutId],
							shortcutId: definition.shortcutId,
						}),
					];
		}),
	);
}

const SCOPED_DECLARATIONS: Readonly<
	Record<KeyboardShortcutPlatform, readonly KeyboardShortcutScopedDeclaration[]>
> = Object.freeze({
	apple: platformDeclarations('apple'),
	standard: platformDeclarations('standard'),
});

/** The scoped shortcuts that exist on `platform`, in the Workbench's declaration order. */
export function keyboardShortcutScopedDeclarations(
	platform: KeyboardShortcutPlatform,
): readonly KeyboardShortcutScopedDeclaration[] {
	return SCOPED_DECLARATIONS[platform];
}

export function keyboardShortcutScopedDeclaration(
	shortcutId: string,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutScopedDeclaration | undefined {
	return SCOPED_DECLARATIONS[platform].find((declaration) => declaration.shortcutId === shortcutId);
}

/**
 * Validates keys for a scoped shortcut under its scope's rule. Where a scope admits no sequence, or
 * where a lone `Mod+K` would always start one, the keys could never match and are refused.
 */
export function normalizeKeyboardShortcutScopedSequence(
	declaration: KeyboardShortcutScopedDeclaration,
	input: unknown,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutSequence | null {
	const scope = declaration.definition.scope;
	const sequence = normalizeKeyboardShortcutSequence(
		input,
		platform,
		workbenchScopedShortcutRule(scope),
	);
	if (sequence === null) return null;
	if (!workbenchScopeAdmitsSequences(scope)) return sequence.length === 1 ? sequence : null;
	return sequence.length === 2 ||
		keyboardShortcutChordKey(sequence[0], platform) !==
			keyboardShortcutChordKey(KEYBOARD_SHORTCUT_SEQUENCE_LEADER, platform)
		? sequence
		: null;
}

/**
 * How Settings records a scoped shortcut's keys: the chord rule of its scope, and whether `Mod+K`
 * starts a sequence there.
 */
export function keyboardShortcutScopedRecording(declaration: KeyboardShortcutScopedDeclaration): {
	readonly rule: KeyboardShortcutChordRule;
	readonly sequences: boolean;
} {
	const scope = declaration.definition.scope;
	return {
		rule: workbenchScopedShortcutRule(scope),
		sequences: workbenchScopeAdmitsSequences(scope),
	};
}

export function keyboardShortcutScopedSequence(
	declaration: KeyboardShortcutScopedDeclaration,
	overrides: readonly KeyboardShortcutScopedOverride[],
): KeyboardShortcutSequence {
	return (
		overrides.find((override) => override.shortcutId === declaration.shortcutId)?.sequence ??
		declaration.defaultSequence
	);
}

export function createKeyboardShortcutScopedBindings(
	platform: KeyboardShortcutPlatform,
	overrides: readonly KeyboardShortcutScopedOverride[],
): readonly WorkbenchScopedShortcutBinding[] {
	return Object.freeze(
		keyboardShortcutScopedDeclarations(platform).map((declaration) =>
			Object.freeze({
				definition: declaration.definition,
				sequence: keyboardShortcutScopedSequence(declaration, overrides),
			}),
		),
	);
}

/** Replaces or removes one override, keeping the list ordered by shortcut ID. */
export function replaceKeyboardShortcutScopedOverride(
	overrides: readonly KeyboardShortcutScopedOverride[],
	shortcutId: WorkbenchScopedShortcutId,
	sequence: KeyboardShortcutSequence | null,
): readonly KeyboardShortcutScopedOverride[] {
	const next = overrides.filter((override) => override.shortcutId !== shortcutId);
	if (sequence !== null) next.push(Object.freeze({ sequence, shortcutId }));
	return Object.freeze(next.sort(byShortcutId));
}

export function keyboardShortcutScopedOverridesEqual(
	left: readonly KeyboardShortcutScopedOverride[],
	right: readonly KeyboardShortcutScopedOverride[],
	platform: KeyboardShortcutPlatform,
): boolean {
	return (
		left.length === right.length &&
		left.every((override, index) => {
			const other = right[index];
			return (
				other !== undefined &&
				other.shortcutId === override.shortcutId &&
				keyboardShortcutSequenceKey(other.sequence, platform) ===
					keyboardShortcutSequenceKey(override.sequence, platform)
			);
		})
	);
}

/**
 * Two bindings conflict when one key press reaches both at the same step of the scope walk: the
 * same keys in the same scope, where the window scope holds every window shortcut as well. A
 * narrower scope's binding for a wider one's keys is a shadow, not a conflict.
 */
export function findKeyboardShortcutScopedConflict(
	bindings: KeyboardShortcutBindings,
	overrides: readonly KeyboardShortcutScopedOverride[],
	platform: KeyboardShortcutPlatform,
): {
	readonly firstShortcutId: KeyboardShortcutAnyId;
	readonly secondShortcutId: KeyboardShortcutAnyId;
} | null {
	const firstIdByKey = new Map<string, KeyboardShortcutAnyId>();
	for (const declaration of KEYBOARD_SHORTCUT_CATALOG) {
		const chordKey = keyboardShortcutChordKey(bindings[declaration.shortcutId], platform);
		firstIdByKey.set(scopedKey('shell', chordKey), declaration.shortcutId);
	}
	for (const declaration of keyboardShortcutScopedDeclarations(platform)) {
		const sequence = keyboardShortcutScopedSequence(declaration, overrides);
		const key = scopedKey(
			declaration.definition.scope,
			keyboardShortcutSequenceKey(sequence, platform),
		);
		const firstShortcutId = firstIdByKey.get(key);
		if (firstShortcutId !== undefined) {
			return { firstShortcutId, secondShortcutId: declaration.shortcutId };
		}
		firstIdByKey.set(key, declaration.shortcutId);
	}
	return null;
}

/** The wider binding whose keys `declaration` takes while focus is inside its scope, if any. */
export function findKeyboardShortcutScopedShadow(
	bindings: KeyboardShortcutBindings,
	overrides: readonly KeyboardShortcutScopedOverride[],
	declaration: KeyboardShortcutScopedDeclaration,
	platform: KeyboardShortcutPlatform,
): KeyboardShortcutAnyId | null {
	const key = keyboardShortcutSequenceKey(
		keyboardShortcutScopedSequence(declaration, overrides),
		platform,
	);
	for (const scope of widerWorkbenchShortcutScopes(declaration.definition.scope)) {
		const wider = keyboardShortcutScopedDeclarations(platform).find(
			(candidate) =>
				candidate.definition.scope === scope &&
				keyboardShortcutSequenceKey(
					keyboardShortcutScopedSequence(candidate, overrides),
					platform,
				) === key,
		);
		if (wider !== undefined) return wider.shortcutId;
		if (scope !== 'shell') continue;
		const windowShortcut = KEYBOARD_SHORTCUT_CATALOG.find(
			(candidate) => keyboardShortcutChordKey(bindings[candidate.shortcutId], platform) === key,
		);
		if (windowShortcut !== undefined) return windowShortcut.shortcutId;
	}
	return null;
}

function scopedKey(scope: WorkbenchScopedShortcutScope, keys: string): string {
	return `${scope}\u0000${keys}`;
}

function byShortcutId(
	left: KeyboardShortcutScopedOverride,
	right: KeyboardShortcutScopedOverride,
): number {
	if (left.shortcutId === right.shortcutId) return 0;
	return left.shortcutId < right.shortcutId ? -1 : 1;
}
