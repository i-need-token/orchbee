import {
	keyboardShortcutChordKey,
	normalizeKeyboardShortcutChord,
	type KeyboardShortcutChord,
	type KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';

import type { ProjectedTerminalShortcutBindingSource } from '../../projected-terminal-keyboard-shortcuts';
import { PreferenceWriteQueue } from '../../PreferenceWriteQueue';
import {
	defaultKeyboardShortcutBindings,
	findKeyboardShortcutConflict,
	type KeyboardShortcutBindings,
	type KeyboardShortcutId,
} from './keyboard-shortcut-catalog';
import {
	createKeyboardShortcutBindings,
	keyboardShortcutOverridesEqual,
	replaceKeyboardShortcutOverride,
	type KeyboardShortcutOverride,
} from './keyboard-shortcut-overrides';
import {
	readKeyboardShortcutOverrides,
	writeKeyboardShortcutOverrides,
	type KeyboardShortcutPersistenceIssue,
	type KeyboardShortcutStorage,
} from './keyboard-shortcut-storage';

export interface KeyboardShortcutSettingsSnapshot {
	readonly bindings: KeyboardShortcutBindings;
	readonly overrides: readonly KeyboardShortcutOverride[];
	readonly persistenceIssue: KeyboardShortcutPersistenceIssue | null;
}

interface KeyboardShortcutConflict {
	readonly chord: KeyboardShortcutChord;
	readonly conflictingShortcutId: KeyboardShortcutId;
	readonly shortcutId: KeyboardShortcutId;
}

type KeyboardShortcutUpdateResult =
	| { readonly kind: 'applied-and-saved' }
	| { readonly kind: 'applied-session-only' }
	| { readonly conflict: KeyboardShortcutConflict; readonly kind: 'conflict' }
	| { readonly kind: 'disposed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'unchanged' };

interface KeyboardShortcutSettingsControllerOptions {
	readonly platform: KeyboardShortcutPlatform;
	readonly storage: KeyboardShortcutStorage;
}

type Listener = () => void;

export class KeyboardShortcutSettingsController implements ProjectedTerminalShortcutBindingSource {
	readonly #listeners = new Set<Listener>();
	readonly #platform: KeyboardShortcutPlatform;
	readonly #persistence = new PreferenceWriteQueue();
	readonly #storage: KeyboardShortcutStorage;
	#disposed = false;
	#revision = 0;
	#snapshot: KeyboardShortcutSettingsSnapshot;

	constructor({ platform, storage }: KeyboardShortcutSettingsControllerOptions) {
		this.#platform = platform;
		this.#storage = storage;
		const restored = readKeyboardShortcutOverrides(storage, platform);
		this.#snapshot = createSnapshot(
			createKeyboardShortcutBindings(platform, restored.overrides),
			restored.overrides,
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
		return this.#apply(shortcutId, chord, overrides);
	}

	resetShortcut(shortcutId: KeyboardShortcutId): KeyboardShortcutUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		const overrides = replaceKeyboardShortcutOverride(this.#snapshot.overrides, shortcutId, null);
		return this.#apply(
			shortcutId,
			defaultKeyboardShortcutBindings(this.#platform)[shortcutId],
			overrides,
		);
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#revision += 1;
		this.#persistence.dispose();
		this.#listeners.clear();
	}

	#apply(
		shortcutId: KeyboardShortcutId,
		chord: KeyboardShortcutChord,
		overrides: readonly KeyboardShortcutOverride[],
	): KeyboardShortcutUpdateResult {
		const bindings = createKeyboardShortcutBindings(this.#platform, overrides);
		const conflictPair = findKeyboardShortcutConflict(bindings, this.#platform);
		if (conflictPair !== null) {
			const conflictingShortcutId =
				conflictPair.firstShortcutId === shortcutId
					? conflictPair.secondShortcutId
					: conflictPair.firstShortcutId;
			return {
				conflict: { chord, conflictingShortcutId, shortcutId },
				kind: 'conflict',
			};
		}
		const changed = !keyboardShortcutOverridesEqual(
			this.#snapshot.overrides,
			overrides,
			this.#platform,
		);
		if (!changed && this.#snapshot.persistenceIssue === null) return { kind: 'unchanged' };

		const revision = ++this.#revision;
		this.#snapshot = createSnapshot(bindings, overrides, this.#snapshot.persistenceIssue);
		this.#persistence.write(() => {
			const saved = writeKeyboardShortcutOverrides(this.#storage, overrides);
			if (this.#disposed || this.#revision !== revision) return;
			this.#snapshot = createSnapshot(bindings, overrides, saved ? null : 'write-failed');
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
	shortcutId: KeyboardShortcutId,
): 'custom' | 'default' {
	return snapshot.overrides.some((override) => override.shortcutId === shortcutId)
		? 'custom'
		: 'default';
}

function createSnapshot(
	bindings: KeyboardShortcutBindings,
	overrides: readonly KeyboardShortcutOverride[],
	persistenceIssue: KeyboardShortcutPersistenceIssue | null,
): KeyboardShortcutSettingsSnapshot {
	return Object.freeze({ bindings, overrides, persistenceIssue });
}
