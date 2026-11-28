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
	type WorkbenchTabPosition,
} from '../../workbench';

import {
	KEYBOARD_SHORTCUT_CATALOG,
	type KeyboardShortcutBindings,
	type KeyboardShortcutId,
} from './keyboard-shortcut-catalog';

type KeyboardShortcutScopedName =
	| 'close-all-editors'
	| 'close-editor'
	| 'close-other-editors'
	| 'close-saved-editors'
	| 'keep-editor'
	| 'last-editor'
	| 'navigate-next-in-quick-open'
	| 'navigate-previous-in-quick-open'
	| 'next-editor'
	| 'pin-editor'
	| 'previous-editor'
	| 'quick-open-least-recently-used-editor'
	| 'quick-open-previous-recently-used-editor'
	| 'reopen-closed-editor'
	| 'toggle-file-bar'
	| 'toggle-file-overlay-full'
	| 'unpin-editor';

/** The name Settings gives a scoped shortcut, and the tab it shows when it names one by its place. */
type KeyboardShortcutScopedNaming =
	| { readonly name: 'position-editor'; readonly position: WorkbenchTabPosition }
	| { readonly name: KeyboardShortcutScopedName; readonly position?: undefined };

/**
 * A File Bar family shortcut as Settings lists it on one platform. It exists only where the File
 * Bar is offered, and only on a platform its definition gives a default.
 */
export type KeyboardShortcutScopedDeclaration = KeyboardShortcutScopedNaming & {
	readonly defaultSequence: KeyboardShortcutSequence;
	readonly definition: WorkbenchScopedShortcutDefinition;
	readonly shortcutId: WorkbenchScopedShortcutId;
};

/** The person's own keys for one scoped shortcut; a default is never stored. */
export interface KeyboardShortcutScopedOverride {
	readonly sequence: KeyboardShortcutSequence;
	readonly shortcutId: WorkbenchScopedShortcutId;
}

/** Any shortcut a conflict or a shadow can name: a window shortcut or a scoped one. */
export type KeyboardShortcutAnyId = KeyboardShortcutId | WorkbenchScopedShortcutId;

type ScopedShortcutEntry =
	(typeof WORKBENCH_SCOPED_SHORTCUTS)[keyof typeof WORKBENCH_SCOPED_SHORTCUTS];

/** The shortcuts with an input, which name the tab they show by its place. */
type PositionalScopedShortcutEntry = Extract<ScopedShortcutEntry, { readonly input: unknown }>;

const SCOPED_SHORTCUT_NAMES: Readonly<
	Record<
		Exclude<ScopedShortcutEntry, PositionalScopedShortcutEntry>['shortcutId'],
		KeyboardShortcutScopedName
	>
> = Object.freeze({
	'workbench.file-bar.toggle': 'toggle-file-bar',
	'workbench.file-bar.toggle.in-file-bar': 'toggle-file-bar',
	'file-overlay.tier.toggle-full': 'toggle-file-overlay-full',
	'file-overlay.tab.close': 'close-editor',
	'file-overlay.tab.close.function-key': 'close-editor',
	'file-overlay.tab.close-others': 'close-other-editors',
	'file-overlay.tab.close-saved': 'close-saved-editors',
	'file-overlay.tab.close-all': 'close-all-editors',
	'file-overlay.tab.close-all.held-modifier': 'close-all-editors',
	'file-overlay.tab.reopen': 'reopen-closed-editor',
	'file-overlay.tab.select-next': 'next-editor',
	'file-overlay.tab.select-next.bracket': 'next-editor',
	'file-overlay.tab.select-previous': 'previous-editor',
	'file-overlay.tab.select-previous.bracket': 'previous-editor',
	'file-overlay.tab.select-last': 'last-editor',
	'file-overlay.tab.select-last.digit-nine': 'last-editor',
	'file-overlay.tab.keep': 'keep-editor',
	'file-overlay.tab.pin': 'pin-editor',
	'file-overlay.tab.unpin': 'unpin-editor',
	'file-overlay.tab.quick-open-previous': 'quick-open-previous-recently-used-editor',
	'file-overlay.tab.quick-open-least-recent': 'quick-open-least-recently-used-editor',
	'file-overlay.tab-picker.navigate-next': 'navigate-next-in-quick-open',
	'file-overlay.tab-picker.navigate-previous': 'navigate-previous-in-quick-open',
});

function scopedNaming(definition: ScopedShortcutEntry): KeyboardShortcutScopedNaming {
	if ('input' in definition)
		return { name: 'position-editor', position: definition.input.position };
	return { name: SCOPED_SHORTCUT_NAMES[definition.shortcutId] };
}

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
							...scopedNaming(definition),
							defaultSequence,
							definition,
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
 * same keys in the same scope, where the window scope holds every window shortcut as well, unless
 * one names the other as never offered together with it. A narrower scope's binding for a wider
 * one's keys is a shadow, not a conflict.
 */
export function findKeyboardShortcutScopedConflict(
	bindings: KeyboardShortcutBindings,
	overrides: readonly KeyboardShortcutScopedOverride[],
	platform: KeyboardShortcutPlatform,
): {
	readonly firstShortcutId: KeyboardShortcutAnyId;
	readonly secondShortcutId: KeyboardShortcutAnyId;
} | null {
	const holdersByKey = new Map<string, readonly KeyboardShortcutAnyId[]>();
	const hold = (key: string, shortcutId: KeyboardShortcutAnyId): void => {
		holdersByKey.set(key, [...(holdersByKey.get(key) ?? []), shortcutId]);
	};
	for (const declaration of KEYBOARD_SHORTCUT_CATALOG) {
		const chordKey = keyboardShortcutChordKey(bindings[declaration.shortcutId], platform);
		hold(scopedKey('shell', chordKey), declaration.shortcutId);
	}
	for (const declaration of keyboardShortcutScopedDeclarations(platform)) {
		const sequence = keyboardShortcutScopedSequence(declaration, overrides);
		const key = scopedKey(
			declaration.definition.scope,
			keyboardShortcutSequenceKey(sequence, platform),
		);
		const firstShortcutId = holdersByKey
			.get(key)
			?.find((holder) => !scopedShortcutsExclusive(declaration, holder, platform));
		if (firstShortcutId !== undefined) {
			return { firstShortcutId, secondShortcutId: declaration.shortcutId };
		}
		hold(key, declaration.shortcutId);
	}
	return null;
}

/** Whether either of two shortcuts names the other as never offered together with it. */
function scopedShortcutsExclusive(
	declaration: KeyboardShortcutScopedDeclaration,
	otherShortcutId: KeyboardShortcutAnyId,
	platform: KeyboardShortcutPlatform,
): boolean {
	return (
		declaration.definition.exclusiveWith === otherShortcutId ||
		keyboardShortcutScopedDeclaration(otherShortcutId, platform)?.definition.exclusiveWith ===
			declaration.shortcutId
	);
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
