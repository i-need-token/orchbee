import { describe, expect, it } from 'vitest';

import {
	formatWorkbenchShortcut,
	matchesWorkbenchShortcut,
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
		expect(WORKBENCH_SHORTCUTS.newTerminal.id).toBe('workbench.terminal.create');
		expect(WORKBENCH_SHORTCUTS.search.id).toBe('workbench.search.open');
		expect(WORKBENCH_SHORTCUTS.settings.id).toBe('workbench.settings.open');
		expect(WORKBENCH_SHORTCUTS.toggleSidebar.id).toBe('workbench.toggle-left-panel');
		expect(formatWorkbenchShortcut('newTerminal', 'apple')).toBe('⌘N');
		expect(formatWorkbenchShortcut('search', 'apple')).toBe('⌘K');
		expect(formatWorkbenchShortcut('settings', 'apple')).toBe('⌘,');
		expect(formatWorkbenchShortcut('toggleSidebar', 'apple')).toBe('⌘B');
		expect(formatWorkbenchShortcut('newTerminal', 'standard')).toBe('Ctrl N');
		expect(formatWorkbenchShortcut('search', 'standard')).toBe('Ctrl K');
		expect(formatWorkbenchShortcut('settings', 'standard')).toBe('Ctrl+,');
		expect(formatWorkbenchShortcut('toggleSidebar', 'standard')).toBe('Ctrl B');
	});

	it.each([
		['newTerminal', 'KeyN'],
		['search', 'KeyK'],
		['settings', 'Comma'],
		['toggleSidebar', 'KeyB'],
	] as const)('matches the exact platform primary modifier for %s', (action, code) => {
		expect(matchesWorkbenchShortcut(shortcutEvent({ code, metaKey: true }), 'apple', action)).toBe(
			true,
		);
		expect(
			matchesWorkbenchShortcut(shortcutEvent({ code, ctrlKey: true }), 'standard', action),
		).toBe(true);
		expect(matchesWorkbenchShortcut(shortcutEvent({ code, ctrlKey: true }), 'apple', action)).toBe(
			false,
		);
		expect(
			matchesWorkbenchShortcut(shortcutEvent({ code, metaKey: true }), 'standard', action),
		).toBe(false);
	});

	it('rejects ambiguous, composing, repeated, and unrelated key events', () => {
		for (const event of [
			shortcutEvent({ altKey: true, metaKey: true }),
			shortcutEvent({ ctrlKey: true, metaKey: true }),
			shortcutEvent({ isComposing: true, metaKey: true }),
			shortcutEvent({ metaKey: true, repeat: true }),
			shortcutEvent({ metaKey: true, shiftKey: true }),
			shortcutEvent({ code: 'KeyK', metaKey: true }),
		]) {
			expect(matchesWorkbenchShortcut(event, 'apple', 'toggleSidebar')).toBe(false);
		}
	});
});
