import { describe, expect, it } from 'vitest';

import { groupChanges } from '../change-groups';
import { type FloatEvent, floatOf, type FloatWindows, reduceFloatWindows } from '../float-reducer';
import type { FloatTarget } from '../float-tabs';

const WORKSPACE = 'workspace-a';

function file(path: string): FloatTarget {
	return { kind: 'file', path };
}

function run(...events: readonly FloatEvent[]): FloatWindows {
	return events.reduce(reduceFloatWindows, new Map());
}

function open(
	target: FloatTarget,
	preview: boolean,
	workspaceId = WORKSPACE,
	unsaved: ReadonlySet<string> = new Set(),
): FloatEvent {
	return { kind: 'open', preview, target, unsaved, workspaceId };
}

function shown(windows: FloatWindows, workspaceId = WORKSPACE): readonly string[] {
	return floatOf(windows, workspaceId).tabs.map(
		(tab) => `${tab.id}${tab.preview ? ' (preview)' : ''}`,
	);
}

describe('float window tabs', () => {
	it('lets the next preview take the preview tab’s place instead of piling up tabs', () => {
		const windows = run(
			open(file('a.ts'), false),
			open(file('b.ts'), true),
			open(file('c.ts'), false),
			open(file('d.ts'), true),
		);
		expect(shown(windows)).toEqual(['file:a.ts', 'file:d.ts (preview)', 'file:c.ts']);
		expect(floatOf(windows, WORKSPACE).activeId).toBe('file:d.ts');
	});

	it('keeps a preview tab once it is opened for keeps or double-clicked, so the next preview adds a tab', () => {
		const reopened = run(
			open(file('a.ts'), true),
			open(file('a.ts'), false),
			open(file('b.ts'), true),
		);
		expect(shown(reopened)).toEqual(['file:a.ts', 'file:b.ts (preview)']);

		const kept = run(
			open(file('a.ts'), true),
			{ id: 'file:a.ts', kind: 'keep', workspaceId: WORKSPACE },
			open(file('b.ts'), true),
		);
		expect(shown(kept)).toEqual(['file:a.ts', 'file:b.ts (preview)']);
	});

	it('never lets a preview with unsaved edits be replaced, which would drop them without asking', () => {
		const unsaved = new Set(['a.ts']);
		const replacing = run(open(file('a.ts'), true), open(file('b.ts'), true, WORKSPACE, unsaved));
		expect(shown(replacing)).toEqual(['file:a.ts', 'file:b.ts (preview)']);

		const edited = run(open(file('a.ts'), true), {
			kind: 'keep-unsaved',
			paths: unsaved,
			workspaceId: WORKSPACE,
		});
		expect(shown(edited)).toEqual(['file:a.ts']);
	});

	it('does not turn a kept tab back into a preview when it is previewed again', () => {
		const windows = run(
			open(file('a.ts'), false),
			open(file('b.ts'), false),
			open(file('a.ts'), true),
		);
		expect(shown(windows)).toEqual(['file:a.ts', 'file:b.ts']);
		expect(floatOf(windows, WORKSPACE).activeId).toBe('file:a.ts');
	});

	it('opens a new tab right after the active one', () => {
		const windows = run(
			open(file('a.ts'), false),
			open(file('b.ts'), false),
			{ id: 'file:a.ts', kind: 'activate', workspaceId: WORKSPACE },
			open(file('c.ts'), false),
		);
		expect(shown(windows)).toEqual(['file:a.ts', 'file:c.ts', 'file:b.ts']);
	});

	it('hands the active tab to its right neighbor, or its left one at the end, when it closes', () => {
		const three = [open(file('a.ts'), false), open(file('b.ts'), false), open(file('c.ts'), false)];
		const middle = run(
			...three,
			{ id: 'file:b.ts', kind: 'activate', workspaceId: WORKSPACE },
			{
				ids: new Set(['file:b.ts']),
				kind: 'close',
				workspaceId: WORKSPACE,
			},
		);
		expect(floatOf(middle, WORKSPACE).activeId).toBe('file:c.ts');

		const last = run(...three, {
			ids: new Set(['file:c.ts', 'file:b.ts']),
			kind: 'close',
			workspaceId: WORKSPACE,
		});
		expect(shown(last)).toEqual(['file:a.ts']);
		expect(floatOf(last, WORKSPACE).activeId).toBe('file:a.ts');

		const inactive = run(...three, {
			ids: new Set(['file:a.ts']),
			kind: 'close',
			workspaceId: WORKSPACE,
		});
		expect(floatOf(inactive, WORKSPACE).activeId).toBe('file:c.ts');
	});

	it('forgets the window with its last tab, and keeps every Workspace’s window apart', () => {
		const windows = run(open(file('a.ts'), false), open(file('b.ts'), false, 'workspace-b'), {
			ids: new Set(['file:a.ts']),
			kind: 'close',
			workspaceId: WORKSPACE,
		});
		expect(windows.has(WORKSPACE)).toBe(false);
		expect(shown(windows, 'workspace-b')).toEqual(['file:b.ts']);
	});

	it('collapses to the pill and comes back with the same tabs', () => {
		const collapsed = run(open(file('a.ts'), false), { kind: 'collapse', workspaceId: WORKSPACE });
		expect(floatOf(collapsed, WORKSPACE)).toMatchObject({ activeId: 'file:a.ts', open: false });
		const expanded = reduceFloatWindows(collapsed, { kind: 'expand', workspaceId: WORKSPACE });
		expect(floatOf(expanded, WORKSPACE)).toMatchObject({ activeId: 'file:a.ts', open: true });
		expect(shown(expanded)).toEqual(['file:a.ts']);
	});

	it('gives the staged and the unstaged change of one file their own tabs, as the Changes view lists them twice', () => {
		const { staged, unstaged } = groupChanges([
			{
				conflicted: false,
				index: 'modified',
				originalPath: null,
				path: 'src/a.ts',
				worktree: 'modified',
			},
		]);
		const [stagedItem] = staged;
		const [unstagedItem] = unstaged;
		if (stagedItem === undefined || unstagedItem === undefined)
			throw new Error('expected both changes');
		const windows = run(
			open({ item: stagedItem, kind: 'diff' }, false),
			open({ item: unstagedItem, kind: 'diff' }, false),
			open(file('src/a.ts'), false),
		);
		expect(floatOf(windows, WORKSPACE).tabs).toHaveLength(3);
	});
});
