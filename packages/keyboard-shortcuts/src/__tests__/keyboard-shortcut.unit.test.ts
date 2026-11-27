import { describe, expect, it } from 'vitest';

import {
	captureKeyboardShortcutChord,
	formatKeyboardShortcutChord,
	formatKeyboardShortcutSequence,
	keyboardShortcutChordKey,
	keyboardShortcutSequenceKey,
	matchesKeyboardShortcutChord,
	normalizeKeyboardShortcutChord,
	normalizeKeyboardShortcutSequence,
	type KeyboardShortcutEvent,
} from '../keyboard-shortcut';

describe('keyboard shortcut chords', () => {
	it('captures the platform primary modifier as Mod and formats one canonical chord', () => {
		const apple = captureKeyboardShortcutChord(
			shortcutEvent({ code: 'KeyP', metaKey: true, shiftKey: true }),
			'apple',
		);
		const standard = captureKeyboardShortcutChord(
			shortcutEvent({ code: 'KeyP', ctrlKey: true, shiftKey: true }),
			'standard',
		);

		expect(apple).toEqual({
			chord: { code: 'KeyP', modifiers: ['mod', 'shift'] },
			kind: 'captured',
		});
		expect(standard).toEqual(apple);
		if (apple.kind !== 'captured' || standard.kind !== 'captured') return;
		expect(formatKeyboardShortcutChord(apple.chord, 'apple')).toBe('⇧⌘P');
		expect(formatKeyboardShortcutChord(standard.chord, 'standard')).toBe('Ctrl+Shift+P');
	});

	it('formats Apple modifier glyphs in platform menu order', () => {
		const chord = normalizeKeyboardShortcutChord(
			{ code: 'KeyA', modifiers: ['mod', 'control', 'alt', 'shift'] },
			'apple',
		);
		expect(chord).not.toBeNull();
		if (chord === null) return;
		expect(formatKeyboardShortcutChord(chord, 'apple')).toBe('⌃⌥⇧⌘A');
	});

	it('preserves the literal secondary Control and Meta modifiers', () => {
		const apple = captureKeyboardShortcutChord(
			shortcutEvent({ code: 'KeyA', ctrlKey: true }),
			'apple',
		);
		const standard = captureKeyboardShortcutChord(
			shortcutEvent({ code: 'KeyA', metaKey: true }),
			'standard',
		);
		expect(apple).toEqual({
			chord: { code: 'KeyA', modifiers: ['control'] },
			kind: 'captured',
		});
		expect(standard).toEqual({
			chord: { code: 'KeyA', modifiers: ['meta'] },
			kind: 'captured',
		});
		if (apple.kind !== 'captured' || standard.kind !== 'captured') return;
		expect(formatKeyboardShortcutChord(apple.chord, 'apple')).toBe('⌃A');
		expect(formatKeyboardShortcutChord(standard.chord, 'standard')).toBe('Super+A');
	});

	it('rejects aliases that collapse to the same effective platform modifier', () => {
		expect(
			normalizeKeyboardShortcutChord({ code: 'KeyA', modifiers: ['mod', 'meta'] }, 'apple'),
		).toBeNull();
		expect(
			normalizeKeyboardShortcutChord({ code: 'KeyA', modifiers: ['mod', 'control'] }, 'standard'),
		).toBeNull();
	});

	it('ignores unsafe, composing, repeated, and modifier-only captures', () => {
		expect(captureKeyboardShortcutChord(shortcutEvent({ code: 'KeyA' }), 'standard')).toEqual({
			kind: 'ignored',
			reason: 'missing-modifier',
		});
		expect(
			captureKeyboardShortcutChord(
				shortcutEvent({ code: 'KeyA', ctrlKey: true, isComposing: true }),
				'standard',
			),
		).toEqual({ kind: 'ignored', reason: 'composing' });
		expect(
			captureKeyboardShortcutChord(
				shortcutEvent({ code: 'KeyA', ctrlKey: true, keyCode: 229 }),
				'standard',
			),
		).toEqual({ kind: 'ignored', reason: 'composing' });
		expect(
			captureKeyboardShortcutChord(
				shortcutEvent({ code: 'KeyA', ctrlKey: true, repeat: true }),
				'standard',
			),
		).toEqual({ kind: 'ignored', reason: 'repeat' });
		expect(
			captureKeyboardShortcutChord(
				shortcutEvent({ code: 'ControlLeft', ctrlKey: true }),
				'standard',
			),
		).toEqual({ kind: 'ignored', reason: 'modifier-only' });
		expect(
			captureKeyboardShortcutChord(shortcutEvent({ code: 'Escape', ctrlKey: true }), 'standard'),
		).toEqual({ kind: 'ignored', reason: 'unsupported-code' });
	});

	it('matches the complete modifier set and rejects composition and repeat', () => {
		const chord = normalizeKeyboardShortcutChord(
			{ code: 'KeyF', modifiers: ['mod', 'shift'] },
			'standard',
		);
		expect(chord).not.toBeNull();
		if (chord === null) return;
		expect(
			matchesKeyboardShortcutChord(
				shortcutEvent({ code: 'KeyF', ctrlKey: true, shiftKey: true }),
				chord,
				'standard',
			),
		).toBe(true);
		expect(
			matchesKeyboardShortcutChord(
				shortcutEvent({ altKey: true, code: 'KeyF', ctrlKey: true, shiftKey: true }),
				chord,
				'standard',
			),
		).toBe(false);
		expect(
			matchesKeyboardShortcutChord(
				shortcutEvent({ code: 'KeyF', ctrlKey: true, repeat: true, shiftKey: true }),
				chord,
				'standard',
			),
		).toBe(false);
		expect(
			matchesKeyboardShortcutChord(
				shortcutEvent({ code: 'KeyF', ctrlKey: true, keyCode: 229, shiftKey: true }),
				chord,
				'standard',
			),
		).toBe(false);
	});

	it('derives one effective identity for equivalent platform aliases', () => {
		const mod = normalizeKeyboardShortcutChord({ code: 'Comma', modifiers: ['mod'] }, 'standard');
		const control = normalizeKeyboardShortcutChord(
			{ code: 'Comma', modifiers: ['control'] },
			'standard',
		);
		expect(mod).not.toBeNull();
		expect(control).not.toBeNull();
		if (mod === null || control === null) return;
		expect(keyboardShortcutChordKey(mod, 'standard')).toBe(
			keyboardShortcutChordKey(control, 'standard'),
		);
	});

	it('strictly rejects unknown fields, duplicate modifiers, and unsupported codes', () => {
		expect(
			normalizeKeyboardShortcutChord(
				{ code: 'KeyA', modifiers: ['mod'], unexpected: true },
				'apple',
			),
		).toBeNull();
		expect(
			normalizeKeyboardShortcutChord({ code: 'KeyA', modifiers: ['mod', 'mod'] }, 'apple'),
		).toBeNull();
		expect(
			normalizeKeyboardShortcutChord({ code: 'NumpadAdd', modifiers: ['mod'] }, 'apple'),
		).toBeNull();
	});
});

describe('keyboard shortcut rules and sequences', () => {
	it('admits unmodified keys only where a rule allows them and never Tab', () => {
		const f2 = { code: 'F2', modifiers: [] };
		expect(normalizeKeyboardShortcutChord(f2, 'standard')).toBeNull();
		expect(normalizeKeyboardShortcutChord(f2, 'standard', 'widget')).toEqual(f2);
		expect(normalizeKeyboardShortcutChord(f2, 'standard', 'second-step')).toEqual(f2);
		expect(
			normalizeKeyboardShortcutChord({ code: 'Delete', modifiers: ['shift'] }, 'apple', 'widget'),
		).toEqual({ code: 'Delete', modifiers: ['shift'] });
		const f4 = { code: 'F4', modifiers: [] };
		expect(normalizeKeyboardShortcutChord(f4, 'standard', 'region')).toEqual(f4);
		expect(
			normalizeKeyboardShortcutChord({ code: 'F4', modifiers: ['shift'] }, 'apple', 'region'),
		).toEqual({ code: 'F4', modifiers: ['shift'] });
		expect(
			normalizeKeyboardShortcutChord({ code: 'KeyP', modifiers: ['shift'] }, 'apple', 'region'),
		).toBeNull();
		for (const rule of ['window', 'region', 'widget', 'second-step'] as const) {
			expect(
				normalizeKeyboardShortcutChord({ code: 'Tab', modifiers: [] }, 'apple', rule),
			).toBeNull();
		}
		expect(
			captureKeyboardShortcutChord(shortcutEvent({ code: 'F2' }), 'standard', 'widget'),
		).toEqual({ chord: f2, kind: 'captured' });
		expect(captureKeyboardShortcutChord(shortcutEvent({ code: 'F2' }), 'standard')).toEqual({
			kind: 'ignored',
			reason: 'missing-modifier',
		});
	});

	it('admits Escape only as a declared widget default, never as a capture', () => {
		const escape = { code: 'Escape', modifiers: [] };
		expect(normalizeKeyboardShortcutChord(escape, 'apple', 'widget')).toEqual(escape);
		expect(normalizeKeyboardShortcutChord(escape, 'apple', 'second-step')).toBeNull();
		expect(
			captureKeyboardShortcutChord(shortcutEvent({ code: 'Escape' }), 'apple', 'widget'),
		).toEqual({ kind: 'ignored', reason: 'unsupported-code' });
		expect(matchesKeyboardShortcutChord(shortcutEvent({ code: 'Escape' }), escape, 'apple')).toBe(
			true,
		);
		expect(formatKeyboardShortcutChord(escape, 'standard')).toBe('Escape');
	});

	it('requires Mod+K before a second step and formats each step', () => {
		const sequence = normalizeKeyboardShortcutSequence(
			[
				{ code: 'KeyK', modifiers: ['control'] },
				{ code: 'KeyW', modifiers: ['mod'] },
			],
			'standard',
		);
		expect(sequence).toEqual([
			{ code: 'KeyK', modifiers: ['mod'] },
			{ code: 'KeyW', modifiers: ['mod'] },
		]);
		if (sequence === null) return;
		expect(formatKeyboardShortcutSequence(sequence, 'standard')).toBe('Ctrl+K Ctrl+W');
		expect(formatKeyboardShortcutSequence(sequence, 'apple')).toBe('⌘K ⌘W');
		expect(
			normalizeKeyboardShortcutSequence(
				[
					{ code: 'KeyK', modifiers: ['mod'] },
					{ code: 'Enter', modifiers: [] },
				],
				'apple',
			),
		).not.toBeNull();
		for (const invalid of [
			[
				{ code: 'KeyJ', modifiers: ['mod'] },
				{ code: 'KeyW', modifiers: ['mod'] },
			],
			[
				{ code: 'KeyK', modifiers: ['mod', 'shift'] },
				{ code: 'KeyW', modifiers: [] },
			],
			[
				{ code: 'KeyK', modifiers: ['mod'] },
				{ code: 'Escape', modifiers: [] },
			],
			[
				{ code: 'KeyK', modifiers: ['mod'] },
				{ code: 'Tab', modifiers: [] },
			],
			[],
		]) {
			expect(normalizeKeyboardShortcutSequence(invalid, 'apple')).toBeNull();
		}
		const single = normalizeKeyboardShortcutSequence(
			[{ code: 'KeyW', modifiers: ['mod'] }],
			'apple',
		);
		expect(single).not.toBeNull();
		if (single === null) return;
		expect(keyboardShortcutSequenceKey(single, 'apple')).not.toBe(
			keyboardShortcutSequenceKey(sequence, 'apple'),
		);
	});
});

function shortcutEvent(
	overrides: Partial<KeyboardShortcutEvent> & Pick<KeyboardShortcutEvent, 'code'>,
): KeyboardShortcutEvent {
	return {
		altKey: false,
		ctrlKey: false,
		isComposing: false,
		metaKey: false,
		repeat: false,
		shiftKey: false,
		...overrides,
	};
}
