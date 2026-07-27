import { describe, expect, it } from 'vitest';

import {
	formatWorkbenchActionShortcut,
	formatWorkbenchShortcut,
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
		expect(formatWorkbenchShortcut('actionsSearch', 'standard')).toBe('Ctrl Shift P');
		expect(formatWorkbenchShortcut('newTerminal', 'standard')).toBe('Ctrl N');
		expect(formatWorkbenchShortcut('search', 'standard')).toBe('Ctrl K');
		expect(formatWorkbenchShortcut('settings', 'standard')).toBe('Ctrl+,');
		expect(formatWorkbenchShortcut('toggleSidebar', 'standard')).toBe('Ctrl B');
		expect(formatWorkbenchActionShortcut('workbench.search.open', 'apple')).toBe('⌘K');
		expect(formatWorkbenchActionShortcut('unknown.action', 'apple')).toBe(null);
		expect(
			new Set(
				Object.values(WORKBENCH_SHORTCUTS).map((binding) => `${binding.code}:${binding.shiftKey}`),
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
		const shiftKey = WORKBENCH_SHORTCUTS[action].shiftKey;
		expect(
			resolveWorkbenchShortcut(shortcutEvent({ code, metaKey: true, shiftKey }), 'apple'),
		).toBe(WORKBENCH_SHORTCUTS[action]);
		expect(
			resolveWorkbenchShortcut(shortcutEvent({ code, ctrlKey: true, shiftKey }), 'standard'),
		).toBe(WORKBENCH_SHORTCUTS[action]);
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

	it('resolves the enumerable binding table directly to Action IDs', () => {
		expect(resolveWorkbenchShortcut(shortcutEvent({ code: 'KeyN', metaKey: true }), 'apple')).toBe(
			WORKBENCH_SHORTCUTS.newTerminal,
		);
		expect(
			resolveWorkbenchShortcut(shortcutEvent({ code: 'Comma', ctrlKey: true }), 'standard'),
		).toBe(WORKBENCH_SHORTCUTS.settings);
		expect(resolveWorkbenchShortcut(shortcutEvent({ code: 'KeyZ', metaKey: true }), 'apple')).toBe(
			null,
		);
	});
});
