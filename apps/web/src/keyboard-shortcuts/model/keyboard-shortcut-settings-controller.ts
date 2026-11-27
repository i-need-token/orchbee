import {
	keyboardShortcutChordKey,
	keyboardShortcutSequenceKey,
	normalizeKeyboardShortcutChord,
	type KeyboardShortcutPlatform,
	type KeyboardShortcutSequence,
} from '@orchbee/keyboard-shortcuts';

import type { ProjectedTerminalShortcutBindingSource } from '../../projected-terminal-keyboard-shortcuts';
import type { WorkbenchScopedShortcutBinding, WorkbenchScopedShortcutId } from '../../workbench';
import {
	defaultKeyboardShortcutBindings,
	findKeyboardShortcutConflict,
	offeredKeyboardShortcutDeclarations,
	type KeyboardShortcutBindings,
	type KeyboardShortcutDeclaration,
	type KeyboardShortcutId,
} from './keyboard-shortcut-catalog';
import {
	createKeyboardShortcutBindings,
	keyboardShortcutOverridesEqual,
	replaceKeyboardShortcutOverride,
	type KeyboardShortcutOverride,
} from './keyboard-shortcut-overrides';
import {
	createKeyboardShortcutScopedBindings,
	findKeyboardShortcutScopedConflict,
	keyboardShortcutScopedDeclaration,
	keyboardShortcutScopedDeclarations,
	keyboardShortcutScopedOverridesEqual,
	normalizeKeyboardShortcutScopedSequence,
	replaceKeyboardShortcutScopedOverride,
	type KeyboardShortcutAnyId,
	type KeyboardShortcutScopedDeclaration,
	type KeyboardShortcutScopedOverride,
} from './keyboard-shortcut-scoped-catalog';
import {
	readKeyboardShortcutOverrides,
	readKeyboardShortcutSteps,
	writeKeyboardShortcutOverrides,
	writeKeyboardShortcutSteps,
	type KeyboardShortcutPersistenceIssue,
	type KeyboardShortcutStorage,
} from './keyboard-shortcut-storage';
import { KeyboardShortcutWriteQueue } from './KeyboardShortcutWriteQueue';

export interface KeyboardShortcutSettingsSnapshot {
	readonly bindings: KeyboardShortcutBindings;
	readonly overrides: readonly KeyboardShortcutOverride[];
	readonly persistenceIssue: KeyboardShortcutPersistenceIssue | null;
	/** The File Bar family's current keys; none where the File Bar is not offered. */
	readonly scopedBindings: readonly WorkbenchScopedShortcutBinding[];
	readonly scopedOverrides: readonly KeyboardShortcutScopedOverride[];
}

interface KeyboardShortcutConflict {
	readonly conflictingShortcutId: KeyboardShortcutAnyId;
	readonly sequence: KeyboardShortcutSequence;
	readonly shortcutId: KeyboardShortcutAnyId;
}

type KeyboardShortcutUpdateResult =
	| { readonly kind: 'applied-and-saved' }
	| { readonly kind: 'applied-session-only' }
	| { readonly conflict: KeyboardShortcutConflict; readonly kind: 'conflict' }
	| { readonly kind: 'disposed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'unchanged' };

interface KeyboardShortcutSettingsControllerOptions {
	/**
	 * Whether the window offers the File Bar. Only then does it list the File Bar family's
	 * shortcuts and keep the v2 record beside v1; a release reads and writes v1 alone.
	 */
	readonly fileBarOffered?: boolean | undefined;
	readonly platform: KeyboardShortcutPlatform;
	/** Whether the window offers Search; a release withholds it. Defaults to offered. */
	readonly searchOffered?: boolean | undefined;
	readonly storage: KeyboardShortcutStorage;
}

type Listener = () => void;

const NO_SCOPED_DECLARATIONS: readonly KeyboardShortcutScopedDeclaration[] = Object.freeze([]);
const NO_SCOPED_BINDINGS: readonly WorkbenchScopedShortcutBinding[] = Object.freeze([]);
const NO_SCOPED_OVERRIDES: readonly KeyboardShortcutScopedOverride[] = Object.freeze([]);

export class KeyboardShortcutSettingsController implements ProjectedTerminalShortcutBindingSource {
	/** The shortcuts this window lists and offers, in catalog order. */
	readonly declarations: readonly KeyboardShortcutDeclaration[];
	/** The File Bar family's shortcuts this window lists; none without the File Bar. */
	readonly scopedDeclarations: readonly KeyboardShortcutScopedDeclaration[];
	readonly #fileBarOffered: boolean;
	readonly #listeners = new Set<Listener>();
	readonly #platform: KeyboardShortcutPlatform;
	readonly #persistence = new KeyboardShortcutWriteQueue();
	readonly #storage: KeyboardShortcutStorage;
	#disposed = false;
	#revision = 0;
	#snapshot: KeyboardShortcutSettingsSnapshot;

	constructor({
		fileBarOffered = false,
		platform,
		searchOffered = true,
		storage,
	}: KeyboardShortcutSettingsControllerOptions) {
		this.declarations = offeredKeyboardShortcutDeclarations(searchOffered);
		this.scopedDeclarations = fileBarOffered
			? keyboardShortcutScopedDeclarations(platform)
			: NO_SCOPED_DECLARATIONS;
		this.#fileBarOffered = fileBarOffered;
		this.#platform = platform;
		this.#storage = storage;
		const restored = fileBarOffered
			? readKeyboardShortcutSteps(storage, platform)
			: {
					...readKeyboardShortcutOverrides(storage, platform),
					scopedOverrides: NO_SCOPED_OVERRIDES,
				};
		this.#snapshot = createSnapshot(
			createKeyboardShortcutBindings(platform, restored.overrides),
			restored.overrides,
			this.#scopedBindings(restored.scopedOverrides),
			restored.scopedOverrides,
			restored.issue,
		);
	}

	current = (): KeyboardShortcutBindings => this.#snapshot.bindings;

	getSnapshot = (): KeyboardShortcutSettingsSnapshot => this.#snapshot;

	subscribe = (listener: Listener): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	setShortcut(shortcutId: KeyboardShortcutId, input: unknown): KeyboardShortcutUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		const chord = normalizeKeyboardShortcutChord(input, this.#platform);
		if (chord === null) return { kind: 'invalid' };
		const defaultChord = defaultKeyboardShortcutBindings(this.#platform)[shortcutId];
		const isDefault =
			keyboardShortcutChordKey(chord, this.#platform) ===
			keyboardShortcutChordKey(defaultChord, this.#platform);
		const overrides = replaceKeyboardShortcutOverride(
			this.#snapshot.overrides,
			shortcutId,
			isDefault ? null : chord,
		);
		return this.#apply(shortcutId, [chord], overrides, this.#snapshot.scopedOverrides);
	}

	resetShortcut(shortcutId: KeyboardShortcutId): KeyboardShortcutUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		const overrides = replaceKeyboardShortcutOverride(this.#snapshot.overrides, shortcutId, null);
		return this.#apply(
			shortcutId,
			[defaultKeyboardShortcutBindings(this.#platform)[shortcutId]],
			overrides,
			this.#snapshot.scopedOverrides,
		);
	}

	/** Sets a File Bar family shortcut's keys: one chord under its scope's rule, or a sequence. */
	setScopedShortcut(
		shortcutId: WorkbenchScopedShortcutId,
		input: unknown,
	): KeyboardShortcutUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		const declaration = this.#scopedDeclaration(shortcutId);
		if (declaration === undefined) return { kind: 'invalid' };
		const sequence = normalizeKeyboardShortcutScopedSequence(declaration, input, this.#platform);
		if (sequence === null) return { kind: 'invalid' };
		const isDefault =
			keyboardShortcutSequenceKey(sequence, this.#platform) ===
			keyboardShortcutSequenceKey(declaration.defaultSequence, this.#platform);
		const scopedOverrides = replaceKeyboardShortcutScopedOverride(
			this.#snapshot.scopedOverrides,
			shortcutId,
			isDefault ? null : sequence,
		);
		return this.#apply(shortcutId, sequence, this.#snapshot.overrides, scopedOverrides);
	}

	resetScopedShortcut(shortcutId: WorkbenchScopedShortcutId): KeyboardShortcutUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		const declaration = this.#scopedDeclaration(shortcutId);
		if (declaration === undefined) return { kind: 'invalid' };
		const scopedOverrides = replaceKeyboardShortcutScopedOverride(
			this.#snapshot.scopedOverrides,
			shortcutId,
			null,
		);
		return this.#apply(
			shortcutId,
			declaration.defaultSequence,
			this.#snapshot.overrides,
			scopedOverrides,
		);
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#revision += 1;
		this.#persistence.dispose();
		this.#listeners.clear();
	}

	#scopedDeclaration(shortcutId: string): KeyboardShortcutScopedDeclaration | undefined {
		return this.#fileBarOffered
			? keyboardShortcutScopedDeclaration(shortcutId, this.#platform)
			: undefined;
	}

	#scopedBindings(
		scopedOverrides: readonly KeyboardShortcutScopedOverride[],
	): readonly WorkbenchScopedShortcutBinding[] {
		return this.#fileBarOffered
			? createKeyboardShortcutScopedBindings(this.#platform, scopedOverrides)
			: NO_SCOPED_BINDINGS;
	}

	#apply(
		shortcutId: KeyboardShortcutAnyId,
		sequence: KeyboardShortcutSequence,
		overrides: readonly KeyboardShortcutOverride[],
		scopedOverrides: readonly KeyboardShortcutScopedOverride[],
	): KeyboardShortcutUpdateResult {
		const bindings = createKeyboardShortcutBindings(this.#platform, overrides);
		const conflictPair =
			findKeyboardShortcutConflict(bindings, this.#platform) ??
			(this.#fileBarOffered
				? findKeyboardShortcutScopedConflict(bindings, scopedOverrides, this.#platform)
				: null);
		if (conflictPair !== null) {
			const conflictingShortcutId =
				conflictPair.firstShortcutId === shortcutId
					? conflictPair.secondShortcutId
					: conflictPair.firstShortcutId;
			return {
				conflict: { conflictingShortcutId, sequence, shortcutId },
				kind: 'conflict',
			};
		}
		const changed =
			!keyboardShortcutOverridesEqual(this.#snapshot.overrides, overrides, this.#platform) ||
			!keyboardShortcutScopedOverridesEqual(
				this.#snapshot.scopedOverrides,
				scopedOverrides,
				this.#platform,
			);
		if (!changed && this.#snapshot.persistenceIssue === null) return { kind: 'unchanged' };

		const revision = ++this.#revision;
		const scopedBindings = this.#scopedBindings(scopedOverrides);
		this.#snapshot = createSnapshot(
			bindings,
			overrides,
			scopedBindings,
			scopedOverrides,
			this.#snapshot.persistenceIssue,
		);
		this.#persistence.write(() => {
			const saved = this.#fileBarOffered
				? writeKeyboardShortcutSteps(this.#storage, overrides, scopedOverrides)
				: writeKeyboardShortcutOverrides(this.#storage, overrides);
			if (this.#disposed || this.#revision !== revision) return;
			this.#snapshot = createSnapshot(
				bindings,
				overrides,
				scopedBindings,
				scopedOverrides,
				saved ? null : 'write-failed',
			);
			this.#emit();
		});
		if (this.#disposed) return { kind: 'disposed' };
		return this.#snapshot.persistenceIssue === null && !this.#persistence.pending
			? { kind: 'applied-and-saved' }
			: { kind: 'applied-session-only' };
	}

	#emit(): void {
		const listeners = new Set(this.#listeners);
		for (const listener of listeners) {
			if (this.#disposed) break;
			try {
				listener();
			} catch {
				// A consumer cannot prevent the remaining stable subscribers from observing the update.
			}
		}
	}
}

export function keyboardShortcutBindingSource(
	snapshot: KeyboardShortcutSettingsSnapshot,
	shortcutId: KeyboardShortcutAnyId,
): 'custom' | 'default' {
	return snapshot.overrides.some((override) => override.shortcutId === shortcutId) ||
		snapshot.scopedOverrides.some((override) => override.shortcutId === shortcutId)
		? 'custom'
		: 'default';
}

function createSnapshot(
	bindings: KeyboardShortcutBindings,
	overrides: readonly KeyboardShortcutOverride[],
	scopedBindings: readonly WorkbenchScopedShortcutBinding[],
	scopedOverrides: readonly KeyboardShortcutScopedOverride[],
	persistenceIssue: KeyboardShortcutPersistenceIssue | null,
): KeyboardShortcutSettingsSnapshot {
	return Object.freeze({ bindings, overrides, persistenceIssue, scopedBindings, scopedOverrides });
}
