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
	it('combines the five Workbench and three terminal owner declarations once', () => {
		expect(KEYBOARD_SHORTCUT_CATALOG).toHaveLength(8);
		expect(new Set(KEYBOARD_SHORTCUT_IDS).size).toBe(8);
		expect(keyboardShortcutDeclaration('workbench.search.actions.open')).toMatchObject({
			actionId: 'workbench.search.open',
			actionInput: { filter: 'actions' },
			scope: 'workbench',
		});
		expect(keyboardShortcutDeclaration('workbench.search.all.open')).toMatchObject({
			actionId: 'workbench.search.open',
			actionInput: { filter: 'all' },
		});
	});

	it('provides conflict-free platform defaults with terminal-safe Standard bindings', () => {
		const apple = defaultKeyboardShortcutBindings('apple');
		const standard = defaultKeyboardShortcutBindings('standard');
		expect(findKeyboardShortcutConflict(apple, 'apple')).toBeNull();
		expect(findKeyboardShortcutConflict(standard, 'standard')).toBeNull();
		expect(formatKeyboardShortcutChord(standard['workbench.terminal.create'], 'standard')).toBe(
			'Ctrl+Shift+N',
		);
		expect(formatKeyboardShortcutChord(standard['terminal.search.open'], 'standard')).toBe(
			'Ctrl+Shift+F',
		);
		expect(formatKeyboardShortcutChord(standard['workbench.settings.open'], 'standard')).toBe(
			'Ctrl+,',
		);
	});
});
