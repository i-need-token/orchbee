import { readdir } from 'node:fs/promises';
import { basename, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const repositoryRoot = resolve(import.meta.dirname, '../../../..');
const componentDirectory = resolve(repositoryRoot, 'packages/ui/src/components/ui');
const storyDirectory = resolve(repositoryRoot, 'packages/ui/stories');

describe('UI Story coverage', () => {
	it('keeps one Story file for every public component module', async () => {
		const [componentEntries, storyEntries] = await Promise.all([
			readdir(componentDirectory),
			readdir(storyDirectory),
		]);
		const componentNames = componentEntries
			.filter((entry) => entry.endsWith('.tsx'))
			.map((entry) => basename(entry, '.tsx'))
			.toSorted();
		const storyNames = storyEntries
			.filter((entry) => entry.endsWith('.stories.tsx'))
			.map((entry) => basename(entry, '.stories.tsx'))
			.toSorted();

		expect(storyNames).toEqual(componentNames);
	});
});
