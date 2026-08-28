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

	it('applies a custom directional focus binding without changing the other directions', () => {
		const bindings = createKeyboardShortcutBindings('standard', [
			{
				chord: { code: 'KeyH', modifiers: ['alt', 'mod'] },
				shortcutId: 'workbench.panel.active.focus-left',
			},
		]);

		expect(bindings['workbench.panel.active.focus-left']).toEqual({
			code: 'KeyH',
			modifiers: ['alt', 'mod'],
		});
		expect(bindings['workbench.panel.active.focus-right']).toEqual({
			code: 'ArrowRight',
			modifiers: ['alt', 'mod'],
		});
	});

	it('applies a custom directional Panel movement binding without changing the other directions', () => {
		const bindings = createKeyboardShortcutBindings('standard', [
			{
				chord: { code: 'KeyH', modifiers: ['alt', 'mod', 'shift'] },
				shortcutId: 'workbench.panel.active.move-left',
			},
		]);

		expect(bindings['workbench.panel.active.move-left']).toEqual({
			code: 'KeyH',
			modifiers: ['alt', 'mod', 'shift'],
		});
		expect(bindings['workbench.panel.active.move-right']).toEqual({
			code: 'ArrowRight',
			modifiers: ['mod', 'alt', 'shift'],
		});
	});

	it('applies a custom Panel equalization binding without changing its platform default elsewhere', () => {
		const bindings = createKeyboardShortcutBindings('apple', [
			{
				chord: { code: 'KeyE', modifiers: ['control', 'mod'] },
				shortcutId: 'workbench.panel.active.equalize',
			},
		]);

		expect(bindings['workbench.panel.active.equalize']).toEqual({
			code: 'KeyE',
			modifiers: ['control', 'mod'],
		});
		expect(
			createKeyboardShortcutBindings('standard', [])['workbench.panel.active.equalize'],
		).toEqual({
			code: 'Equal',
			modifiers: ['alt', 'mod'],
		});
	});

	it('applies one positional Tab override without changing the other positions', () => {
		const bindings = createKeyboardShortcutBindings('apple', [
			{
				chord: { code: 'Digit2', modifiers: ['alt', 'mod'] },
				shortcutId: 'workbench.tab.select.2',
			},
		]);

		expect(bindings['workbench.tab.select.2']).toEqual({
			code: 'Digit2',
			modifiers: ['alt', 'mod'],
		});
		expect(bindings['workbench.tab.select.3']).toEqual({
			code: 'Digit3',
			modifiers: ['mod'],
		});
	});

	it('applies one adjacent Tab override without changing the opposite direction', () => {
		const bindings = createKeyboardShortcutBindings('standard', [
			{
				chord: { code: 'BracketRight', modifiers: ['alt', 'mod'] },
				shortcutId: 'workbench.tab.select-next',
			},
		]);

		expect(bindings['workbench.tab.select-next']).toEqual({
			code: 'BracketRight',
			modifiers: ['alt', 'mod'],
		});
		expect(bindings['workbench.tab.select-previous']).toEqual({
			code: 'PageUp',
			modifiers: ['mod'],
		});
	});

	it('applies one active Tab movement override without changing the opposite direction', () => {
		const bindings = createKeyboardShortcutBindings('apple', [
			{
				chord: { code: 'KeyM', modifiers: ['alt', 'mod'] },
				shortcutId: 'workbench.tab.active.move-forward',
			},
		]);

		expect(bindings['workbench.tab.active.move-forward']).toEqual({
			code: 'KeyM',
			modifiers: ['alt', 'mod'],
		});
		expect(bindings['workbench.tab.active.move-backward']).toEqual({
			code: 'Comma',
			modifiers: ['control', 'shift'],
		});
	});

	it('applies a custom Tab title binding without changing its platform default elsewhere', () => {
		const bindings = createKeyboardShortcutBindings('apple', [
			{
				chord: { code: 'KeyY', modifiers: ['mod', 'shift'] },
				shortcutId: 'workbench.tab.title.set',
			},
		]);

		expect(bindings['workbench.tab.title.set']).toEqual({
			code: 'KeyY',
			modifiers: ['mod', 'shift'],
		});
		expect(createKeyboardShortcutBindings('standard', [])['workbench.tab.title.set']).toEqual({
			code: 'KeyT',
			modifiers: ['mod', 'alt', 'shift'],
		});
	});
});
