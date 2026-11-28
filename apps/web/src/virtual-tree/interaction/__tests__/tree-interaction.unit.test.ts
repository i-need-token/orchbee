import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { TreeFind } from '../../find/TreeFind';
import { TreeViewport } from '../../layout/TreeViewport';
import { VirtualTreeModel } from '../../model/VirtualTreeModel';
import type { VirtualTreeRow } from '../../model/virtual-tree-types';
import type { VirtualTreeOpenEvent } from '../../types/virtual-tree-events';
import { runTreeKeyCommand, selectRange, toggleRow } from '../tree-commands';
import { treeKeyCommands } from '../tree-keys';
import { TreeTypeNavigation } from '../type-navigation';

// The keyboard and pointer rules VS Code's lists and trees follow, on a model and its view. Rows are
// 22 px high in a view 220 px high, so ten rows show at a time.

interface Item {
	readonly key: string;
	readonly children: readonly Item[] | null;
}

interface KeyModifiers {
	readonly altKey?: boolean;
	readonly ctrlKey?: boolean;
	readonly metaKey?: boolean;
	readonly shiftKey?: boolean;
	/** The key's meaning, when it differs from its code, as on the numeric pad. */
	readonly key?: string;
}

function file(key: string): Item {
	return { children: null, key };
}

function folder(key: string, count: number): Item {
	return {
		children: Array.from({ length: count }, (_, index) => file(`${key}/${index + 1}`)),
		key,
	};
}

async function openTree(items: readonly Item[], expanded: readonly string[] = []) {
	const model = new VirtualTreeModel<string, Item>({
		expandedByDefault: (item) => expanded.includes(item.key),
		input: { children: items, key: '' },
		source: {
			children: (item) => item.children ?? [],
			hasChildren: (item) => item.children !== null,
			key: (item) => item.key,
		},
	});
	await model.refresh(null);
	const viewport = new TreeViewport(model);
	// The view follows the model only while something listens, as the tree's components do.
	viewport.subscribe(() => undefined);
	viewport.setSize(300, 220);
	const opened: VirtualTreeOpenEvent<string, Item>[] = [];
	const open = (event: VirtualTreeOpenEvent<string, Item>) => {
		opened.push(event);
	};
	let find: TreeFind<string, Item> | null = null;
	/** Gives the tree a find over its keys' labels, as a tree with type navigation labels has. */
	const startFind = (): TreeFind<string, Item> => {
		const started = new TreeFind(viewport, {
			focusTree: () => undefined,
			label: (row: VirtualTreeRow<string, Item>) => row.key,
			provider: () => null,
		});
		started.connect();
		find = started;
		return started;
	};
	/** Presses a key as the focused tree does, and reports whether the tree used it. */
	const press = (
		code: string,
		modifiers: KeyModifiers = {},
		platform: KeyboardShortcutPlatform = 'standard',
	): boolean => {
		const event = {
			altKey: modifiers.altKey ?? false,
			code,
			ctrlKey: modifiers.ctrlKey ?? false,
			key: modifiers.key ?? code,
			metaKey: modifiers.metaKey ?? false,
			shiftKey: modifiers.shiftKey ?? false,
		};
		return treeKeyCommands(event, platform).some((command) =>
			runTreeKeyCommand(viewport, command, { find, open }),
		);
	};
	const takeOpened = () =>
		opened
			.splice(0)
			.map(({ pinned, preserveFocus, row }) => ({ key: row.key, pinned, preserveFocus }));
	return { model, press, startFind, takeOpened, viewport };
}

function selected(model: VirtualTreeModel<string, Item>): string[] {
	return [...model.selection].sort();
}

describe('tree keys', () => {
	it('moves a page at a time: to the edge of the view first, then a view further, below the sticky rows', async () => {
		const { model, press, viewport } = await openTree(
			[folder('a', 3), folder('b', 40), file('x')],
			['a', 'b'],
		);

		expect(press('PageDown')).toBe(true);
		expect([model.focus, viewport.scrollTop]).toEqual(['b/5', 0]);
		press('PageDown');
		expect([model.focus, viewport.scrollTop, viewport.stickyCount]).toEqual(['b/15', 220, 1]);
		// `b` sticks over the row at the top, so Page Up stops at the first row below it.
		press('PageUp');
		expect([model.focus, viewport.scrollTop]).toEqual(['b/7', 220]);
		press('PageUp');
		expect([model.focus, viewport.scrollTop]).toEqual(['a', 0]);
	});

	it('grows the selection with Shift and an arrow, shrinks it moving back, and previews a sole selected row', async () => {
		const { model, press, takeOpened } = await openTree(
			[folder('a', 3), file('x'), file('y'), file('z')],
			['a'],
		);
		model.setFocus('x');
		model.setAnchor('x');
		model.setSelection(['x']);

		press('ArrowDown', { shiftKey: true });
		press('ArrowDown', { shiftKey: true });
		expect([model.focus, selected(model)]).toEqual(['z', ['x', 'y', 'z']]);
		expect(takeOpened()).toEqual([]);
		press('ArrowUp', { shiftKey: true });
		press('ArrowUp', { shiftKey: true });
		expect([model.focus, model.anchor, selected(model)]).toEqual(['x', 'x', ['x']]);
		expect(takeOpened()).toEqual([{ key: 'x', pinned: false, preserveFocus: true }]);
	});

	it('collapses a row or moves to its parent with Left, and expands one or moves to its first child with Right', async () => {
		const { model, press } = await openTree([folder('a', 3), file('x')], ['a']);
		model.setFocus('a/2');

		press('ArrowLeft');
		expect(model.focus).toBe('a');
		press('ArrowLeft');
		expect(model.rowCount).toBe(2);
		// A collapsed top-level row leaves the key to the page.
		expect(press('ArrowLeft')).toBe(false);
		press('ArrowRight');
		await vi.waitFor(() => {
			expect(model.rowCount).toBe(5);
		});
		press('ArrowRight');
		expect(model.focus).toBe('a/1');
		expect(press('ArrowRight')).toBe(false);
		// The numeric pad's arrows move as the others while Num Lock is off.
		press('Numpad2', { key: 'ArrowDown' });
		expect(model.focus).toBe('a/2');
	});

	it('selects and opens the focused row for good with Enter, and toggles a folder or previews a file with Space', async () => {
		const { model, press, takeOpened } = await openTree([folder('a', 3), file('x')]);
		model.setFocus('x');

		press('Enter');
		expect([model.anchor, selected(model)]).toEqual(['x', ['x']]);
		expect(takeOpened()).toEqual([{ key: 'x', pinned: true, preserveFocus: false }]);
		model.setFocus('a');
		press('Space');
		await vi.waitFor(() => {
			expect(model.rowCount).toBe(5);
		});
		expect([selected(model), takeOpened()]).toEqual([['x'], []]);
		// Enter on a folder toggles it as well.
		press('Enter');
		expect([model.rowCount, selected(model)]).toEqual([2, ['a']]);
		expect(takeOpened()).toEqual([{ key: 'a', pinned: true, preserveFocus: false }]);
		press('End');
		press('Space');
		expect(selected(model)).toEqual(['x']);
		expect(takeOpened()).toEqual([{ key: 'x', pinned: false, preserveFocus: true }]);
	});

	it("selects the focused row's siblings, then their parent too, then every row", async () => {
		const { model, press } = await openTree(
			[folder('a', 3), folder('b', 2), file('x')],
			['a', 'b'],
		);
		model.setFocus('a/2');
		const selectAll = () => {
			press('KeyA', { ctrlKey: true });
			return selected(model);
		};

		expect(selectAll()).toEqual(['a/1', 'a/2', 'a/3']);
		expect(selectAll()).toEqual(['a', 'a/1', 'a/2', 'a/3']);
		expect(selectAll()).toEqual(['a', 'a/1', 'a/2', 'a/3', 'b', 'b/1', 'b/2', 'x']);
	});

	it('clears several selected rows with Escape, then focus as well, and leaves the key once nothing is left', async () => {
		const { model, press } = await openTree([file('x'), file('y')]);
		model.setFocus('y');
		model.setAnchor('x');
		model.setSelection(['x', 'y']);

		press('Escape');
		expect([model.focus, model.anchor, selected(model)]).toEqual(['y', null, []]);
		model.setSelection(['y']);
		press('Escape');
		expect([model.focus, selected(model)]).toEqual([null, []]);
		expect(press('Escape')).toBe(false);
	});

	it('moves focus only to rows the find matched while two or more match, and to any row with Alt', async () => {
		const others = Array.from({ length: 8 }, (_, index) => file(`b${index + 2}`));
		const { model, press, startFind } = await openTree([
			file('b0'),
			file('a1'),
			file('b1'),
			file('a2'),
			...others,
			file('a3'),
			file('bz'),
		]);
		const find = startFind();
		find.open();
		find.type('a');
		await vi.waitFor(() => {
			expect(model.findStatus?.query.pattern).toBe('a');
		});
		expect(model.focus).toBe('a1');

		const focusAfter = (code: string, modifiers: KeyModifiers = {}) => {
			press(code, modifiers);
			return model.focus;
		};
		expect([
			focusAfter('ArrowDown'),
			focusAfter('ArrowDown', { altKey: true }),
			focusAfter('ArrowUp'),
			focusAfter('Home'),
			focusAfter('Home', { altKey: true }),
			focusAfter('End'),
			focusAfter('End', { altKey: true }),
			focusAfter('Home'),
			// The last matched row in sight, where the page's last row does not match.
			focusAfter('PageDown'),
		]).toEqual(['a2', 'b2', 'a2', 'a1', 'b0', 'a3', 'bz', 'a1', 'a2']);
	});

	it('scrolls with Cmd+Up on macOS until the top, where it collapses, and with Ctrl+Up only elsewhere', async () => {
		const { model, press, viewport } = await openTree([folder('b', 40)], ['b']);
		model.setFocus('b');
		model.setScrollTop(15);

		press('ArrowUp', { metaKey: true }, 'apple');
		expect(viewport.scrollTop).toBe(5);
		press('ArrowUp', { metaKey: true }, 'apple');
		expect([viewport.scrollTop, model.rowCount]).toEqual([0, 41]);
		press('ArrowUp', { metaKey: true }, 'apple');
		expect(model.rowCount).toBe(1);
		expect(press('ArrowUp', { ctrlKey: true })).toBe(false);
	});
});

describe('tree clicks', () => {
	it('selects from the anchor with Shift in place of its run of selected rows, and toggles one row with Ctrl', async () => {
		const { model, viewport } = await openTree(
			Array.from({ length: 8 }, (_, index) => file(`r${index}`)),
		);
		model.setSelection(['r0', 'r1', 'r5']);
		model.setAnchor('r1');
		model.setFocus('r1');

		selectRange(viewport, 3);
		expect([model.focus, model.anchor, selected(model)]).toEqual([
			'r3',
			'r1',
			['r1', 'r2', 'r3', 'r5'],
		]);
		toggleRow(model, 'r5', 'r5');
		expect([model.focus, model.anchor, selected(model)]).toEqual(['r5', 'r5', ['r1', 'r2', 'r3']]);
		// Without an anchor, the focused row becomes one.
		model.setAnchor(null);
		model.setSelection([]);
		model.setFocus('r6');
		selectRange(viewport, 4);
		expect([model.anchor, selected(model)]).toEqual(['r6', ['r4', 'r5', 'r6']]);
	});
});

describe('type navigation', () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	it('moves focus to the next row a typed word starts, until an 800 ms pause ends the word', async () => {
		const { model, viewport } = await openTree(
			['alpha', 'beta', 'better', 'bravo', 'Charlie'].map((key) => file(key)),
		);
		vi.useFakeTimers();
		const navigation = new TreeTypeNavigation<string, Item>();
		const label = (row: VirtualTreeRow<string, Item>) => row.element.key;
		const type = (character: string) => {
			navigation.type(viewport, label, character);
			return model.focus;
		};

		expect(type('b')).toBe('beta');
		// Later characters look from the focused row, so a longer word moves only when it must.
		expect(type('e')).toBe('beta');
		expect(type('t')).toBe('beta');
		expect(type('t')).toBe('better');
		vi.advanceTimersByTime(800);
		// A new word looks past the focused row; a word no row starts leaves focus.
		expect(type('b')).toBe('bravo');
		expect(type('b')).toBe('bravo');
		vi.advanceTimersByTime(800);
		expect(type('b')).toBe('beta');
		vi.advanceTimersByTime(800);
		expect(type('c')).toBe('Charlie');
		expect(selected(model)).toEqual([]);
		navigation.dispose();
	});
});
