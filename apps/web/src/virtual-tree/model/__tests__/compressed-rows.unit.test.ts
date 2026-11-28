import { describe, expect, it } from 'vitest';

import { VirtualTreeModel } from '../VirtualTreeModel';
import type { VirtualTreeSource } from '../virtual-tree-types';
import fixture from './fixtures/vscode-compressed-tree.json';

/** An element as VS Code's compressed tree model takes it. */
interface FixtureNode {
	readonly element: number;
	readonly incompressible: boolean;
	readonly children: readonly FixtureNode[];
}

interface FixtureTree {
	readonly name: string;
	readonly tree: FixtureNode;
	readonly rows: readonly (readonly number[])[];
}

interface FixtureSequence {
	readonly name: string;
	readonly steps: readonly {
		readonly parent: number | null;
		readonly children: readonly FixtureNode[];
		readonly rows: readonly (readonly number[])[];
	}[];
}

const trees: readonly FixtureTree[] = fixture.trees;
const sequences: readonly FixtureSequence[] = fixture.sequences;

/** The input, which VS Code's model calls null. */
const INPUT = -1;

/** Serves the children each element was last given, as `setChildren` gave them to VS Code's model. */
class FixtureWorld {
	private readonly lists = new Map<number, readonly number[]>([[INPUT, []]]);
	private readonly incompressible = new Set<number>();

	readonly source: VirtualTreeSource<number, number> = {
		children: (element) => this.lists.get(element) ?? [],
		compressible: (element) => !this.incompressible.has(element),
		hasChildren: (element) => (this.lists.get(element)?.length ?? 0) > 0,
		key: (element) => element,
	};

	setChildren(parent: number, children: readonly FixtureNode[]): void {
		this.lists.set(
			parent,
			children.map((child) => child.element),
		);
		for (const child of children) {
			if (child.incompressible) this.incompressible.add(child.element);
			else this.incompressible.delete(child.element);
			this.setChildren(child.element, child.children);
		}
	}
}

function createModel(world: FixtureWorld): VirtualTreeModel<number, number> {
	// VS Code's object tree leaves every element it is given expanded.
	return new VirtualTreeModel({
		expandedByDefault: () => true,
		input: INPUT,
		source: world.source,
	});
}

/** Expands the rows a refresh left collapsed, such as a leaf that came to have children. */
async function expandEveryRow(model: VirtualTreeModel<number, number>): Promise<void> {
	for (let round = 0; ; round += 1) {
		expect(round).toBeLessThan(100);
		const collapsed: number[] = [];
		for (let index = 0; index < model.rowCount; index += 1) {
			const row = model.rowAt(index);
			if (row.collapsible && row.collapsed) collapsed.push(row.key);
		}
		if (collapsed.length === 0) return;
		for (const key of collapsed) await model.expand(key);
	}
}

function rowsOf(model: VirtualTreeModel<number, number>): number[][] {
	return Array.from({ length: model.rowCount }, (_, index) =>
		model.rowAt(index).segments.map((segment) => segment.key),
	);
}

describe('compact rows', () => {
	it.each(trees)('join items as VS Code compresses the $name tree', async ({ rows, tree }) => {
		const world = new FixtureWorld();
		world.setChildren(INPUT, [tree]);
		const model = createModel(world);
		await model.refresh(null);
		await expandEveryRow(model);
		expect(rowsOf(model)).toEqual(rows);
	});

	it.each(sequences)(
		'rejoin items as VS Code recompresses the $name sequence',
		async ({ steps }) => {
			const world = new FixtureWorld();
			const model = createModel(world);
			for (const { children, parent, rows } of steps) {
				world.setChildren(parent ?? INPUT, children);
				await model.refresh(parent);
				await expandEveryRow(model);
				expect(rowsOf(model)).toEqual(rows);
			}
		},
	);
});
