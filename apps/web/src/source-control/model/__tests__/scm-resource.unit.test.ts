import { describe, expect, it } from 'vitest';

import type { GitStatusEntry } from '../../../file-session';
import { SCM_STATUS_FACTS, scmResourceGroups } from '../scm-resource';

const entry = (xy: string, path: string, original: string | null = null): GitStatusEntry => ({
	original,
	path,
	submodule: false,
	x: xy.charAt(0),
	y: xy.charAt(1),
});

/** Rows as `group letter path`, the shape VS Code's Changes view shows them in. */
function rows(entries: readonly GitStatusEntry[]): string[] {
	const groups = scmResourceGroups(entries);
	return [...groups.merge, ...groups.index, ...groups.workingTree].map(
		({ group, path, status }) => `${group} ${SCM_STATUS_FACTS[status].letter} ${path}`,
	);
}

describe('scmResourceGroups', () => {
	it('sorts porcelain pairs into the groups VS Code gives them', () => {
		expect(
			rows([
				entry('M.', '/r/staged'),
				entry('.M', '/r/changed'),
				entry('MM', '/r/both'),
				entry('??', '/r/new'),
				entry('!!', '/r/ignored'),
				entry('A.', '/r/added'),
				entry('.A', '/r/intent'),
				entry('D.', '/r/removed'),
				entry('.D', '/r/deleted'),
				entry('.T', '/r/type'),
				entry('UU', '/r/conflict'),
				entry('DD', '/r/both-deleted'),
			]),
		).toEqual([
			'merge ! /r/conflict',
			'merge ! /r/both-deleted',
			'index M /r/staged',
			'index M /r/both',
			'index A /r/added',
			'index D /r/removed',
			'workingTree M /r/changed',
			'workingTree M /r/both',
			'workingTree U /r/new',
			'workingTree I /r/ignored',
			'workingTree A /r/intent',
			'workingTree D /r/deleted',
			'workingTree T /r/type',
		]);
	});

	it('shows a rename at its new path and keeps the source for both of its rows', () => {
		const groups = scmResourceGroups([entry('RM', '/r/new', '/r/old')]);
		expect(groups.index).toEqual([
			{ group: 'index', original: '/r/old', path: '/r/new', status: 'index-renamed' },
		]);
		expect(groups.workingTree).toEqual([
			{ group: 'workingTree', original: '/r/old', path: '/r/new', status: 'modified' },
		]);
	});

	it('gives a staged type change no row, as VS Code reads only M, A, D, R and C in the index', () => {
		expect(rows([entry('T.', '/r/link')])).toEqual([]);
	});
});
