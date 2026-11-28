import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { fileTabLabels } from '../fileTabLabels';

interface GeneratedFile {
	readonly drive: 'C' | 'D';
	readonly folders: readonly string[];
	readonly name: string;
	readonly style: 'drive' | 'posix' | 'unc';
}

const fileArbitrary: fc.Arbitrary<GeneratedFile> = fc.record({
	drive: fc.constantFrom('C', 'D'),
	// Few names on purpose, so that folders share runs and the shortest unique run is searched.
	folders: fc.array(fc.constantFrom('a', 'b', 'src', 'a b', 'lib'), { maxLength: 4 }),
	name: fc.constantFrom('index.ts', 'a.ts', 'README.md'),
	style: fc.constantFrom('drive', 'posix', 'unc'),
});

function pathOf(file: GeneratedFile): string {
	switch (file.style) {
		case 'drive':
			return `${file.drive}:\\${[...file.folders, file.name].join('\\')}`;
		case 'posix':
			return `/${[...file.folders, file.name].join('/')}`;
		case 'unc':
			return `\\\\server\\share\\${[...file.folders, file.name].join('\\')}`;
	}
}

/** Which folder holds the file, independent of how its path spells it. */
function folderOf(file: GeneratedFile): string {
	return `${file.style}:${file.style === 'drive' ? file.drive : ''}:${file.folders.join('/')}`;
}

const rowArbitrary = fc.array(fileArbitrary, { maxLength: 8, minLength: 1 });

describe('fileTabLabels', () => {
	it('names each tab by its file and describes exactly the names that several folders share', () => {
		fc.assert(
			fc.property(rowArbitrary, (files) => {
				const labels = fileTabLabels(files.map(pathOf));
				files.forEach((file, index) => {
					expect(labels[index]?.name).toBe(file.name);
					const sharing = files.filter((other) => other.name === file.name);
					const folders = new Set(sharing.map(folderOf));
					expect(labels[index]?.description === '').toBe(folders.size < 2);
					files.forEach((other, otherIndex) => {
						if (other.name !== file.name) return;
						// Two tabs of one name read the same exactly when their files share a folder.
						expect(labels[index]?.description === labels[otherIndex]?.description).toBe(
							folderOf(file) === folderOf(other),
						);
					});
				});
			}),
		);
	});

	it('labels a tab the same wherever it stands in the row', () => {
		const rowAndOrder = rowArbitrary.chain((files) =>
			fc.tuple(
				fc.constant(files),
				fc.shuffledSubarray(files, { maxLength: files.length, minLength: files.length }),
			),
		);
		fc.assert(
			fc.property(rowAndOrder, ([files, reordered]) => {
				const labelOf = (row: readonly GeneratedFile[]) => {
					const paths = row.map(pathOf);
					const labels = fileTabLabels(paths);
					return new Map(paths.map((path, index) => [path, labels[index]]));
				};
				expect(labelOf(reordered)).toStrictEqual(labelOf(files));
			}),
		);
	});

	// VS Code's own `shorten` cases (src/vs/base/test/common/labels.test.ts) that describe folders a
	// file can have: two tabs of one name read the folder parts VS Code shows.
	it.each([
		[
			['a/b', 'c/d', 'e/f'],
			['…/b', '…/d', '…/f'],
		],
		[
			['a/b', 'a/b/c'],
			['…/b', '…/c'],
		],
		[
			['/a/b', '/a/c'],
			['/a/b', '/a/c'],
		],
		[
			['a/b/c', 'd/b/c'],
			['a/…', 'd/…'],
		],
		[
			['d/e/a/b/c', 'd/b/c'],
			['…/a/…', 'd/b/…'],
		],
		[
			['a/b/c/d', 'a/f/b/c/d'],
			['a/b/…', '…/f/…'],
		],
		[
			['/x/b', '/y/b'],
			['/x/…', '/y/…'],
		],
		[
			['a/b/c', 'd/b/C'],
			['…/c', '…/C'],
		],
		[
			['x:\\a\\b', 'x:\\a\\c'],
			['x:\\…\\b', 'x:\\…\\c'],
		],
		[
			['x:\\a\\b\\c', 'x:\\0\\a\\b\\c'],
			['x:\\a\\…', 'x:\\0\\…'],
		],
		[
			['x:\\a\\b', 'y:\\x\\a\\b'],
			['x:\\…', 'y:\\…'],
		],
		[
			['\\\\x\\b', '\\\\y\\b'],
			['\\\\x\\…', '\\\\y\\…'],
		],
		[
			['\\\\a\\b', '\\\\a\\c'],
			['\\\\a\\b', '\\\\a\\c'],
		],
	])('describes the folders %j as %j', (folders, descriptions) => {
		const paths = folders.map((folder) => `${folder}${folder.includes('\\') ? '\\' : '/'}x.ts`);
		expect(fileTabLabels(paths).map((label) => label.description)).toStrictEqual(descriptions);
	});
});
