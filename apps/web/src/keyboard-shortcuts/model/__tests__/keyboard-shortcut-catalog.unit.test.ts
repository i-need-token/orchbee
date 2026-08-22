import { formatKeyboardShortcutChord } from '@orchbee/keyboard-shortcuts';
import { describe, expect, it } from 'vitest';

import {
	KEYBOARD_SHORTCUT_CATALOG,
	KEYBOARD_SHORTCUT_IDS,
	defaultKeyboardShortcutBindings,
	findKeyboardShortcutConflict,
	keyboardShortcutDeclaration,
} from '../keyboard-shortcut-catalog';

describe('keyboard shortcut catalog', () => {
	it('combines the nine Workbench and four terminal owner declarations once', () => {
		expect(KEYBOARD_SHORTCUT_CATALOG).toHaveLength(13);
		expect(new Set(KEYBOARD_SHORTCUT_IDS).size).toBe(13);
		expect(keyboardShortcutDeclaration('workbench.search.actions.open')).toMatchObject({
			actionId: 'workbench.search.open',
			actionInput: { filter: 'actions' },
			scope: 'workbench',
		});
		expect(keyboardShortcutDeclaration('workbench.search.all.open')).toMatchObject({
			actionId: 'workbench.search.open',
			actionInput: { filter: 'all' },
		});
		expect(keyboardShortcutDeclaration('workbench.terminal.active.close')).toMatchObject({
			actionId: 'workbench.terminal.active.close',
			availability: 'active-terminal',
			scope: 'active-terminal',
		});
		expect(keyboardShortcutDeclaration('terminal.clipboard.paste')).toMatchObject({
			actionId: 'terminal.clipboard.paste',
			scope: 'focused-terminal',
		});
	});

	it('provides conflict-free platform defaults with terminal-safe Standard bindings', () => {
		const apple = defaultKeyboardShortcutBindings('apple');
		const standard = defaultKeyboardShortcutBindings('standard');
		expect(findKeyboardShortcutConflict(apple, 'apple')).toBeNull();
		expect(findKeyboardShortcutConflict(standard, 'standard')).toBeNull();
		expect(formatKeyboardShortcutChord(standard['workbench.terminal.create'], 'standard')).toBe(
			'Ctrl+Shift+T',
		);
		expect(formatKeyboardShortcutChord(apple['workbench.terminal.active.close'], 'apple')).toBe(
			'⌘W',
		);
		expect(
			formatKeyboardShortcutChord(standard['workbench.panel.active.split-below'], 'standard'),
		).toBe('Ctrl+Shift+E');
		expect(formatKeyboardShortcutChord(standard['terminal.clipboard.paste'], 'standard')).toBe(
			'Ctrl+Shift+V',
		);
		expect(formatKeyboardShortcutChord(standard['terminal.search.open'], 'standard')).toBe(
			'Ctrl+Shift+F',
		);
		expect(formatKeyboardShortcutChord(standard['workbench.settings.open'], 'standard')).toBe(
			'Ctrl+,',
		);
	});
});
