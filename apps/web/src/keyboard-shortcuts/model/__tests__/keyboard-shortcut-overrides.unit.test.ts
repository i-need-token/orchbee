import { describe, expect, it } from 'vitest';

import {
	createKeyboardShortcutBindings,
	keyboardShortcutOverridesEqual,
	replaceKeyboardShortcutOverride,
} from '../keyboard-shortcut-overrides';

describe('keyboard shortcut overrides', () => {
	it('replaces and removes one override in stable shortcut-ID order', () => {
		const terminalOverride = {
			chord: { code: 'KeyH', modifiers: ['mod', 'shift'] as const },
			shortcutId: 'terminal.search.open' as const,
		};
		const withWorkbench = replaceKeyboardShortcutOverride(
			[terminalOverride],
			'workbench.search.all.open',
			{ code: 'KeyG', modifiers: ['mod', 'shift'] },
		);
		expect(withWorkbench.map((override) => override.shortcutId)).toEqual([
			'terminal.search.open',
			'workbench.search.all.open',
		]);
		expect(
			createKeyboardShortcutBindings('standard', withWorkbench)['workbench.search.all.open'],
		).toEqual({ code: 'KeyG', modifiers: ['mod', 'shift'] });
		expect(
			replaceKeyboardShortcutOverride(withWorkbench, 'terminal.search.open', null).map(
				(override) => override.shortcutId,
			),
		).toEqual(['workbench.search.all.open']);
	});

	it('compares normalized effective override identity', () => {
		expect(
			keyboardShortcutOverridesEqual(
				[
					{
						chord: { code: 'KeyG', modifiers: ['mod', 'shift'] },
						shortcutId: 'workbench.search.all.open',
					},
				],
				[
					{
						chord: { code: 'KeyG', modifiers: ['control', 'shift'] },
						shortcutId: 'workbench.search.all.open',
					},
				],
				'standard',
			),
		).toBe(true);
	});
});
