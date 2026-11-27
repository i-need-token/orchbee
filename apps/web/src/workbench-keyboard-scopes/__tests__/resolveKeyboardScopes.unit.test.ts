import type {
	KeyboardShortcutChord,
	KeyboardShortcutEvent,
	KeyboardShortcutPlatform,
	KeyboardShortcutSequence,
} from '@orchbee/keyboard-shortcuts';
import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	resolveKeyboardScopes,
	type KeyboardScopeFrame,
	type KeyboardScopeResolution,
	type KeyboardSequencePending,
} from '../resolveKeyboardScopes';

/** A sequence waits five seconds for its second step, as VS Code's does. */
const KEYBOARD_SEQUENCE_TIMEOUT_MILLISECONDS = 5_000;
const LEADER: KeyboardShortcutChord = { code: 'KeyK', modifiers: ['mod'] };
const CODES = ['KeyA', 'KeyW', 'F2', 'Enter', 'Delete'] as const;
const MODIFIER_SETS = [[], ['mod'], ['shift'], ['mod', 'shift'], ['alt']] as const;

const platformArbitrary = fc.constantFrom<KeyboardShortcutPlatform>('apple', 'standard');
const chordArbitrary: fc.Arbitrary<KeyboardShortcutChord> = fc.record({
	code: fc.constantFrom(...CODES),
	modifiers: fc.constantFrom(...MODIFIER_SETS),
});
const sequenceArbitrary: fc.Arbitrary<KeyboardShortcutSequence> = fc.oneof(
	chordArbitrary.map((chord) => [chord] as const),
	chordArbitrary.map((chord) => [LEADER, chord] as const),
);

interface GeneratedBinding {
	readonly available: boolean;
	readonly sequence: KeyboardShortcutSequence;
}

interface GeneratedFrame {
	readonly bindings: readonly GeneratedBinding[];
	/** Whether the frame has a fallback owner, and whether that owner takes keys. */
	readonly fallback: boolean | null;
	readonly sequences: boolean;
	readonly yields: readonly KeyboardShortcutChord[];
}

const framesArbitrary: fc.Arbitrary<readonly GeneratedFrame[]> = fc.array(
	fc.record({
		bindings: fc.array(fc.record({ available: fc.boolean(), sequence: sequenceArbitrary }), {
			maxLength: 4,
		}),
		fallback: fc.option(fc.boolean(), { freq: 4 }),
		sequences: fc.boolean(),
		yields: fc.oneof(fc.constant([]), fc.array(chordArbitrary, { maxLength: 2 })),
	}),
	{ maxLength: 5 },
);

/** Names the frame and binding a target came from; a fallback has no binding. */
interface Target {
	readonly available: boolean;
	readonly binding: number | 'fallback';
	readonly frame: number;
}

function framesOf(generated: readonly GeneratedFrame[]): readonly KeyboardScopeFrame<Target>[] {
	return generated.map((frame, frameIndex) => ({
		bindings: frame.bindings.map((binding, bindingIndex) => ({
			sequence: binding.sequence,
			target: { available: binding.available, binding: bindingIndex, frame: frameIndex },
		})),
		fallback:
			frame.fallback === null
				? undefined
				: { available: frame.fallback, binding: 'fallback', frame: frameIndex },
		sequences: frame.sequences,
		yields: frame.yields,
	}));
}

function withoutRefused(generated: readonly GeneratedFrame[]): readonly GeneratedFrame[] {
	return generated.map((frame) => ({
		...frame,
		bindings: frame.bindings.filter((binding) => binding.available),
		fallback: frame.fallback === true ? true : null,
	}));
}

function press(
	chord: KeyboardShortcutChord,
	platform: KeyboardShortcutPlatform,
	overrides: Partial<KeyboardShortcutEvent> = {},
): KeyboardShortcutEvent {
	const modifiers = new Set<string>(chord.modifiers);
	return {
		altKey: modifiers.has('alt'),
		code: chord.code,
		ctrlKey: platform === 'standard' && modifiers.has('mod'),
		isComposing: false,
		metaKey: platform === 'apple' && modifiers.has('mod'),
		repeat: false,
		shiftKey: modifiers.has('shift'),
		...overrides,
	};
}

function sameChord(left: KeyboardShortcutChord, right: KeyboardShortcutChord): boolean {
	return (
		left.code === right.code &&
		left.modifiers.length === right.modifiers.length &&
		left.modifiers.every((modifier) => right.modifiers.includes(modifier))
	);
}

function bindingOf(generated: readonly GeneratedFrame[], target: Target): GeneratedBinding | null {
	return target.binding === 'fallback'
		? null
		: (generated[target.frame]?.bindings[target.binding] ?? null);
}

function resolve(
	generated: readonly GeneratedFrame[],
	event: KeyboardShortcutEvent,
	platform: KeyboardShortcutPlatform,
	pending: KeyboardSequencePending | null = null,
	now = 0,
): { readonly offered: readonly Target[]; readonly resolution: KeyboardScopeResolution<Target> } {
	const offered: Target[] = [];
	const resolution = resolveKeyboardScopes(
		{ event, frames: framesOf(generated), now, pending, platform },
		(target) => {
			offered.push(target);
			return target.available;
		},
	);
	return { offered, resolution };
}

/** A resolution with a match reduced to the frame it came from, which outlives filtering. */
function summarize(resolution: KeyboardScopeResolution<Target>) {
	return resolution.kind === 'match'
		? { frame: resolution.target.frame, kind: 'match' as const }
		: resolution;
}

function admitsSequences(generated: readonly GeneratedFrame[]): boolean {
	return generated.some((frame) => frame.sequences);
}

/** What a waiting sequence's second step must resolve to, by the rules alone. */
function expectedSecondStep(
	generated: readonly GeneratedFrame[],
	chord: KeyboardShortcutChord,
	asFirstKey: KeyboardScopeResolution<Target>,
): ReturnType<typeof summarize> {
	if (!admitsSequences(generated)) return summarize(asFirstKey);
	const owner = generated.findIndex((frame) =>
		frame.bindings.some(
			(binding) =>
				binding.available &&
				binding.sequence[1] !== undefined &&
				sameChord(binding.sequence[1], chord),
		),
	);
	return owner === -1 ? { kind: 'missed' } : { frame: owner, kind: 'match' };
}

describe('keyboard scope resolution', () => {
	it('runs the narrowest available single key and never consumes a key for an unavailable one', () => {
		fc.assert(
			fc.property(
				platformArbitrary,
				framesArbitrary,
				chordArbitrary,
				(platform, generated, chord) => {
					const { offered, resolution } = resolve(generated, press(chord, platform), platform);
					const takes = (frame: GeneratedFrame): boolean =>
						frame.fallback === true ||
						frame.bindings.some(
							(binding) =>
								binding.available &&
								binding.sequence.length === 1 &&
								sameChord(binding.sequence[0], chord),
						);
					const yieldsKey = (frame: GeneratedFrame): boolean =>
						frame.yields.some((yielded) => sameChord(yielded, chord));
					const owner = generated.findIndex((frame) => takes(frame) || yieldsKey(frame));
					const ownerFrame = generated[owner];
					const taken = ownerFrame !== undefined && takes(ownerFrame);

					for (const target of offered) {
						const binding = bindingOf(generated, target);
						if (binding === null) continue;
						expect(binding.sequence.length).toBe(1);
						expect(sameChord(binding.sequence[0], chord)).toBe(true);
					}
					expect(summarize(resolution)).toEqual(
						taken ? { frame: owner, kind: 'match' } : { kind: 'none', pending: null },
					);
					expect(offered.filter((target) => target.available)).toEqual(
						resolution.kind === 'match' ? [resolution.target] : [],
					);
				},
			),
			{ numRuns: 400, seed: 2_026_11_27 },
		);
	});

	it('resolves the same without the targets it refuses', () => {
		fc.assert(
			fc.property(
				platformArbitrary,
				framesArbitrary,
				fc.oneof(chordArbitrary, fc.constant(LEADER)),
				fc.boolean(),
				(platform, generated, chord, waiting) => {
					const pending = waiting ? { deadline: KEYBOARD_SEQUENCE_TIMEOUT_MILLISECONDS } : null;
					const event = press(chord, platform);
					const all = resolve(generated, event, platform, pending).resolution;
					const accepted = resolve(withoutRefused(generated), event, platform, pending).resolution;
					expect(summarize(accepted)).toEqual(summarize(all));
				},
			),
			{ numRuns: 400, seed: 2_026_11_28 },
		);
	});

	it('starts a sequence with Mod+K only where a scope admits one, whatever its availability', () => {
		fc.assert(
			fc.property(
				platformArbitrary,
				framesArbitrary,
				fc.nat(1_000_000),
				(platform, generated, now) => {
					const { offered, resolution } = resolve(
						generated,
						press(LEADER, platform),
						platform,
						null,
						now,
					);
					const sequences = admitsSequences(generated);
					expect(resolution.kind === 'pending' ? resolution.pending : null).toEqual(
						sequences ? { deadline: now + KEYBOARD_SEQUENCE_TIMEOUT_MILLISECONDS } : null,
					);
					expect(sequences ? offered : []).toEqual([]);
				},
			),
			{ numRuns: 300, seed: 2_026_11_29 },
		);
	});

	it('ends a waiting sequence on its second step, which runs an available binding or is missed', () => {
		fc.assert(
			fc.property(
				platformArbitrary,
				framesArbitrary,
				chordArbitrary,
				(platform, generated, chord) => {
					const pending = { deadline: KEYBOARD_SEQUENCE_TIMEOUT_MILLISECONDS };
					const event = press(chord, platform);
					const { offered, resolution } = resolve(generated, event, platform, pending, 1);
					// Where no scope admits sequences, as in a terminal, the key is a first key.
					const asFirstKey = resolve(generated, event, platform, null, 1).resolution;

					expect(summarize(resolution)).toEqual(expectedSecondStep(generated, chord, asFirstKey));
					for (const target of admitsSequences(generated) ? offered : []) {
						expect(bindingOf(generated, target)?.sequence.length).toBe(2);
					}
				},
			),
			{ numRuns: 400, seed: 2_026_11_30 },
		);
	});

	it('forgets a sequence at its deadline or outside sequence scopes, keeps it through modifiers and repeats, and drops it on composition', () => {
		fc.assert(
			fc.property(
				platformArbitrary,
				framesArbitrary,
				chordArbitrary,
				fc.nat(KEYBOARD_SEQUENCE_TIMEOUT_MILLISECONDS * 2),
				(platform, generated, chord, elapsed) => {
					const pending = { deadline: KEYBOARD_SEQUENCE_TIMEOUT_MILLISECONDS };
					const sequences = admitsSequences(generated);
					const event = press(chord, platform);
					const late = resolve(generated, event, platform, pending, elapsed).resolution;
					const forgotten = elapsed >= pending.deadline || !sequences;
					expect(forgotten ? late : null).toEqual(
						forgotten ? resolve(generated, event, platform, null, elapsed).resolution : null,
					);
					expect(forgotten || late.kind === 'match' || late.kind === 'missed').toBe(true);

					const now = Math.min(elapsed, pending.deadline - 1);
					const shift = press({ code: 'ShiftLeft', modifiers: ['shift'] }, platform);
					expect(resolve(generated, shift, platform, pending, now).resolution).toEqual({
						kind: 'none',
						pending,
					});
					const repeat = press(chord, platform, { repeat: true });
					expect(resolve(generated, repeat, platform, pending, now).resolution).toEqual(
						sequences
							? { kind: 'pending', pending }
							: resolve(generated, repeat, platform, null, now).resolution,
					);
					const composing = press(chord, platform, { keyCode: 229 });
					expect(resolve(generated, composing, platform, pending, now).resolution).toEqual({
						kind: 'none',
						pending: null,
					});
				},
			),
			{ numRuns: 300, seed: 2_026_12_01 },
		);
	});
});
