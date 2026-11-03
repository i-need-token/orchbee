import { describe, expect, it } from 'vitest';

import { sameDirectorySpelling, worktreeDestination } from '../worktree-paths';

// The Rust protocol's and native Clients' own cases, so a Worktree this Client names is the one the
// TUI and CLI name. Each result stays beside its input, so a failure names the case.
describe('sameDirectorySpelling', () => {
	it('compares POSIX spellings exactly and never equals them to Windows ones', () => {
		const others = [
			'/Trees/a',
			'/trees/A',
			'/trees/a/',
			'/trees//a',
			String.raw`\trees\a`,
			'C:/trees/a',
		];
		expect(sameDirectorySpelling('/trees/a', '/trees/a')).toBe(true);
		expect(
			others.map((other) => [
				other,
				sameDirectorySpelling('/trees/a', other) || sameDirectorySpelling(other, '/trees/a'),
			]),
		).toEqual(others.map((other) => [other, false]));
	});

	it('lets Windows spellings differ only in separators, a trailing separator and ASCII case', () => {
		expect(sameDirectorySpelling('C:\\Trees\\A\\', 'c:/trees/a')).toBe(true);
		expect(sameDirectorySpelling('c:\\TREES\\猫\\', 'C:/trees/猫')).toBe(true);
		expect(sameDirectorySpelling('C:/trees/猫x', 'C:/trees/猫')).toBe(false);
	});
});

describe('worktreeDestination', () => {
	it('joins the configured root, the repository name and a bounded branch slug', () => {
		const slugs = [
			['worktree/brave-river', 'worktree-brave-river'],
			['issue/137 Worktree Spaces', 'issue-137-worktree-spaces'],
			['///', 'worktree'],
			['../A__B/', 'a-b'],
			['中文', 'worktree'],
		] as const;
		expect(
			slugs.map(([branch]) => [branch, worktreeDestination('/srv/trees', '/home/me/repo', branch)]),
		).toEqual(slugs.map(([branch, slug]) => [branch, `/srv/trees/repo/${slug}`]));
		expect(
			worktreeDestination(String.raw`C:\configured`, String.raw`D:\Projects\repo`, 'Feature/One'),
		).toBe(String.raw`C:\configured\repo\feature-one`);
		expect(worktreeDestination('C:/configured/', 'D:/Projects/repo/', 'x')).toBe(
			'C:/configured/repo\\x',
		);
		expect(worktreeDestination('/srv', '/', 'x')).toBe('/srv/repository/x');
	});
});
