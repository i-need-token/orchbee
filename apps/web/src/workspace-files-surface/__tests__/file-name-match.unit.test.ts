import { describe, expect, it } from 'vitest';

import {
	describeMatch,
	lowerAligned,
	narrows,
	parseFileQuery,
	scorePath,
} from '../file-name-match';

// Quick open's ranking decides which file a person reaches with Enter, and no browser runs here to
// try it, so these rules are kept as evidence: names before folders, words before scattered
// letters, every piece required, the location suffix, and marks that fall on the matched text.

function rank(text: string, paths: readonly string[]): readonly string[] {
	const query = parseFileQuery(text);
	return paths
		.flatMap((path) => {
			const score = scorePath(query, path, lowerAligned(path));
			return score === null ? [] : [{ path, score }];
		})
		.sort((left, right) => right.score - left.score || left.path.length - right.path.length)
		.map(({ path }) => path);
}

describe('parseFileQuery', () => {
	it('takes a trailing line and column off the name', () => {
		expect(parseFileQuery('main.rs:12:5')).toEqual({
			column: 5,
			line: 12,
			pieces: ['main.rs'],
			typed: ['main.rs'],
		});
		expect(parseFileQuery('main.rs:12,5')).toMatchObject({ column: 5, line: 12 });
		expect(parseFileQuery('main.rs:0')).toMatchObject({ pieces: ['main.rs:0'] });
	});

	it('splits pieces on whitespace and reads backslashes as slashes', () => {
		expect(parseFileQuery('  Src\\App  view ')).toMatchObject({
			pieces: ['src/app', 'view'],
			typed: ['Src/App', 'view'],
		});
	});
});

describe('scorePath', () => {
	it('prefers a match in the file name to one spread across folders', () => {
		expect(rank('button', ['src/button/index.ts', 'src/ui/Button.tsx'])).toEqual([
			'src/ui/Button.tsx',
			'src/button/index.ts',
		]);
	});

	it('prefers letters together and at word starts to scattered ones', () => {
		expect(rank('fr', ['a/fooBar.ts', 'a/frame.ts'])[0]).toBe('a/frame.ts');
		expect(rank('wfs', ['a/workspaceFilesSurface.tsx', 'a/wolfsbane.ts'])[0]).toBe(
			'a/workspaceFilesSurface.tsx',
		);
	});

	it('needs every piece, and matches a piece with a slash against the path', () => {
		expect(rank('ws ui', ['ws/ui.ts', 'ws/core.ts'])).toEqual(['ws/ui.ts']);
		expect(rank('ui/btn', ['src/ui/Button.tsx', 'src/btn.ts'])).toEqual(['src/ui/Button.tsx']);
		expect(rank('zzz', ['a.ts'])).toEqual([]);
	});
});

describe('describeMatch', () => {
	it('marks what matched in the name and in the folder', () => {
		const query = parseFileQuery('ui btn');
		const path = 'src/ui/Button.tsx';
		const match = describeMatch(query, path, lowerAligned(path));
		expect(match).toMatchObject({ folder: 'src/ui', name: 'Button.tsx' });
		expect(match.folderMatches).toEqual([{ end: 6, start: 4 }]);
		expect(match.nameMatches).toEqual([
			{ end: 1, start: 0 },
			{ end: 4, start: 3 },
			{ end: 6, start: 5 },
		]);
	});
});

describe('narrows', () => {
	it('lets a longer query start from what a shorter one kept', () => {
		expect(narrows(parseFileQuery('but'), parseFileQuery('butt'))).toBe(true);
		expect(narrows(parseFileQuery('but'), parseFileQuery('but ui'))).toBe(true);
		expect(narrows(parseFileQuery('butt'), parseFileQuery('but'))).toBe(false);
	});
});
