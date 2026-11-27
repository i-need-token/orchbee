import {
	KEYBOARD_SHORTCUT_SEQUENCE_LEADER,
	isKeyboardShortcutModifierCode,
	matchesKeyboardShortcutChord,
	type KeyboardShortcutChord,
	type KeyboardShortcutEvent,
	type KeyboardShortcutPlatform,
	type KeyboardShortcutSequence,
} from '@orchbee/keyboard-shortcuts';

/** How long a started sequence waits for its second step, as VS Code waits. */
const KEYBOARD_SEQUENCE_TIMEOUT_MILLISECONDS = 5_000;

export interface KeyboardScopeBinding<T> {
	readonly sequence: KeyboardShortcutSequence;
	readonly target: T;
}

/** One scope on the focus path. */
export interface KeyboardScopeFrame<T> {
	readonly bindings: readonly KeyboardScopeBinding<T>[];
	/**
	 * Offered every single key that reaches this scope after its own bindings, for an owner that
	 * matches keys itself, such as configured commands.
	 */
	readonly fallback?: T | undefined;
	/** Whether `Mod+K` starts a sequence while this scope is on the focus path. */
	readonly sequences: boolean;
	/**
	 * The chords this scope's widget handles itself, such as a code editor's own key table: the walk
	 * stops at this scope and the key goes on to the widget. A sequence never yields.
	 */
	readonly yields?: readonly KeyboardShortcutChord[] | undefined;
}

/** `Mod+K` started a sequence; its second step must come before `deadline`. */
export interface KeyboardSequencePending {
	readonly deadline: number;
}

export interface KeyboardScopeRequest<T> {
	readonly event: KeyboardShortcutEvent;
	/** The scopes on the focus path, narrowest first. */
	readonly frames: readonly KeyboardScopeFrame<T>[];
	/** The current time on the clock that set the pending deadline. */
	readonly now: number;
	readonly pending: KeyboardSequencePending | null;
	readonly platform: KeyboardShortcutPlatform;
}

export type KeyboardScopeResolution<T> =
	/** The key goes on to the focused widget; `pending` is the sequence still waiting, if any. */
	| { readonly kind: 'none'; readonly pending: KeyboardSequencePending | null }
	/** `target` accepted the key, which is consumed and ends any sequence. */
	| { readonly kind: 'match'; readonly target: T }
	/** The key is consumed and a sequence waits: it started one, or repeated while one waits. */
	| { readonly kind: 'pending'; readonly pending: KeyboardSequencePending }
	/** The key is consumed: as a second step it names nothing available, so the sequence ends. */
	| { readonly kind: 'missed' };

/**
 * Resolves one key press over the scopes on the focus path. `accept` is offered each matching
 * target, narrowest scope first, and says whether it is available, running it if so; the first
 * target it accepts wins, and a refused target never consumes the key.
 *
 * Sequences live only where a scope on the path admits them. There, while no sequence waits, `Mod+K`
 * starts one whatever its second steps' availability; while one waits, the next key other than a
 * modifier is its second step: a two-step binding on the path takes it or it is missed, and either
 * way the sequence ends. A repeat is consumed without ending it. Composition, or a key where no scope
 * admits sequences, as in a terminal, ends a waiting sequence and goes on as any other key.
 */
export function resolveKeyboardScopes<T>(
	request: KeyboardScopeRequest<T>,
	accept: (target: T) => boolean,
): KeyboardScopeResolution<T> {
	const { event, frames, now, platform } = request;
	const pending =
		request.pending !== null && now < request.pending.deadline ? request.pending : null;
	if (isKeyboardShortcutModifierCode(event.code)) return { kind: 'none', pending };
	if (event.isComposing || event.keyCode === 229) return { kind: 'none', pending: null };
	const sequences = frames.some((frame) => frame.sequences);
	if (sequences && pending !== null) {
		if (event.repeat) return { kind: 'pending', pending };
		for (const frame of frames) {
			for (const binding of frame.bindings) {
				const [, step] = binding.sequence;
				if (step === undefined || !matchesKeyboardShortcutChord(event, step, platform)) continue;
				if (accept(binding.target)) return { kind: 'match', target: binding.target };
			}
		}
		return { kind: 'missed' };
	}
	if (
		sequences &&
		matchesKeyboardShortcutChord(event, KEYBOARD_SHORTCUT_SEQUENCE_LEADER, platform)
	) {
		return {
			kind: 'pending',
			pending: { deadline: now + KEYBOARD_SEQUENCE_TIMEOUT_MILLISECONDS },
		};
	}
	for (const frame of frames) {
		for (const binding of frame.bindings) {
			const [chord, step] = binding.sequence;
			if (step !== undefined || !matchesKeyboardShortcutChord(event, chord, platform)) continue;
			if (accept(binding.target)) return { kind: 'match', target: binding.target };
		}
		if (frame.fallback !== undefined && accept(frame.fallback)) {
			return { kind: 'match', target: frame.fallback };
		}
		if (frame.yields?.some((chord) => matchesKeyboardShortcutChord(event, chord, platform))) {
			return { kind: 'none', pending: null };
		}
	}
	return { kind: 'none', pending: null };
}
