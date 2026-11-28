import { describe, expect, it } from 'vitest';

import type { GitStatusEntry } from '../../../file-session';
import { VirtualTreeModel } from '../../../virtual-tree';
import { scmResourceGroups } from '../scm-resource';
import { SCM_TREE_INPUT, scmTreeSource, type ScmTreeItem, type ScmTreeSnapshot } from '../scm-tree';

const collator = new Intl.Collator('en', { numeric: true, sensitivity: 'base' });

const OPTIONS: ScmTreeSnapshot['options'] = {
	compareNames: (one, other) => collator.compare(one, other) || (one < other ? -1 : 1),
	mode: 'list',
	separator: '/',
	sortKey: 'path',
	statusText: (status) => status,
};

const entry = (xy: string, path: string): GitStatusEntry => ({
	original: null,
	path,
	submodule: false,
	x: xy.charAt(0),
	y: xy.charAt(1),
});

const repository = (
	path: string,
	entries: readonly GitStatusEntry[],
	button = false,
): ScmTreeSnapshot['repositories'][number] => ({
	button,
	groups: scmResourceGroups(entries),
	inputHeight: 24,
	repository: path,
});

function label(item: ScmTreeItem): string {
	switch (item.kind) {
		case 'workspace':
			return 'workspace';
		case 'repository':
			return `repo ${item.repository}`;
		case 'input':
		case 'button':
			return item.kind;
		case 'group':
			return `group ${item.group}`;
		case 'folder':
			return `folder ${item.names.join('/')}`;
		case 'resource':
			return `file ${item.name}`;
	}
}

/** Every row, expanded, indented by depth. */
async function rows(
	repositories: ScmTreeSnapshot['repositories'],
	options: Partial<ScmTreeSnapshot['options']> = {},
): Promise<string[]> {
	const source = scmTreeSource(() => ({ options: { ...OPTIONS, ...options }, repositories }));
	const model = new VirtualTreeModel({
		expandedByDefault: () => true,
		input: SCM_TREE_INPUT,
		source,
	});
	await model.refresh(null);
	return Array.from({ length: model.rowCount }, (_, index) => {
		const row = model.rowAt(index);
		return `${'  '.repeat(row.depth - 1)}${label(row.element)}`;
	});
}

const CHANGES = [
	repository('/repo', [
		entry('.M', '/repo/src/b10.ts'),
		entry('??', '/repo/src/b9.ts'),
		entry('.D', '/repo/A.md'),
		entry('.M', '/repo/src/deep/one/two/c.ts'),
		entry('.M', '/repo/z.md'),
	]),
];

describe('scmTreeSource', () => {
	it("lists rows by path, a folder's own files before its subfolders', names numerically", async () => {
		expect(await rows(CHANGES)).toEqual([
			'input',
			'group workingTree',
			'  file A.md',
			'  file z.md',
			'  file b9.ts',
			'  file b10.ts',
			'  file c.ts',
		]);
	});

	it('lists rows by name or by status text when asked', async () => {
		expect(await rows(CHANGES, { sortKey: 'name' })).toEqual([
			'input',
			'group workingTree',
			'  file A.md',
			'  file b9.ts',
			'  file b10.ts',
			'  file c.ts',
			'  file z.md',
		]);
		expect(await rows(CHANGES, { sortKey: 'status' })).toEqual([
			'input',
			'group workingTree',
			'  file A.md',
			'  file z.md',
			'  file b10.ts',
			'  file c.ts',
			'  file b9.ts',
		]);
	});

	it('nests a tree with folders first and merges single-folder chains', async () => {
		expect(await rows(CHANGES, { mode: 'tree' })).toEqual([
			'input',
			'group workingTree',
			'  folder src',
			'    folder deep/one/two',
			'      file c.ts',
			'    file b9.ts',
			'    file b10.ts',
			'  file A.md',
			'  file z.md',
		]);
	});

	it('shows a path staged and changed once in each group', async () => {
		expect(await rows([repository('/repo', [entry('MM', '/repo/both.ts')])])).toEqual([
			'input',
			'group index',
			'  file both.ts',
			'group workingTree',
			'  file both.ts',
		]);
	});

	it('keeps an empty Changes for one repository without a button and drops the groups of an unchanged one otherwise', async () => {
		expect(await rows([repository('/repo', [])])).toEqual(['input', 'group workingTree']);
		expect(await rows([repository('/repo', [], true)])).toEqual(['input', 'button']);
		expect(await rows([repository('/repo', [entry('.M', '/repo/a.ts')], true)])).toEqual([
			'input',
			'button',
			'group workingTree',
			'  file a.ts',
		]);
		expect(
			await rows([repository('/a', []), repository('/b', [entry('M.', '/b/staged.ts')])]),
		).toEqual([
			'repo /a',
			'  input',
			'repo /b',
			'  input',
			'  group index',
			'    file staged.ts',
			'  group workingTree',
		]);
	});

	it('splits Windows paths at the Worker separator', async () => {
		const source = scmTreeSource(() => ({
			options: { ...OPTIONS, mode: 'tree', separator: '\\' },
			repositories: [repository('C:\\repo', [entry('.M', 'C:\\repo\\src\\a.ts')])],
		}));
		const model = new VirtualTreeModel({
			expandedByDefault: () => true,
			input: SCM_TREE_INPUT,
			source,
		});
		await model.refresh(null);
		expect(model.rowAt(2).element).toMatchObject({
			kind: 'folder',
			names: ['src'],
			path: 'C:\\repo\\src',
		});
	});
});
