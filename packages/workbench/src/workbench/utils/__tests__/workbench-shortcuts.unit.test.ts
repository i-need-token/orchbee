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
		expect(WORKBENCH_SHORTCUTS.focusActivePanelDown.id).toBe('workbench.panel.active.focus-down');
		expect(WORKBENCH_SHORTCUTS.focusActivePanelLeft.id).toBe('workbench.panel.active.focus-left');
		expect(WORKBENCH_SHORTCUTS.focusActivePanelRight.id).toBe('workbench.panel.active.focus-right');
		expect(WORKBENCH_SHORTCUTS.focusActivePanelUp.id).toBe('workbench.panel.active.focus-up');
		expect(WORKBENCH_SHORTCUTS.maximizeActivePanel.id).toBe(
			'workbench.panel.active.maximization.toggle',
		);
		expect(WORKBENCH_SHORTCUTS.newTerminal.id).toBe('workbench.terminal.create');
		expect(WORKBENCH_SHORTCUTS.search.id).toBe('workbench.search.open');
		expect(WORKBENCH_SHORTCUTS.settings.id).toBe('workbench.settings.open');
		expect(WORKBENCH_SHORTCUTS.splitActivePanelBelow.id).toBe('workbench.panel.active.split-below');
		expect(WORKBENCH_SHORTCUTS.splitActivePanelRight.id).toBe('workbench.panel.active.split-right');
		expect(WORKBENCH_SHORTCUTS.toggleSidebar.id).toBe('workbench.sidebar.toggle');
		expect(formatWorkbenchShortcut('actionsSearch', 'apple')).toBe('⇧⌘P');
		expect(formatWorkbenchShortcut('closeActiveTerminal', 'apple')).toBe('⌘W');
		expect(formatWorkbenchShortcut('focusActivePanelDown', 'apple')).toBe('⌥⌘↓');
		expect(formatWorkbenchShortcut('focusActivePanelLeft', 'apple')).toBe('⌥⌘←');
		expect(formatWorkbenchShortcut('focusActivePanelRight', 'apple')).toBe('⌥⌘→');
		expect(formatWorkbenchShortcut('focusActivePanelUp', 'apple')).toBe('⌥⌘↑');
		expect(formatWorkbenchShortcut('maximizeActivePanel', 'apple')).toBe('⇧⌘Enter');
		expect(formatWorkbenchShortcut('newTerminal', 'apple')).toBe('⌘T');
		expect(formatWorkbenchShortcut('search', 'apple')).toBe('⇧⌘F');
		expect(formatWorkbenchShortcut('settings', 'apple')).toBe('⌘,');
		expect(formatWorkbenchShortcut('splitActivePanelBelow', 'apple')).toBe('⇧⌘D');
		expect(formatWorkbenchShortcut('splitActivePanelRight', 'apple')).toBe('⌘D');
		expect(formatWorkbenchShortcut('toggleSidebar', 'apple')).toBe('⌘B');
		expect(formatWorkbenchShortcut('actionsSearch', 'standard')).toBe('Ctrl+Shift+P');
		expect(formatWorkbenchShortcut('closeActiveTerminal', 'standard')).toBe('Ctrl+Shift+W');
		expect(formatWorkbenchShortcut('focusActivePanelDown', 'standard')).toBe('Ctrl+Alt+↓');
		expect(formatWorkbenchShortcut('focusActivePanelLeft', 'standard')).toBe('Ctrl+Alt+←');
		expect(formatWorkbenchShortcut('focusActivePanelRight', 'standard')).toBe('Ctrl+Alt+→');
		expect(formatWorkbenchShortcut('focusActivePanelUp', 'standard')).toBe('Ctrl+Alt+↑');
		expect(formatWorkbenchShortcut('maximizeActivePanel', 'standard')).toBe('Ctrl+Shift+Enter');
		expect(formatWorkbenchShortcut('newTerminal', 'standard')).toBe('Ctrl+Shift+T');
		expect(formatWorkbenchShortcut('search', 'standard')).toBe('Ctrl+Shift+K');
		expect(formatWorkbenchShortcut('settings', 'standard')).toBe('Ctrl+,');
		expect(formatWorkbenchShortcut('splitActivePanelBelow', 'standard')).toBe('Ctrl+Shift+E');
		expect(formatWorkbenchShortcut('splitActivePanelRight', 'standard')).toBe('Ctrl+Shift+D');
		expect(formatWorkbenchShortcut('toggleSidebar', 'standard')).toBe('Ctrl+Shift+B');
		expect(formatWorkbenchActionShortcut('workbench.search.open', 'apple')).toBe('⇧⌘F');
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
		['focusActivePanelDown', 'ArrowDown', 'ArrowDown'],
		['focusActivePanelLeft', 'ArrowLeft', 'ArrowLeft'],
		['focusActivePanelRight', 'ArrowRight', 'ArrowRight'],
		['focusActivePanelUp', 'ArrowUp', 'ArrowUp'],
		['maximizeActivePanel', 'Enter', 'Enter'],
		['newTerminal', 'KeyT', 'KeyT'],
		['search', 'KeyF', 'KeyK'],
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
