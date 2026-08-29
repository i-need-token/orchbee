import path from 'node:path';

import { describe, expect, it } from 'vitest';

import { resolveAgentExecutableCandidates } from '../resolveAgentExecutableCandidates';

describe('resolveAgentExecutableCandidates', () => {
	it('rejects names outside the closed command-name grammar', () => {
		const searchDirectory = path.resolve(path.sep, 'tools');
		expect(resolveAgentExecutableCandidates('a'.repeat(32), [searchDirectory])).toEqual({
			kind: 'candidates',
			paths: [path.join(searchDirectory, 'a'.repeat(32))],
		});
		expect(resolveAgentExecutableCandidates('a'.repeat(33), [searchDirectory])).toEqual({
			kind: 'invalid-name',
		});
		expect(resolveAgentExecutableCandidates('../codex', [process.env['PATH']])).toEqual({
			kind: 'invalid-name',
		});
		expect(resolveAgentExecutableCandidates('Codex', [process.env['PATH']])).toEqual({
			kind: 'invalid-name',
		});
	});

	it('preserves source order while rejecting unsafe directories and duplicates', () => {
		const firstDirectory = path.resolve(path.sep, 'tools', 'first');
		const secondDirectory = path.resolve(path.sep, 'tools', 'second');
		const maximumDirectory = createAbsoluteDirectoryWithLength(4096);
		const searchPaths = [
			[
				'',
				'relative',
				firstDirectory,
				`${path.sep}unsafe\ndirectory`,
				createAbsoluteDirectoryWithLength(4097),
				firstDirectory,
				maximumDirectory,
			].join(path.delimiter),
			[secondDirectory, firstDirectory].join(path.delimiter),
		];

		expect(resolveAgentExecutableCandidates('codex', searchPaths)).toEqual({
			kind: 'candidates',
			paths: [
				path.join(firstDirectory, 'codex'),
				path.join(maximumDirectory, 'codex'),
				path.join(secondDirectory, 'codex'),
			],
		});
	});

	it('bounds the directories admitted from each search-path source', () => {
		const directories = Array.from({ length: 257 }, (_, index) =>
			path.resolve(path.sep, 'tools', String(index)),
		);
		const resolution = resolveAgentExecutableCandidates('codex', [
			directories.join(path.delimiter),
		]);

		expect(resolution.kind).toBe('candidates');
		if (resolution.kind !== 'candidates') return;
		expect(resolution.paths).toHaveLength(256);
		expect(resolution.paths.at(-1)).toBe(path.join(directories[255] ?? '', 'codex'));
	});
});

function createAbsoluteDirectoryWithLength(length: number) {
	const root = path.parse(path.resolve(path.sep)).root;
	return `${root}${'x'.repeat(length - root.length)}`;
}
