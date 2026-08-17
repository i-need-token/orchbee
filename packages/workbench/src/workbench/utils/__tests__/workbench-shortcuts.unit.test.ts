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
		expect(WORKBENCH_SHORTCUTS.newTerminal.id).toBe('workbench.terminal.create');
		expect(WORKBENCH_SHORTCUTS.search.id).toBe('workbench.search.open');
		expect(WORKBENCH_SHORTCUTS.settings.id).toBe('workbench.settings.open');
		expect(WORKBENCH_SHORTCUTS.toggleSidebar.id).toBe('workbench.sidebar.toggle');
		expect(formatWorkbenchShortcut('actionsSearch', 'apple')).toBe('⌘⇧P');
		expect(formatWorkbenchShortcut('newTerminal', 'apple')).toBe('⌘N');
		expect(formatWorkbenchShortcut('search', 'apple')).toBe('⌘K');
		expect(formatWorkbenchShortcut('settings', 'apple')).toBe('⌘,');
		expect(formatWorkbenchShortcut('toggleSidebar', 'apple')).toBe('⌘B');
		expect(formatWorkbenchShortcut('actionsSearch', 'standard')).toBe('Ctrl+Shift+P');
		expect(formatWorkbenchShortcut('newTerminal', 'standard')).toBe('Ctrl+Shift+N');
		expect(formatWorkbenchShortcut('search', 'standard')).toBe('Ctrl+Shift+K');
		expect(formatWorkbenchShortcut('settings', 'standard')).toBe('Ctrl+,');
		expect(formatWorkbenchShortcut('toggleSidebar', 'standard')).toBe('Ctrl+Shift+B');
		expect(formatWorkbenchActionShortcut('workbench.search.open', 'apple')).toBe('⌘K');
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
		['actionsSearch', 'KeyP'],
		['newTerminal', 'KeyN'],
		['search', 'KeyK'],
		['settings', 'Comma'],
		['toggleSidebar', 'KeyB'],
	] as const)('resolves the exact platform primary modifier for %s', (action, code) => {
		const appleModifiers = WORKBENCH_SHORTCUTS[action].defaults.apple.modifiers;
		const standardModifiers = WORKBENCH_SHORTCUTS[action].defaults.standard.modifiers;
		expect(
			resolveWorkbenchShortcut(
				shortcutEvent({
					code,
					metaKey: true,
					shiftKey: appleModifiers.includes('shift'),
				}),
				'apple',
			),
		).toMatchObject({ action, shortcutId: WORKBENCH_SHORTCUTS[action].shortcutId });
		expect(
			resolveWorkbenchShortcut(
				shortcutEvent({
					code,
					ctrlKey: true,
					shiftKey: standardModifiers.includes('shift'),
				}),
				'standard',
			),
		).toMatchObject({ action, shortcutId: WORKBENCH_SHORTCUTS[action].shortcutId });
		expect(resolveWorkbenchShortcut(shortcutEvent({ code, ctrlKey: true }), 'apple')).toBe(null);
		expect(resolveWorkbenchShortcut(shortcutEvent({ code, metaKey: true }), 'standard')).toBe(null);
	});

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

	it('resolves the enumerable binding table to stable shortcut identities', () => {
		expect(
			resolveWorkbenchShortcut(shortcutEvent({ code: 'KeyN', metaKey: true }), 'apple'),
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
