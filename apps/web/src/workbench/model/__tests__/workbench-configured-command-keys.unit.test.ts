import {
	formatKeyboardShortcutChord,
	type KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';
import { describe, expect, it } from 'vitest';

import type { WorkbenchConfiguredCommand } from '../../types/workbench-configured-commands';
import {
	findWorkbenchConfiguredCommandForKey,
	resolveWorkbenchConfiguredCommandKeys,
} from '../workbench-configured-command-keys';

function command(id: string, bindings: readonly string[]): WorkbenchConfiguredCommand {
	return { bindings, description: null, display: null, id };
}

/** The chord the window binds for one TUI key label, as Search shows it. */
function chordLabel(label: string, platform: KeyboardShortcutPlatform = 'standard') {
	const keys = resolveWorkbenchConfiguredCommandKeys([command('probe', [label])], [], platform);
	const chord = keys.chords.get('probe')?.[0];
	return chord === undefined ? null : formatKeyboardShortcutChord(chord, platform);
}

function key(code: string, flags: { alt?: boolean; ctrl?: boolean } = {}) {
	return {
		altKey: flags.alt ?? false,
		code,
		ctrlKey: flags.ctrl ?? false,
		isComposing: false,
		metaKey: false,
		repeat: false,
		shiftKey: false,
	};
}

describe('configured command key labels', () => {
	it('binds the key a TUI label names and refuses what the window cannot bind', () => {
		expect(chordLabel('prefix+g')).toBeNull();
		expect(chordLabel('prefix+ctrl+g')).toBeNull();
		expect(chordLabel('alt+shift+g')).toBe('Alt+Shift+G');
		expect(chordLabel('ctrl+f5')).toBe('Ctrl+F5');
		expect(chordLabel('alt+f24')).toBe('Alt+F24');
		// A key without Ctrl, Alt or Super stays with the terminal.
		expect(chordLabel('f5')).toBeNull();
		expect(chordLabel('shift+f5')).toBeNull();
		// `plus` is the US key that types it, with Shift; `space` is the space bar.
		expect(chordLabel('alt+plus')).toBe('Alt+Shift+=');
		expect(chordLabel('ctrl+alt+space')).toBe('Ctrl+Alt+Space');
		expect(chordLabel('alt+?')).toBe('Alt+Shift+/');
		expect(chordLabel('super+k')).toBe('Super+K');
		// The TUI's Ctrl is the Control key on a Mac, and its Super is Command.
		expect(chordLabel('ctrl+k', 'apple')).toBe('⌃K');
		expect(chordLabel('super+k', 'apple')).toBe('⌘K');
		expect(chordLabel('alt+esc')).toBeNull();
		expect(chordLabel('alt+é')).toBeNull();
		expect(chordLabel('shift+alt+g')).toBeNull();
	});
});

describe('resolveWorkbenchConfiguredCommandKeys', () => {
	it('takes a default chord, yields to a personal one, and notes what the window cannot bind', () => {
		const keys = resolveWorkbenchConfiguredCommandKeys(
			[
				command('took', ['ctrl+alt+f']),
				command('yielded', ['ctrl+alt+v', 'alt+b']),
				command('tui', ['prefix+g']),
				command('later', ['alt+b']),
			],
			[
				{
					action: 'Find',
					chord: { code: 'KeyF', modifiers: ['mod', 'alt'] },
					id: 'find',
					personal: false,
				},
				{
					action: 'Paste',
					chord: { code: 'KeyV', modifiers: ['mod', 'alt'] },
					id: 'paste',
					personal: true,
				},
			],
			'standard',
		);

		expect(keys.notes.get('took')).toEqual({
			action: 'Find',
			kind: 'took',
			shortcut: 'Ctrl+Alt+F',
		});
		expect(keys.heldLabels.has('Ctrl+Alt+F')).toBe(true);
		expect(
			findWorkbenchConfiguredCommandForKey(
				keys,
				key('KeyF', { alt: true, ctrl: true }),
				'standard',
			),
		).toBe('took');

		expect(keys.notes.get('yielded')).toEqual({
			action: 'Paste',
			kind: 'yielded',
			shortcut: 'Ctrl+Alt+V',
		});
		expect(keys.heldLabels.has('Ctrl+Alt+V')).toBe(false);
		expect(
			findWorkbenchConfiguredCommandForKey(
				keys,
				key('KeyV', { alt: true, ctrl: true }),
				'standard',
			),
		).toBeNull();
		expect(findWorkbenchConfiguredCommandForKey(keys, key('KeyB', { alt: true }), 'standard')).toBe(
			'yielded',
		);

		expect(keys.notes.get('tui')).toEqual({ kind: 'tui-only', shortcut: 'prefix+g' });
		expect(keys.chords.get('tui')).toEqual([]);
		// The first command in the list keeps a chord two commands share.
		expect(keys.chords.get('later')).toEqual([]);
	});
});
