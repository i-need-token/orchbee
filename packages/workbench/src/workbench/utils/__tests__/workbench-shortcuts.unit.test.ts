import { keyboardShortcutChordKey } from '@orchbee/keyboard-shortcuts';
import { describe, expect, it } from 'vitest';

import {
	formatWorkbenchActionShortcut,
	formatWorkbenchShortcut,
	defaultWorkbenchShortcutBindings,
	resolveWorkbenchShortcut,
	WORKBENCH_SHORTCUTS,
} from '../workbench-shortcuts';

interface ShortcutEventOverrides {
	readonly altKey?: boolean;
	readonly code?: string;
	readonly ctrlKey?: boolean;
	readonly isComposing?: boolean;
	readonly metaKey?: boolean;
	readonly repeat?: boolean;
	readonly shiftKey?: boolean;
}

function shortcutEvent(overrides: ShortcutEventOverrides = {}) {
	return {
		altKey: false,
		code: 'KeyB',
		ctrlKey: false,
		isComposing: false,
		metaKey: false,
		repeat: false,
		shiftKey: false,
		...overrides,
	};
}

describe('Workbench shortcuts', () => {
	it('owns stable semantic actions and platform displays', () => {
		expect(WORKBENCH_SHORTCUTS.actionsSearch.id).toBe('workbench.search.open');
		expect(WORKBENCH_SHORTCUTS.closeActiveTerminal.id).toBe('workbench.terminal.active.close');
		expect(WORKBENCH_SHORTCUTS.decreaseTerminalFontSize.id).toBe(
			'workbench.terminal.font-size.decrease',
		);
		expect(WORKBENCH_SHORTCUTS.focusActivePanelDown.id).toBe('workbench.panel.active.focus-down');
		expect(WORKBENCH_SHORTCUTS.focusActivePanelLeft.id).toBe('workbench.panel.active.focus-left');
		expect(WORKBENCH_SHORTCUTS.focusActivePanelRight.id).toBe('workbench.panel.active.focus-right');
		expect(WORKBENCH_SHORTCUTS.focusActivePanelUp.id).toBe('workbench.panel.active.focus-up');
		expect(WORKBENCH_SHORTCUTS.increaseTerminalFontSize.id).toBe(
			'workbench.terminal.font-size.increase',
		);
		expect(WORKBENCH_SHORTCUTS.maximizeActivePanel.id).toBe(
			'workbench.panel.active.maximization.toggle',
		);
		expect(WORKBENCH_SHORTCUTS.newTerminal.id).toBe('workbench.terminal.create');
		expect(WORKBENCH_SHORTCUTS.search.id).toBe('workbench.search.open');
		expect(WORKBENCH_SHORTCUTS.resetTerminalFontSize.id).toBe('workbench.terminal.font-size.reset');
		expect(WORKBENCH_SHORTCUTS.selectNextTab).toMatchObject({
			id: 'workbench.tab.select-next',
			input: undefined,
			shortcutId: 'workbench.tab.select-next',
		});
		expect(WORKBENCH_SHORTCUTS.selectPreviousTab).toMatchObject({
			id: 'workbench.tab.select-previous',
			input: undefined,
			shortcutId: 'workbench.tab.select-previous',
		});
		expect(WORKBENCH_SHORTCUTS.selectTab1).toMatchObject({
			id: 'workbench.tab.select-by-position',
			input: { position: 1 },
			shortcutId: 'workbench.tab.select.1',
		});
		expect(WORKBENCH_SHORTCUTS.selectTab9).toMatchObject({
			id: 'workbench.tab.select-by-position',
			input: { position: 9 },
			shortcutId: 'workbench.tab.select.9',
		});
		expect(WORKBENCH_SHORTCUTS.settings.id).toBe('workbench.settings.open');
		expect(WORKBENCH_SHORTCUTS.splitActivePanelBelow.id).toBe('workbench.panel.active.split-below');
		expect(WORKBENCH_SHORTCUTS.splitActivePanelRight.id).toBe('workbench.panel.active.split-right');
		expect(WORKBENCH_SHORTCUTS.toggleSidebar.id).toBe('workbench.sidebar.toggle');
		expect(formatWorkbenchShortcut('actionsSearch', 'apple')).toBe('⇧⌘P');
		expect(formatWorkbenchShortcut('closeActiveTerminal', 'apple')).toBe('⌘W');
		expect(formatWorkbenchShortcut('decreaseTerminalFontSize', 'apple')).toBe('⌘-');
		expect(formatWorkbenchShortcut('focusActivePanelDown', 'apple')).toBe('⌥⌘↓');
		expect(formatWorkbenchShortcut('focusActivePanelLeft', 'apple')).toBe('⌥⌘←');
		expect(formatWorkbenchShortcut('focusActivePanelRight', 'apple')).toBe('⌥⌘→');
		expect(formatWorkbenchShortcut('focusActivePanelUp', 'apple')).toBe('⌥⌘↑');
		expect(formatWorkbenchShortcut('increaseTerminalFontSize', 'apple')).toBe('⌘=');
		expect(formatWorkbenchShortcut('maximizeActivePanel', 'apple')).toBe('⇧⌘Enter');
		expect(formatWorkbenchShortcut('newTerminal', 'apple')).toBe('⌘T');
		expect(formatWorkbenchShortcut('search', 'apple')).toBe('⇧⌘F');
		expect(formatWorkbenchShortcut('resetTerminalFontSize', 'apple')).toBe('⌘0');
		expect(formatWorkbenchShortcut('selectNextTab', 'apple')).toBe('⇧⌘]');
		expect(formatWorkbenchShortcut('selectPreviousTab', 'apple')).toBe('⇧⌘[');
		expect(formatWorkbenchShortcut('selectTab1', 'apple')).toBe('⌘1');
		expect(formatWorkbenchShortcut('selectTab9', 'apple')).toBe('⌘9');
		expect(formatWorkbenchShortcut('settings', 'apple')).toBe('⌘,');
		expect(formatWorkbenchShortcut('splitActivePanelBelow', 'apple')).toBe('⇧⌘D');
		expect(formatWorkbenchShortcut('splitActivePanelRight', 'apple')).toBe('⌘D');
		expect(formatWorkbenchShortcut('toggleSidebar', 'apple')).toBe('⌘B');
		expect(formatWorkbenchShortcut('actionsSearch', 'standard')).toBe('Ctrl+Shift+P');
		expect(formatWorkbenchShortcut('closeActiveTerminal', 'standard')).toBe('Ctrl+Shift+W');
		expect(formatWorkbenchShortcut('decreaseTerminalFontSize', 'standard')).toBe('Ctrl+-');
		expect(formatWorkbenchShortcut('focusActivePanelDown', 'standard')).toBe('Ctrl+Alt+↓');
		expect(formatWorkbenchShortcut('focusActivePanelLeft', 'standard')).toBe('Ctrl+Alt+←');
		expect(formatWorkbenchShortcut('focusActivePanelRight', 'standard')).toBe('Ctrl+Alt+→');
		expect(formatWorkbenchShortcut('focusActivePanelUp', 'standard')).toBe('Ctrl+Alt+↑');
		expect(formatWorkbenchShortcut('increaseTerminalFontSize', 'standard')).toBe('Ctrl+=');
		expect(formatWorkbenchShortcut('maximizeActivePanel', 'standard')).toBe('Ctrl+Shift+Enter');
		expect(formatWorkbenchShortcut('newTerminal', 'standard')).toBe('Ctrl+Shift+T');
		expect(formatWorkbenchShortcut('search', 'standard')).toBe('Ctrl+Shift+K');
		expect(formatWorkbenchShortcut('resetTerminalFontSize', 'standard')).toBe('Ctrl+0');
		expect(formatWorkbenchShortcut('selectNextTab', 'standard')).toBe('Ctrl+Page Down');
		expect(formatWorkbenchShortcut('selectPreviousTab', 'standard')).toBe('Ctrl+Page Up');
		expect(formatWorkbenchShortcut('selectTab1', 'standard')).toBe('Ctrl+1');
		expect(formatWorkbenchShortcut('selectTab9', 'standard')).toBe('Ctrl+9');
		expect(formatWorkbenchShortcut('settings', 'standard')).toBe('Ctrl+,');
		expect(formatWorkbenchShortcut('splitActivePanelBelow', 'standard')).toBe('Ctrl+Shift+E');
		expect(formatWorkbenchShortcut('splitActivePanelRight', 'standard')).toBe('Ctrl+Shift+D');
		expect(formatWorkbenchShortcut('toggleSidebar', 'standard')).toBe('Ctrl+Shift+B');
		expect(formatWorkbenchActionShortcut('workbench.search.open', 'apple')).toBe('⇧⌘F');
		expect(formatWorkbenchActionShortcut('workbench.terminal.font-size.increase', 'apple')).toBe(
			'⌘=',
		);
		expect(formatWorkbenchActionShortcut('workbench.tab.select-next', 'apple')).toBe('⇧⌘]');
		expect(formatWorkbenchActionShortcut('workbench.tab.select-previous', 'apple')).toBe('⇧⌘[');
		expect(formatWorkbenchActionShortcut('unknown.action', 'apple')).toBe(null);
		expect(
			new Set(
				Object.values(WORKBENCH_SHORTCUTS).map((definition) =>
					keyboardShortcutChordKey(definition.defaults.apple, 'apple'),
				),
			).size,
		).toBe(Object.keys(WORKBENCH_SHORTCUTS).length);
	});

	it.each([
		['actionsSearch', 'KeyP', 'KeyP'],
		['closeActiveTerminal', 'KeyW', 'KeyW'],
		['decreaseTerminalFontSize', 'Minus', 'Minus'],
		['focusActivePanelDown', 'ArrowDown', 'ArrowDown'],
		['focusActivePanelLeft', 'ArrowLeft', 'ArrowLeft'],
		['focusActivePanelRight', 'ArrowRight', 'ArrowRight'],
		['focusActivePanelUp', 'ArrowUp', 'ArrowUp'],
		['increaseTerminalFontSize', 'Equal', 'Equal'],
		['maximizeActivePanel', 'Enter', 'Enter'],
		['newTerminal', 'KeyT', 'KeyT'],
		['search', 'KeyF', 'KeyK'],
		['resetTerminalFontSize', 'Digit0', 'Digit0'],
		['selectNextTab', 'BracketRight', 'PageDown'],
		['selectPreviousTab', 'BracketLeft', 'PageUp'],
		['selectTab1', 'Digit1', 'Digit1'],
		['selectTab2', 'Digit2', 'Digit2'],
		['selectTab3', 'Digit3', 'Digit3'],
		['selectTab4', 'Digit4', 'Digit4'],
		['selectTab5', 'Digit5', 'Digit5'],
		['selectTab6', 'Digit6', 'Digit6'],
		['selectTab7', 'Digit7', 'Digit7'],
		['selectTab8', 'Digit8', 'Digit8'],
		['selectTab9', 'Digit9', 'Digit9'],
		['settings', 'Comma', 'Comma'],
		['splitActivePanelBelow', 'KeyD', 'KeyE'],
		['splitActivePanelRight', 'KeyD', 'KeyD'],
		['toggleSidebar', 'KeyB', 'KeyB'],
	] as const)(
		'resolves the exact platform primary modifier for %s',
		(action, appleCode, standardCode) => {
			const appleModifiers = WORKBENCH_SHORTCUTS[action].defaults.apple.modifiers;
			const standardModifiers = WORKBENCH_SHORTCUTS[action].defaults.standard.modifiers;
			expect(
				resolveWorkbenchShortcut(
					shortcutEvent({
						code: appleCode,
						altKey: appleModifiers.includes('alt'),
						metaKey: true,
						shiftKey: appleModifiers.includes('shift'),
					}),
					'apple',
				),
			).toMatchObject({ action, shortcutId: WORKBENCH_SHORTCUTS[action].shortcutId });
			expect(
				resolveWorkbenchShortcut(
					shortcutEvent({
						code: standardCode,
						altKey: standardModifiers.includes('alt'),
						ctrlKey: true,
						shiftKey: standardModifiers.includes('shift'),
					}),
					'standard',
				),
			).toMatchObject({ action, shortcutId: WORKBENCH_SHORTCUTS[action].shortcutId });
			expect(
				resolveWorkbenchShortcut(shortcutEvent({ code: appleCode, ctrlKey: true }), 'apple'),
			).toBe(null);
			expect(
				resolveWorkbenchShortcut(shortcutEvent({ code: standardCode, metaKey: true }), 'standard'),
			).toBe(null);
		},
	);

	it('rejects ambiguous, composing, repeated, and unrelated key events', () => {
		for (const event of [
			shortcutEvent({ altKey: true, metaKey: true }),
			shortcutEvent({ ctrlKey: true, metaKey: true }),
			shortcutEvent({ isComposing: true, metaKey: true }),
			shortcutEvent({ metaKey: true, repeat: true }),
			shortcutEvent({ metaKey: true, shiftKey: true }),
			shortcutEvent({ code: 'KeyZ', metaKey: true }),
		]) {
			expect(resolveWorkbenchShortcut(event, 'apple')).toBe(null);
		}
	});

	it('leaves Apple Command+K available to the focused terminal scope', () => {
		expect(
			resolveWorkbenchShortcut(shortcutEvent({ code: 'KeyK', metaKey: true }), 'apple'),
		).toBeNull();
	});

	it('resolves the enumerable binding table to stable shortcut identities', () => {
		expect(
			resolveWorkbenchShortcut(shortcutEvent({ code: 'KeyT', metaKey: true }), 'apple'),
		).toEqual(
			expect.objectContaining({
				action: 'newTerminal',
				shortcutId: WORKBENCH_SHORTCUTS.newTerminal.shortcutId,
			}),
		);
		expect(
			resolveWorkbenchShortcut(shortcutEvent({ code: 'Comma', ctrlKey: true }), 'standard'),
		).toEqual(
			expect.objectContaining({
				action: 'settings',
				shortcutId: WORKBENCH_SHORTCUTS.settings.shortcutId,
			}),
		);
		expect(resolveWorkbenchShortcut(shortcutEvent({ code: 'KeyZ', metaKey: true }), 'apple')).toBe(
			null,
		);
	});

	it('formats and resolves a caller-provided binding table without changing semantic identities', () => {
		const defaults = defaultWorkbenchShortcutBindings('standard');
		const bindings = Object.freeze({
			...defaults,
			[WORKBENCH_SHORTCUTS.search.shortcutId]: Object.freeze({
				code: 'KeyG',
				modifiers: Object.freeze(['control', 'shift'] as const),
			}),
		});

		expect(formatWorkbenchShortcut('search', 'standard', bindings)).toBe('Ctrl+Shift+G');
		expect(
			resolveWorkbenchShortcut(
				shortcutEvent({ code: 'KeyG', ctrlKey: true, shiftKey: true }),
				'standard',
				bindings,
			),
		).toMatchObject({ action: 'search', shortcutId: 'workbench.search.all.open' });
		expect(
			resolveWorkbenchShortcut(
				shortcutEvent({ code: 'KeyK', ctrlKey: true, shiftKey: true }),
				'standard',
				bindings,
			),
		).toBe(null);
	});
});
