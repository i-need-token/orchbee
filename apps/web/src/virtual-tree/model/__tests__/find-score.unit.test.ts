import { describe, expect, it } from 'vitest';

import { findMatcher, findTester } from '../find-score';
import type { VirtualTreeFindMatchType } from '../virtual-tree-types';
import fixture from './fixtures/vscode-fuzzy-score.json';

/**
 * Every fixture pair's score, as rows of the pair's index followed by the score, which is how the
 * fixture records VS Code's results; a pair that misses has no row.
 */
function scoreRows(matchType: VirtualTreeFindMatchType): number[][] {
	const rows: number[][] = [];
	for (const [index, [pattern = '', label = '']] of fixture.pairs.entries()) {
		const score = findMatcher(pattern, matchType)(label);
		if (score !== undefined) rows.push([index, ...score]);
	}
	return rows;
}

/** The indexes of the pairs the find matches without scoring them. */
function testedRows(matchType: VirtualTreeFindMatchType): number[] {
	const indexes: number[] = [];
	for (const [index, [pattern = '', label = '']] of fixture.pairs.entries()) {
		if (findTester(pattern, matchType)(label)) indexes.push(index);
	}
	return indexes;
}

describe('find matching', () => {
	it('matches and scores every pair as VS Code does', () => {
		expect(scoreRows('fuzzy')).toEqual(fixture.fuzzy);
		expect(scoreRows('contiguous')).toEqual(fixture.contiguous);
		expect(testedRows('fuzzy')).toEqual(fixture.fuzzy.map(([index]) => index));
		expect(testedRows('contiguous')).toEqual(fixture.contiguous.map(([index]) => index));
	});
});
