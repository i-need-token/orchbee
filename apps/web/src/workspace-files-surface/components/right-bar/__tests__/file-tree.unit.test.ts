import type { WorkspaceDirectoryEntry } from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import { flattenTree, type Listing, parentPath } from '../file-tree';

function file(name: string): WorkspaceDirectoryEntry {
	return { entryKind: 'file', ignored: false, name, symbolicLink: false };
}

function folder(name: string): WorkspaceDirectoryEntry {
	return { entryKind: 'directory', ignored: false, name, symbolicLink: false };
}

describe('flattenTree', () => {
	it('walks only into expanded folders and notes what is not read yet', () => {
		const listings = new Map<string, Listing>([
			[
				'',
				{
					entries: [folder('src'), folder('docs'), file('README.md')],
					state: 'loaded',
					truncated: false,
				},
			],
			['src', { entries: [folder('empty'), file('main.ts')], state: 'loaded', truncated: true }],
			['src/empty', { entries: [], state: 'loaded', truncated: false }],
			['docs', { reason: 'denied', state: 'failed' }],
		]);
		const rows = flattenTree(listings, new Set(['src', 'src/empty', 'docs', 'missing']));
		expect(
			rows.map((row) =>
				row.kind === 'entry'
					? `${row.depth}:${row.path}${row.expanded ? '/' : ''}`
					: `${row.depth}:(${row.note})`,
			),
		).toEqual([
			'0:src/',
			'1:src/empty/',
			'2:(empty)',
			'1:src/main.ts',
			'1:(truncated)',
			'0:docs/',
			'1:(failed)',
			'0:README.md',
		]);
	});

	it('shows the root as loading before its first listing arrives', () => {
		expect(flattenTree(new Map(), new Set())).toEqual([
			expect.objectContaining({ kind: 'note', note: 'loading' }),
		]);
	});
});

describe('parentPath', () => {
	it('names the folder a path sits in', () => {
		expect(parentPath('a')).toBe('');
		expect(parentPath('a/b/c')).toBe('a/b');
	});
});
