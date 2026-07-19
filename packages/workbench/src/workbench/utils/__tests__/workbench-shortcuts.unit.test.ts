import { describe, expect, it } from 'vitest';

import {
	formatWorkbenchToggleSidebarShortcut,
	matchesWorkbenchToggleSidebarShortcut,
	WORKBENCH_TOGGLE_SIDEBAR_SHORTCUT_ID,
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
	it('owns one stable sidebar action and platform display', () => {
		expect(WORKBENCH_TOGGLE_SIDEBAR_SHORTCUT_ID).toBe('workbench.toggle-left-panel');
		expect(formatWorkbenchToggleSidebarShortcut('apple')).toBe('⌘ B');
		expect(formatWorkbenchToggleSidebarShortcut('standard')).toBe('Ctrl B');
	});

	it('matches the exact platform primary modifier', () => {
		expect(matchesWorkbenchToggleSidebarShortcut(shortcutEvent({ metaKey: true }), 'apple')).toBe(
			true,
		);
		expect(
			matchesWorkbenchToggleSidebarShortcut(shortcutEvent({ ctrlKey: true }), 'standard'),
		).toBe(true);
		expect(matchesWorkbenchToggleSidebarShortcut(shortcutEvent({ ctrlKey: true }), 'apple')).toBe(
			false,
		);
		expect(
			matchesWorkbenchToggleSidebarShortcut(shortcutEvent({ metaKey: true }), 'standard'),
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
			expect(matchesWorkbenchToggleSidebarShortcut(event, 'apple')).toBe(false);
		}
	});
});
