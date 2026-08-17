import { formatKeyboardShortcutChord, keyboardShortcutChordKey } from '@orchbee/keyboard-shortcuts';
import { describe, expect, it } from 'vitest';

import {
	PROJECTED_TERMINAL_SHORTCUTS,
	defaultProjectedTerminalShortcutBindings,
	resolveProjectedTerminalShortcut,
} from '../projected-terminal-keyboard-shortcuts';

describe('projected terminal shortcut declarations', () => {
	it('owns three unique Apple and terminal-safe Standard defaults', () => {
		const apple = defaultProjectedTerminalShortcutBindings('apple');
		const standard = defaultProjectedTerminalShortcutBindings('standard');
		expect(formatKeyboardShortcutChord(apple['terminal.search.open'], 'apple')).toBe('⌘F');
		expect(formatKeyboardShortcutChord(apple['terminal.selection.copy'], 'apple')).toBe('⌘C');
		expect(formatKeyboardShortcutChord(apple['terminal.selection.select-all'], 'apple')).toBe('⌘A');
		expect(formatKeyboardShortcutChord(standard['terminal.search.open'], 'standard')).toBe(
			'Ctrl+Shift+F',
		);
		expect(formatKeyboardShortcutChord(standard['terminal.selection.copy'], 'standard')).toBe(
			'Ctrl+Shift+C',
		);
		expect(formatKeyboardShortcutChord(standard['terminal.selection.select-all'], 'standard')).toBe(
			'Ctrl+Shift+A',
		);
		expect(
			new Set(
				Object.values(PROJECTED_TERMINAL_SHORTCUTS).map((definition) =>
					keyboardShortcutChordKey(definition.defaults.standard, 'standard'),
				),
			).size,
		).toBe(3);
	});

	it('does not intercept the unshifted Standard control sequence', () => {
		expect(
			resolveProjectedTerminalShortcut(
				{
					altKey: false,
					code: 'KeyF',
					ctrlKey: true,
					isComposing: false,
					metaKey: false,
					repeat: false,
					shiftKey: false,
				},
				'standard',
				'terminal',
			),
		).toBeNull();
	});
});
