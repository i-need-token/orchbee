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
	it('combines the thirty-five Workbench and sixteen projected-terminal declarations once', () => {
		expect(KEYBOARD_SHORTCUT_CATALOG).toHaveLength(51);
		expect(new Set(KEYBOARD_SHORTCUT_IDS).size).toBe(51);
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
		expect(keyboardShortcutDeclaration('workbench.panel.active.focus-left')).toMatchObject({
			actionId: 'workbench.panel.active.focus-left',
			availability: 'directional-panel',
			name: 'focus-panel-left',
			scope: 'active-panel',
		});
		expect(keyboardShortcutDeclaration('workbench.panel.active.resize')).toMatchObject({
			actionId: 'workbench.panel.active.resize',
			availability: 'active-panel',
			name: 'resize-panel',
			scope: 'active-panel',
		});
		expect(keyboardShortcutDeclaration('workbench.panel.active.move-left')).toMatchObject({
			actionId: 'workbench.panel.active.move-left',
			availability: 'directional-panel',
			name: 'move-panel-left',
			scope: 'active-panel',
		});
		expect(keyboardShortcutDeclaration('workbench.tab.select.2')).toMatchObject({
			actionId: 'workbench.tab.select-by-position',
			actionInput: { position: 2 },
			availability: 'tab-position',
			name: 'select-tab',
			position: 2,
			scope: 'workbench',
		});
		expect(keyboardShortcutDeclaration('workbench.tab.select-next')).toMatchObject({
			actionId: 'workbench.tab.select-next',
			actionInput: undefined,
			availability: 'multiple-tabs',
			name: 'next-tab',
			scope: 'workbench',
		});
		expect(keyboardShortcutDeclaration('workbench.tab.select-previous')).toMatchObject({
			actionId: 'workbench.tab.select-previous',
			actionInput: undefined,
			availability: 'multiple-tabs',
			name: 'previous-tab',
			scope: 'workbench',
		});
		expect(keyboardShortcutDeclaration('workbench.tab.active.move-backward')).toMatchObject({
			actionId: 'workbench.tab.active.move-backward',
			actionInput: undefined,
			availability: 'movable-tab',
			name: 'move-tab-backward',
			scope: 'workbench',
		});
		expect(keyboardShortcutDeclaration('workbench.tab.active.move-forward')).toMatchObject({
			actionId: 'workbench.tab.active.move-forward',
			actionInput: undefined,
			availability: 'movable-tab',
			name: 'move-tab-forward',
			scope: 'workbench',
		});
		expect(keyboardShortcutDeclaration('workbench.tab.title.set')).toMatchObject({
			actionId: 'workbench.tab.title.set',
			actionInput: undefined,
			availability: 'active-terminal',
			name: 'set-tab-title',
			scope: 'workbench',
		});
		expect(keyboardShortcutDeclaration('workbench.terminal.font-size.increase')).toMatchObject({
			actionId: 'workbench.terminal.font-size.increase',
			availability: 'always-in-workbench',
			name: 'increase-terminal-font-size',
			scope: 'workbench',
		});
		expect(keyboardShortcutDeclaration('terminal.clipboard.paste')).toMatchObject({
			actionId: 'terminal.clipboard.paste',
			scope: 'focused-terminal',
		});
		expect(keyboardShortcutDeclaration('terminal.clear')).toMatchObject({
			actionId: 'terminal.clear',
			availability: 'focused-terminal',
			scope: 'focused-terminal',
		});
		expect(keyboardShortcutDeclaration('terminal.hints.open')).toMatchObject({
			actionId: 'terminal.hints.open',
			availability: 'focused-terminal',
			name: 'hints',
			scope: 'focused-terminal',
		});
		expect(keyboardShortcutDeclaration('terminal.output.show-last')).toMatchObject({
			actionId: 'terminal.output.show-last',
			availability: 'focused-terminal',
			name: 'show-last-command-output',
			scope: 'focused-terminal',
		});
		expect(keyboardShortcutDeclaration('terminal.history.show')).toMatchObject({
			actionId: 'terminal.history.show',
			availability: 'focused-terminal',
			name: 'show-scrollback',
			scope: 'focused-terminal',
		});
		expect(keyboardShortcutDeclaration('terminal.history.prompt-previous')).toMatchObject({
			actionId: 'terminal.history.prompt-previous',
			availability: 'focused-terminal',
			name: 'previous-prompt',
			scope: 'focused-terminal',
		});
		expect(keyboardShortcutDeclaration('terminal.history.scroll-page-up')).toMatchObject({
			actionId: 'terminal.history.scroll-page-up',
			availability: 'focused-terminal',
			name: 'scroll-page-up',
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
		expect(formatKeyboardShortcutChord(apple['workbench.panel.active.focus-left'], 'apple')).toBe(
			'⌥⌘←',
		);
		expect(
			formatKeyboardShortcutChord(standard['workbench.panel.active.focus-right'], 'standard'),
		).toBe('Ctrl+Alt+→');
		expect(formatKeyboardShortcutChord(apple['workbench.panel.active.move-left'], 'apple')).toBe(
			'⌥⇧⌘←',
		);
		expect(
			formatKeyboardShortcutChord(standard['workbench.panel.active.move-right'], 'standard'),
		).toBe('Ctrl+Alt+Shift+→');
		expect(
			formatKeyboardShortcutChord(standard['workbench.panel.active.split-below'], 'standard'),
		).toBe('Ctrl+Shift+E');
		expect(formatKeyboardShortcutChord(standard['terminal.clipboard.paste'], 'standard')).toBe(
			'Ctrl+Shift+V',
		);
		expect(formatKeyboardShortcutChord(standard['terminal.search.open'], 'standard')).toBe(
			'Ctrl+Shift+F',
		);
		expect(formatKeyboardShortcutChord(apple['terminal.clear'], 'apple')).toBe('⌘K');
		expect(formatKeyboardShortcutChord(apple['terminal.hints.open'], 'apple')).toBe('⇧⌘O');
		expect(formatKeyboardShortcutChord(apple['terminal.output.show-last'], 'apple')).toBe('⌃⇧G');
		expect(formatKeyboardShortcutChord(apple['terminal.history.show'], 'apple')).toBe('⌃⇧H');
		expect(
			formatKeyboardShortcutChord(apple['workbench.terminal.font-size.increase'], 'apple'),
		).toBe('⌘=');
		expect(
			formatKeyboardShortcutChord(standard['workbench.terminal.font-size.decrease'], 'standard'),
		).toBe('Ctrl+-');
		expect(
			formatKeyboardShortcutChord(standard['workbench.terminal.font-size.reset'], 'standard'),
		).toBe('Ctrl+0');
		expect(formatKeyboardShortcutChord(apple['workbench.tab.select.2'], 'apple')).toBe('⌘2');
		expect(formatKeyboardShortcutChord(apple['workbench.tab.select-next'], 'apple')).toBe('⇧⌘]');
		expect(formatKeyboardShortcutChord(apple['workbench.tab.select-previous'], 'apple')).toBe(
			'⇧⌘[',
		);
		expect(formatKeyboardShortcutChord(apple['workbench.tab.active.move-backward'], 'apple')).toBe(
			'⌃⇧,',
		);
		expect(formatKeyboardShortcutChord(apple['workbench.tab.active.move-forward'], 'apple')).toBe(
			'⌃⇧.',
		);
		expect(formatKeyboardShortcutChord(standard['workbench.tab.select-next'], 'standard')).toBe(
			'Ctrl+Page Down',
		);
		expect(formatKeyboardShortcutChord(standard['workbench.tab.select-previous'], 'standard')).toBe(
			'Ctrl+Page Up',
		);
		expect(formatKeyboardShortcutChord(standard['workbench.tab.select.9'], 'standard')).toBe(
			'Ctrl+9',
		);
		expect(formatKeyboardShortcutChord(standard['terminal.clear'], 'standard')).toBe(
			'Ctrl+Shift+L',
		);
		expect(formatKeyboardShortcutChord(apple['terminal.history.prompt-previous'], 'apple')).toBe(
			'⌃⇧Z',
		);
		expect(formatKeyboardShortcutChord(standard['terminal.history.prompt-next'], 'standard')).toBe(
			'Ctrl+Shift+X',
		);
		expect(formatKeyboardShortcutChord(apple['terminal.history.scroll-line-up'], 'apple')).toBe(
			'⌃⇧↑',
		);
		expect(
			formatKeyboardShortcutChord(standard['terminal.history.scroll-page-down'], 'standard'),
		).toBe('Ctrl+Shift+Page Down');
		expect(formatKeyboardShortcutChord(standard['workbench.settings.open'], 'standard')).toBe(
			'Ctrl+,',
		);
	});
});
