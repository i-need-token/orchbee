import { formatKeyboardShortcutChord, keyboardShortcutChordKey } from '@orchbee/keyboard-shortcuts';
import { describe, expect, it } from 'vitest';

import {
	PROJECTED_TERMINAL_SHORTCUTS,
	defaultProjectedTerminalShortcutBindings,
	formatProjectedTerminalActionShortcut,
	resolveProjectedTerminalShortcut,
} from '../projected-terminal-keyboard-shortcuts';

describe('projected terminal shortcut declarations', () => {
	it('owns sixteen unique Apple and terminal-safe Standard defaults', () => {
		const apple = defaultProjectedTerminalShortcutBindings('apple');
		const standard = defaultProjectedTerminalShortcutBindings('standard');
		expect(formatKeyboardShortcutChord(apple['terminal.clear'], 'apple')).toBe('⌘K');
		expect(formatKeyboardShortcutChord(apple['terminal.search.open'], 'apple')).toBe('⌘F');
		expect(formatKeyboardShortcutChord(apple['terminal.hints.open'], 'apple')).toBe('⇧⌘O');
		expect(formatKeyboardShortcutChord(apple['terminal.output.show-last'], 'apple')).toBe('⌃⇧G');
		expect(formatKeyboardShortcutChord(apple['terminal.history.show'], 'apple')).toBe('⌃⇧H');
		expect(formatKeyboardShortcutChord(apple['terminal.selection.copy'], 'apple')).toBe('⌘C');
		expect(formatKeyboardShortcutChord(apple['terminal.clipboard.paste'], 'apple')).toBe('⌘V');
		expect(formatKeyboardShortcutChord(apple['terminal.selection.select-all'], 'apple')).toBe('⌘A');
		expect(formatKeyboardShortcutChord(apple['terminal.history.prompt-previous'], 'apple')).toBe(
			'⌃⇧Z',
		);
		expect(formatKeyboardShortcutChord(apple['terminal.history.prompt-next'], 'apple')).toBe('⌃⇧X');
		expect(apple['terminal.history.scroll-line-up']).toEqual({
			code: 'ArrowUp',
			modifiers: ['control', 'shift'],
		});
		expect(apple['terminal.history.scroll-page-down']).toEqual({
			code: 'PageDown',
			modifiers: ['control', 'shift'],
		});
		expect(apple['terminal.history.scroll-top']).toEqual({
			code: 'Home',
			modifiers: ['control', 'shift'],
		});
		expect(formatKeyboardShortcutChord(standard['terminal.search.open'], 'standard')).toBe(
			'Ctrl+Shift+F',
		);
		expect(formatKeyboardShortcutChord(standard['terminal.clear'], 'standard')).toBe(
			'Ctrl+Shift+L',
		);
		expect(formatKeyboardShortcutChord(standard['terminal.selection.copy'], 'standard')).toBe(
			'Ctrl+Shift+C',
		);
		expect(formatKeyboardShortcutChord(standard['terminal.clipboard.paste'], 'standard')).toBe(
			'Ctrl+Shift+V',
		);
		expect(formatKeyboardShortcutChord(standard['terminal.selection.select-all'], 'standard')).toBe(
			'Ctrl+Shift+A',
		);
		expect(
			formatKeyboardShortcutChord(standard['terminal.history.prompt-previous'], 'standard'),
		).toBe('Ctrl+Shift+Z');
		expect(formatKeyboardShortcutChord(standard['terminal.history.prompt-next'], 'standard')).toBe(
			'Ctrl+Shift+X',
		);
		expect(
			formatKeyboardShortcutChord(standard['terminal.history.scroll-line-down'], 'standard'),
		).toBe('Ctrl+Shift+↓');
		expect(
			formatKeyboardShortcutChord(standard['terminal.history.scroll-page-up'], 'standard'),
		).toBe('Ctrl+Shift+Page Up');
		expect(
			formatKeyboardShortcutChord(standard['terminal.history.scroll-bottom'], 'standard'),
		).toBe('Ctrl+Shift+End');
		expect(
			new Set(
				Object.values(PROJECTED_TERMINAL_SHORTCUTS).map((definition) =>
					keyboardShortcutChordKey(definition.defaults.standard, 'standard'),
				),
			).size,
		).toBe(16);
		expect(formatProjectedTerminalActionShortcut('terminal.clear', 'apple')).toBe('⌘K');
		expect(formatProjectedTerminalActionShortcut('terminal.clipboard.paste', 'apple')).toBe('⌘V');
		expect(formatProjectedTerminalActionShortcut('terminal.history.prompt-previous', 'apple')).toBe(
			'⌃⇧Z',
		);
		expect(formatProjectedTerminalActionShortcut('unknown', 'apple')).toBeNull();
	});

	it('resolves scrollback only while the terminal is focused', () => {
		const event = {
			altKey: false,
			code: 'KeyH',
			ctrlKey: true,
			isComposing: false,
			metaKey: false,
			repeat: false,
			shiftKey: true,
		};

		expect(resolveProjectedTerminalShortcut(event, 'apple', 'terminal')).toMatchObject({
			action: 'historyShow',
			actionId: 'terminal.history.show',
		});
		expect(resolveProjectedTerminalShortcut(event, 'apple', 'viewer')).toBeNull();
	});

	it('resolves command output only while the terminal is focused', () => {
		const event = {
			altKey: false,
			code: 'KeyG',
			ctrlKey: true,
			isComposing: false,
			metaKey: false,
			repeat: false,
			shiftKey: true,
		};

		expect(resolveProjectedTerminalShortcut(event, 'apple', 'terminal')).toMatchObject({
			action: 'outputShowLast',
			actionId: 'terminal.output.show-last',
		});
		expect(resolveProjectedTerminalShortcut(event, 'apple', 'viewer')).toBeNull();
	});

	it('resolves terminal hints only while the terminal is focused', () => {
		const event = {
			altKey: false,
			code: 'KeyO',
			ctrlKey: false,
			isComposing: false,
			metaKey: true,
			repeat: false,
			shiftKey: true,
		};

		expect(resolveProjectedTerminalShortcut(event, 'apple', 'terminal')).toMatchObject({
			action: 'hintsOpen',
			actionId: 'terminal.hints.open',
		});
		expect(resolveProjectedTerminalShortcut(event, 'apple', 'find')).toBeNull();
	});

	it.each([
		['ArrowUp', 'scrollLineUp', 'terminal.history.scroll-line-up'],
		['ArrowDown', 'scrollLineDown', 'terminal.history.scroll-line-down'],
		['PageUp', 'scrollPageUp', 'terminal.history.scroll-page-up'],
		['PageDown', 'scrollPageDown', 'terminal.history.scroll-page-down'],
		['Home', 'scrollToTop', 'terminal.history.scroll-top'],
		['End', 'scrollToBottom', 'terminal.history.scroll-bottom'],
	] as const)('resolves %s history scrolling', (code, action, actionId) => {
		expect(
			resolveProjectedTerminalShortcut(
				{
					altKey: false,
					code,
					ctrlKey: true,
					isComposing: false,
					metaKey: false,
					repeat: false,
					shiftKey: true,
				},
				'apple',
				'terminal',
			),
		).toMatchObject({ action, actionId });
	});

	it('resolves prompt navigation only for terminal focus', () => {
		expect(
			resolveProjectedTerminalShortcut(
				{
					altKey: false,
					code: 'KeyZ',
					ctrlKey: true,
					isComposing: false,
					metaKey: false,
					repeat: false,
					shiftKey: true,
				},
				'apple',
				'terminal',
			),
		).toMatchObject({ action: 'promptPrevious', actionId: 'terminal.history.prompt-previous' });
		expect(
			resolveProjectedTerminalShortcut(
				{
					altKey: false,
					code: 'KeyX',
					ctrlKey: true,
					isComposing: false,
					metaKey: false,
					repeat: false,
					shiftKey: true,
				},
				'standard',
				'find',
			),
		).toBeNull();
	});

	it.each(['KeyF', 'KeyL'])('does not intercept unshifted Standard Control+%s', (code) => {
		expect(
			resolveProjectedTerminalShortcut(
				{
					altKey: false,
					code,
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

	it('marks only the platform-default Paste chord for native event fallback', () => {
		expect(
			resolveProjectedTerminalShortcut(
				{
					altKey: false,
					code: 'KeyV',
					ctrlKey: false,
					isComposing: false,
					metaKey: true,
					repeat: false,
					shiftKey: false,
				},
				'apple',
				'terminal',
			),
		).toMatchObject({ action: 'paste', nativePasteFallback: true });
		expect(
			resolveProjectedTerminalShortcut(
				{
					altKey: false,
					code: 'KeyV',
					ctrlKey: true,
					isComposing: false,
					metaKey: false,
					repeat: false,
					shiftKey: true,
				},
				'standard',
				'terminal',
			),
		).toMatchObject({ action: 'paste', nativePasteFallback: true });

		const custom = {
			...defaultProjectedTerminalShortcutBindings('apple'),
			'terminal.clipboard.paste': {
				code: 'KeyP',
				modifiers: ['mod', 'shift'] as const,
			},
		};
		expect(
			resolveProjectedTerminalShortcut(
				{
					altKey: false,
					code: 'KeyP',
					ctrlKey: false,
					isComposing: false,
					metaKey: true,
					repeat: false,
					shiftKey: true,
				},
				'apple',
				'terminal',
				custom,
			),
		).toMatchObject({ action: 'paste', nativePasteFallback: false });
	});
});
