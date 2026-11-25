import type { WorkspaceGitStatusEntry } from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import { groupChanges, splitPath } from '../change-groups';

function entry(
	overrides: Partial<WorkspaceGitStatusEntry> & { path: string },
): WorkspaceGitStatusEntry {
	return {
		conflicted: false,
		index: 'unmodified',
		originalPath: null,
		worktree: 'unmodified',
		...overrides,
	};
}

describe('groupChanges', () => {
	it('sorts changes into conflicts, staged and unstaged, with the sides each diff compares', () => {
		const groups = groupChanges([
			entry({ path: 'src/b.ts', worktree: 'modified' }),
			entry({ index: 'modified', path: 'src/a.ts', worktree: 'modified' }),
			entry({ index: 'added', path: 'new.ts' }),
			entry({ index: 'renamed', originalPath: 'old.ts', path: 'moved.ts' }),
			entry({ index: 'untracked', path: 'notes.md', worktree: 'untracked' }),
			entry({ conflicted: true, index: 'unmerged', path: 'clash.ts', worktree: 'unmerged' }),
		]);
		expect(groups.conflicts.map((item) => [item.path, item.letter])).toEqual([['clash.ts', '!']]);
		expect(
			groups.staged.map((item) => [
				item.path,
				item.letter,
				item.before,
				item.beforePath,
				item.after,
			]),
		).toEqual([
			['moved.ts', 'R', 'head', 'old.ts', 'index'],
			['new.ts', 'A', null, 'new.ts', 'index'],
			['src/a.ts', 'M', 'head', 'src/a.ts', 'index'],
		]);
		expect(
			groups.unstaged.map((item) => [item.path, item.letter, item.before, item.after]),
		).toEqual([
			['notes.md', 'U', null, 'worktree'],
			['src/a.ts', 'M', 'index', 'worktree'],
			['src/b.ts', 'M', 'index', 'worktree'],
		]);
		const keys = [...groups.conflicts, ...groups.staged, ...groups.unstaged].map(
			(item) => item.key,
		);
		expect(new Set(keys).size).toBe(keys.length);
	});
});

describe('splitPath', () => {
	it('splits a path into its name and folder', () => {
		expect(splitPath('a.ts')).toEqual({ folder: '', name: 'a.ts' });
		expect(splitPath('src/x/a.ts')).toEqual({ folder: 'src/x', name: 'a.ts' });
	});
});
