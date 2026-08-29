import { describe, expect, it } from 'vitest';

import {
	INITIAL_KEYBOARD_SHORTCUT_SEARCH_STATE,
	resolveKeyboardShortcutSearch,
	transitionKeyboardShortcutSearch,
	type KeyboardShortcutSearchEntry,
} from '../keyboard-shortcut-search';

const entries: readonly KeyboardShortcutSearchEntry[] = Object.freeze([
	Object.freeze({
		actionId: 'workbench.search.actions.open',
		actionName: 'Search actions',
		availability: 'While the Workbench is active',
		chord: Object.freeze({ code: 'KeyP', modifiers: Object.freeze(['mod', 'shift'] as const) }),
		scope: 'Workbench',
		shortcutId: 'workbench.search.actions.open',
		source: 'Default',
	}),
	Object.freeze({
		actionId: 'workbench.terminal.create-current-directory',
		actionName: 'New terminal in current directory',
		availability: 'When a terminal tab is active',
		chord: Object.freeze({ code: 'Enter', modifiers: Object.freeze(['mod', 'alt'] as const) }),
		scope: 'Active terminal',
		shortcutId: 'workbench.terminal.create-current-directory',
		source: 'Custom',
	}),
	Object.freeze({
		actionId: 'workbench.panel.active.focus-left',
		actionName: 'Focus panel left',
		availability: 'When an adjacent panel exists in that direction',
		chord: Object.freeze({ code: 'ArrowLeft', modifiers: Object.freeze(['mod', 'alt'] as const) }),
		scope: 'Active panel',
		shortcutId: 'workbench.panel.active.focus-left',
		source: 'Default',
	}),
	Object.freeze({
		actionId: 'workbench.panel.active.equalize',
		actionName: 'Equalize panels',
		availability: 'When the active tab has multiple panels',
		chord: Object.freeze({ code: 'Equal', modifiers: Object.freeze(['mod', 'alt'] as const) }),
		scope: 'Active panel',
		shortcutId: 'workbench.panel.active.equalize',
		source: 'Default',
	}),
	Object.freeze({
		actionId: 'terminal.history.scroll-page-up',
		actionName: 'Scroll up one page',
		availability: 'When a terminal is focused',
		chord: Object.freeze({ code: 'PageUp', modifiers: Object.freeze(['mod', 'shift'] as const) }),
		scope: 'Focused terminal',
		shortcutId: 'terminal.history.scroll-page-up',
		source: 'Default',
	}),
]);

describe('keyboard shortcut search', () => {
	it('keeps catalog order for empty and conjunctive localized text searches', () => {
		expect(resolve('', 'standard').matchingShortcutIds).toEqual(
			entries.map((entry) => entry.shortcutId),
		);
		expect(resolve('terminal current', 'standard').matchingShortcutIds).toEqual([
			'workbench.terminal.create-current-directory',
		]);
		expect(resolve('active custom', 'standard').matchingShortcutIds).toEqual([
			'workbench.terminal.create-current-directory',
		]);
		expect(resolve('panel missing', 'standard').matchingShortcutIds).toEqual([]);
	});

	it('searches typed modifier aliases as exact modifier sets', () => {
		expect(resolve('ctrl+shift+p', 'standard').matchingShortcutIds).toEqual([
			'workbench.search.actions.open',
		]);
		expect(resolve('ctrl+p', 'standard').matchingShortcutIds).toEqual([]);
		expect(resolve('ctrl+alt+left', 'standard').matchingShortcutIds).toEqual([
			'workbench.panel.active.focus-left',
		]);
		expect(resolve('ctrl+alt+=', 'standard').matchingShortcutIds).toEqual([
			'workbench.panel.active.equalize',
		]);
	});

	it('searches Apple names and glyphs against the same physical chord', () => {
		expect(resolve('cmd shift p', 'apple').matchingShortcutIds).toEqual([
			'workbench.search.actions.open',
		]);
		expect(resolve('⇧⌘P', 'apple').matchingShortcutIds).toEqual(['workbench.search.actions.open']);
		expect(resolve('option command enter', 'apple').matchingShortcutIds).toEqual([
			'workbench.terminal.create-current-directory',
		]);
	});

	it('resolves a captured chord by canonical identity only', () => {
		const result = resolveKeyboardShortcutSearch(
			entries,
			{ kind: 'chord', chord: { code: 'KeyP', modifiers: ['shift', 'control'] } },
			'standard',
		);
		expect(result.matchingShortcutIds).toEqual(['workbench.search.actions.open']);
	});
});

describe('keyboard shortcut search transitions', () => {
	it('records, rejects, clears, and exits without creating an editing command', () => {
		const recording = transitionKeyboardShortcutSearch(
			INITIAL_KEYBOARD_SHORTCUT_SEARCH_STATE,
			{ type: 'recording-started' },
			'standard',
		);
		expect(recording).toMatchObject({ mode: 'recording', recordingIssue: null });

		const rejected = transitionKeyboardShortcutSearch(
			recording,
			{ type: 'shortcut-rejected' },
			'standard',
		);
		expect(rejected).toMatchObject({
			mode: 'recording',
			query: { kind: 'text', value: '' },
			recordingIssue: 'invalid-chord',
		});

		const captured = transitionKeyboardShortcutSearch(
			rejected,
			{ type: 'shortcut-captured', chord: { code: 'KeyP', modifiers: ['control', 'shift'] } },
			'standard',
		);
		expect(captured).toMatchObject({
			mode: 'recording',
			query: { chord: { code: 'KeyP', modifiers: ['mod', 'shift'] }, kind: 'chord' },
			recordingIssue: null,
		});

		const stopped = transitionKeyboardShortcutSearch(
			captured,
			{ type: 'recording-stopped' },
			'standard',
		);
		expect(stopped).toMatchObject({ mode: 'text', query: captured.query });

		const cleared = transitionKeyboardShortcutSearch(stopped, { type: 'cleared' }, 'standard');
		expect(cleared).toEqual(INITIAL_KEYBOARD_SHORTCUT_SEARCH_STATE);
	});

	it('ignores text changes while recording', () => {
		const recording = transitionKeyboardShortcutSearch(
			INITIAL_KEYBOARD_SHORTCUT_SEARCH_STATE,
			{ type: 'recording-started' },
			'apple',
		);
		expect(
			transitionKeyboardShortcutSearch(
				recording,
				{ type: 'text-changed', value: 'Search actions' },
				'apple',
			),
		).toBe(recording);
	});
});

function resolve(value: string, platform: 'apple' | 'standard') {
	return resolveKeyboardShortcutSearch(entries, { kind: 'text', value }, platform);
}
