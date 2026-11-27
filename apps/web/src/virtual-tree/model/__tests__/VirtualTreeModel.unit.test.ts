import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { VirtualTreeModel } from '../VirtualTreeModel';
import type { VirtualTreeEvent, VirtualTreeSource, VirtualTreeSplice } from '../virtual-tree-types';

interface Item {
	readonly key: string;
	readonly folder: boolean;
	readonly expanded: boolean;
	readonly version: number;
}

interface SeedItem {
	readonly parent: number;
	readonly folder: boolean;
	readonly expanded: boolean;
}

interface FreshItem {
	readonly folder: boolean;
	readonly expanded: boolean;
	readonly position: number;
	/** Files a new folder arrives with. */
	readonly children: number;
}

/** One folder's edit: drop, update, flip or keep each child; reorder, add, move one in, swap. */
interface Edit {
	readonly target: number;
	readonly decisions: readonly number[];
	readonly shuffle: boolean;
	readonly fresh: readonly FreshItem[];
	readonly move: number | null;
	/** Swaps the folder with its first child folder, as renaming `a/b` to `b/a` would. */
	readonly swap: boolean;
}

/** What the source serves. Tests edit it and then refresh the model, as a view would. */
class World {
	readonly input: Item = { expanded: false, folder: true, key: 'root', version: 0 };
	/** Repeats the first child at the end of every list, to exercise repeated keys. */
	echo = false;
	private readonly lists = new Map<string, Item[]>([['root', []]]);
	private next = 0;

	constructor(seed: readonly SeedItem[]) {
		const folders: Item[] = [this.input];
		for (const entry of seed) {
			// Half the items go into the newest folder, so the trees grow deep as well as wide.
			const index = entry.parent % 2 === 0 ? folders.length - 1 : entry.parent % folders.length;
			const parent = folders[index] ?? this.input;
			const item = this.create(entry.folder, entry.expanded);
			this.list(parent.key).push(item);
			if (item.folder) folders.push(item);
		}
	}

	childrenOf(item: Item): readonly Item[] {
		const list = [...(this.lists.get(item.key) ?? [])];
		const first = list[0];
		if (this.echo && first !== undefined) list.push(first);
		return list;
	}

	mutate(folder: Item, edit: Edit): void {
		const { decisions, move } = edit;
		const decide = (index: number): number => decisions[index % decisions.length] ?? 50;
		const next: Item[] = [];
		for (const [index, item] of this.list(folder.key).entries()) {
			const decision = decide(index);
			if (decision < 15) {
				this.forget(item);
			} else if (decision < 30) {
				next.push({ ...item, version: item.version + 1 });
			} else if (decision < 45) {
				if (item.folder) this.forget(item);
				else this.lists.set(item.key, []);
				next.push({ ...item, folder: !item.folder, version: item.version + 1 });
			} else {
				next.push(item);
			}
		}
		if (edit.shuffle) {
			const order = new Map(next.map((item, index) => [item.key, decide(index * 7 + 3)]));
			next.sort((left, right) => (order.get(left.key) ?? 0) - (order.get(right.key) ?? 0));
		}
		for (const entry of edit.fresh) {
			const item = this.create(entry.folder, entry.expanded);
			if (item.folder) {
				for (let index = 0; index < entry.children; index += 1) {
					this.list(item.key).push(this.create(false, false));
				}
			}
			next.splice(entry.position % (next.length + 1), 0, item);
		}
		this.lists.set(folder.key, next);
		if (move !== null) {
			const ancestors = this.ancestorsOf(folder.key);
			const candidates = this.items().filter(
				(item) => !ancestors.has(item.key) && !next.includes(item),
			);
			const moving = candidates[move % Math.max(1, candidates.length)];
			if (moving !== undefined) {
				this.detach(moving);
				next.splice(move % (next.length + 1), 0, moving);
			}
		}
		if (edit.swap) this.swap(folder);
	}

	/** The folder an edit at this key lands in: the item itself, else its parent, else the input. */
	folderFor(key: string): Item {
		const item = this.items().find((candidate) => candidate.key === key);
		if (item === undefined) return this.input;
		if (item.folder) return item;
		return this.parentOf(item) ?? this.input;
	}

	/** Every reachable item, input first. */
	items(): Item[] {
		const result: Item[] = [];
		const visit = (item: Item): void => {
			result.push(item);
			for (const child of this.lists.get(item.key) ?? []) visit(child);
		};
		visit(this.input);
		return result;
	}

	private swap(folder: Item): void {
		const parent = this.parentOf(folder);
		const own = this.list(folder.key);
		const child = own.find((item) => item.folder);
		if (parent === null || child === undefined) return;
		own.splice(own.indexOf(child), 1);
		const siblings = this.list(parent.key);
		siblings.splice(siblings.indexOf(folder), 1, child);
		this.list(child.key).unshift(folder);
	}

	private parentOf(item: Item): Item | null {
		for (const candidate of this.items()) {
			if (this.lists.get(candidate.key)?.includes(item) === true) return candidate;
		}
		return null;
	}

	/** Removes an item from whichever list holds it. */
	private detach(item: Item): void {
		for (const list of this.lists.values()) {
			const index = list.indexOf(item);
			if (index >= 0) list.splice(index, 1);
		}
	}

	private ancestorsOf(key: string): Set<string> {
		const parents = new Map<string, string>();
		for (const [parent, list] of this.lists)
			for (const child of list) parents.set(child.key, parent);
		const result = new Set<string>([key]);
		for (let current = parents.get(key); current !== undefined; current = parents.get(current)) {
			result.add(current);
		}
		return result;
	}

	private create(folder: boolean, expanded: boolean): Item {
		const item = { expanded, folder, key: `k${this.next}`, version: 0 };
		this.next += 1;
		if (folder) this.lists.set(item.key, []);
		return item;
	}

	private forget(item: Item): void {
		for (const child of this.lists.get(item.key) ?? []) this.forget(child);
		this.lists.delete(item.key);
	}

	private list(key: string): Item[] {
		let list = this.lists.get(key);
		if (list === undefined) {
			list = [];
			this.lists.set(key, list);
		}
		return list;
	}
}

interface NaiveNode {
	readonly key: string;
	item: Item;
	readonly parent: NaiveNode | null;
	children: NaiveNode[];
	collapsible: boolean;
	collapsed: boolean;
	stale: boolean;
}

/**
 * The same semantics written as plainly as possible: whole-tree searches, rows rebuilt from scratch,
 * no counts, no index caches, no splices.
 */
class NaiveTree {
	readonly root: NaiveNode;
	focus: string | null = null;
	anchor: string | null = null;
	selection: string[] = [];
	private readonly world: World;
	private readonly defaults: boolean;

	constructor(world: World, defaults: boolean) {
		this.world = world;
		this.defaults = defaults;
		this.root = {
			children: [],
			collapsed: false,
			collapsible: true,
			item: world.input,
			key: world.input.key,
			parent: null,
			stale: true,
		};
	}

	nodes(): NaiveNode[] {
		const result: NaiveNode[] = [];
		const visit = (node: NaiveNode): void => {
			for (const child of node.children) {
				result.push(child);
				visit(child);
			}
		};
		visit(this.root);
		return result;
	}

	rows(): NaiveNode[] {
		const result: NaiveNode[] = [];
		const visit = (node: NaiveNode): void => {
			for (const child of node.children) {
				result.push(child);
				if (!child.collapsed) visit(child);
			}
		};
		visit(this.root);
		return result;
	}

	find(key: string): NaiveNode | null {
		return this.nodes().find((node) => node.key === key) ?? null;
	}

	refresh(key: string | null, recursive: boolean): void {
		const node = key === null ? this.root : this.find(key);
		if (node === null) return;
		if (node !== this.root && node.collapsed) {
			node.collapsible = node.item.folder;
			node.children = [];
			node.stale = node.collapsible;
		} else {
			this.refreshSubtree(node, recursive);
		}
		this.prune();
	}

	expand(key: string, recursive: boolean): void {
		const node = this.find(key);
		if (node === null || !node.collapsible) return;
		if (node.stale) this.load(node, false);
		if (recursive) this.expandBelow(node);
		node.collapsed = false;
		this.prune();
	}

	collapse(key: string, recursive: boolean): void {
		const node = this.find(key);
		if (node === null) return;
		if (node.collapsible) node.collapsed = true;
		if (recursive) this.collapseBelow(node);
	}

	collapseAll(): void {
		this.collapseBelow(this.root);
	}

	reveal(key: string): void {
		const node = this.find(key);
		for (
			let owner = node?.parent ?? null;
			owner !== null && owner !== this.root;
			owner = owner.parent
		) {
			owner.collapsed = false;
		}
	}

	setFocus(key: string | null): void {
		if (key === null || this.find(key) !== null) this.focus = key;
	}

	setAnchor(key: string | null): void {
		if (key === null || this.find(key) !== null) this.anchor = key;
	}

	setSelection(keys: readonly string[]): void {
		this.selection = [...new Set(keys.filter((key) => this.find(key) !== null))];
	}

	selectAll(): void {
		this.selection = this.rows().map((node) => node.key);
	}

	private refreshSubtree(node: NaiveNode, recursive: boolean): void {
		if (node !== this.root) {
			if (!node.item.folder) {
				node.children = [];
				node.collapsible = false;
				node.collapsed = true;
				node.stale = false;
				return;
			}
			node.collapsible = true;
		}
		for (const child of this.load(node, recursive)) {
			if (this.attached(child)) this.refreshSubtree(child, recursive);
		}
	}

	private load(node: NaiveNode, recursive: boolean): NaiveNode[] {
		const toRefresh = this.merge(node, this.world.childrenOf(node.item), recursive);
		node.stale = false;
		return toRefresh;
	}

	private merge(parent: NaiveNode, items: readonly Item[], recursive: boolean): NaiveNode[] {
		const seen = new Set<string>();
		const children: NaiveNode[] = [];
		const toRefresh: NaiveNode[] = [];
		const created: NaiveNode[] = [];
		const displaced: NaiveNode[] = [];
		for (const item of items) {
			if (seen.has(item.key)) continue;
			const existing = this.find(item.key);
			if (existing !== null && existing.parent === parent) {
				seen.add(item.key);
				existing.item = item;
				if (item.folder !== existing.collapsible) {
					existing.collapsible = item.folder;
					existing.children = [];
					existing.collapsed = true;
					existing.stale = item.folder;
				} else if (item.folder) {
					if (!existing.collapsed) toRefresh.push(existing);
					else if (recursive && !existing.stale) {
						existing.children = [];
						existing.stale = true;
					}
				}
				children.push(existing);
				continue;
			}
			if (existing !== null) {
				let ancestor = false;
				for (let current: NaiveNode | null = parent; current !== null; current = current.parent) {
					if (current === existing) ancestor = true;
				}
				if (ancestor) continue;
				displaced.push(existing);
			}
			seen.add(item.key);
			const node: NaiveNode = {
				children: [],
				collapsed: !(item.folder && this.defaults && item.expanded),
				collapsible: item.folder,
				item,
				key: item.key,
				parent,
				stale: item.folder,
			};
			created.push(node);
			children.push(node);
		}
		for (const node of created) {
			if (!node.collapsed) this.load(node, recursive);
		}
		parent.children = children;
		for (const node of displaced) {
			if (node.parent !== null && this.attached(node)) {
				node.parent.children = node.parent.children.filter((child) => child !== node);
			}
		}
		return toRefresh;
	}

	private expandBelow(node: NaiveNode): void {
		for (const child of node.children) {
			// A load can move a later sibling elsewhere; what has left the tree stays as it is.
			if (!child.collapsible || !this.attached(child)) continue;
			if (child.stale) this.load(child, false);
			if (!child.stale) {
				child.collapsed = false;
				this.expandBelow(child);
			}
		}
	}

	private collapseBelow(node: NaiveNode): void {
		for (const child of node.children) {
			if (child.collapsible) child.collapsed = true;
			this.collapseBelow(child);
		}
	}

	private attached(node: NaiveNode): boolean {
		for (let current = node; current.parent !== null; current = current.parent) {
			if (!current.parent.children.includes(current)) return false;
		}
		return true;
	}

	private prune(): void {
		if (this.focus !== null && this.find(this.focus) === null) this.focus = null;
		if (this.anchor !== null && this.find(this.anchor) === null) this.anchor = null;
		this.selection = this.selection.filter((key) => this.find(key) !== null);
	}
}

type Command =
	| { readonly kind: 'mutate'; readonly edits: readonly Edit[] }
	| { readonly kind: 'echo' }
	| { readonly kind: 'refresh'; readonly pick: number; readonly recursive: boolean }
	| { readonly kind: 'refreshTwice'; readonly pick: number }
	| { readonly kind: 'expand'; readonly pick: number; readonly recursive: boolean }
	| { readonly kind: 'collapse'; readonly pick: number; readonly recursive: boolean }
	| { readonly kind: 'collapseAll' }
	| { readonly kind: 'reveal'; readonly pick: number }
	| { readonly kind: 'focus'; readonly pick: number }
	| { readonly kind: 'anchor'; readonly pick: number }
	| { readonly kind: 'select'; readonly picks: readonly number[] }
	| { readonly kind: 'selectAll' };

/** Shrinking long command sequences can take minutes; a failure reports the smallest case found by then. */
const SHRINK_TIME_LIMIT_MS = 30_000;

/**
 * Vitest fails a test that outlasts its timeout even when it runs synchronously, and a loaded machine
 * can spend several seconds on the runs alone, so a property test gets room for its runs and shrinking.
 */
const PROPERTY_TIMEOUT_MS = 2 * SHRINK_TIME_LIMIT_MS;

// fast-check keeps arrays short unless told otherwise; trees and runs need room to grow.
const seedArbitrary = fc.array(
	fc.record({ expanded: fc.boolean(), folder: fc.boolean(), parent: fc.nat() }),
	{ maxLength: 30, size: 'max' },
);

/** Small picks land on the same few rows again and again, so commands meet on one subtree. */
const pickArbitrary = fc.oneof(fc.nat({ max: 7 }), fc.nat());

const editArbitrary: fc.Arbitrary<Edit> = fc.record({
	decisions: fc.array(fc.nat({ max: 99 }), { maxLength: 12, minLength: 1 }),
	fresh: fc.array(
		fc.record({
			children: fc.nat({ max: 3 }),
			expanded: fc.boolean(),
			folder: fc.boolean(),
			position: fc.nat(),
		}),
		{ maxLength: 3 },
	),
	move: fc.option(fc.nat(), { nil: null }),
	shuffle: fc.boolean(),
	swap: fc.boolean(),
	target: pickArbitrary,
});

// Structural commands dominate: they are where the counts, rows and splices can drift apart.
const commandArbitrary: fc.Arbitrary<Command> = fc.oneof(
	{
		arbitrary: fc.record({
			edits: fc.array(editArbitrary, { maxLength: 3, minLength: 1 }),
			kind: fc.constant('mutate'),
		}),
		weight: 4,
	},
	{ arbitrary: fc.record({ kind: fc.constant('echo') }), weight: 1 },
	{
		arbitrary: fc.record({
			kind: fc.constant('refresh'),
			pick: pickArbitrary,
			recursive: fc.boolean(),
		}),
		weight: 4,
	},
	{ arbitrary: fc.record({ kind: fc.constant('refreshTwice'), pick: pickArbitrary }), weight: 2 },
	{
		arbitrary: fc.record({
			kind: fc.constant('expand'),
			pick: pickArbitrary,
			recursive: fc.boolean(),
		}),
		weight: 4,
	},
	{
		arbitrary: fc.record({
			kind: fc.constant('collapse'),
			pick: pickArbitrary,
			recursive: fc.boolean(),
		}),
		weight: 3,
	},
	{ arbitrary: fc.record({ kind: fc.constant('collapseAll') }), weight: 1 },
	{ arbitrary: fc.record({ kind: fc.constant('reveal'), pick: pickArbitrary }), weight: 1 },
	{ arbitrary: fc.record({ kind: fc.constant('focus'), pick: pickArbitrary }), weight: 1 },
	{ arbitrary: fc.record({ kind: fc.constant('anchor'), pick: pickArbitrary }), weight: 1 },
	{
		arbitrary: fc.record({
			kind: fc.constant('select'),
			picks: fc.array(pickArbitrary, { maxLength: 6 }),
		}),
		weight: 1,
	},
	{ arbitrary: fc.record({ kind: fc.constant('selectAll') }), weight: 1 },
);

function createSource(world: World): VirtualTreeSource<string, Item> {
	return {
		children: (item) => world.childrenOf(item),
		hasChildren: (item) => item.folder,
		key: (item) => item.key,
	};
}

/**
 * Mostly a loaded key, where the interesting state lives; sometimes one the world holds but the model
 * has not loaded, or one nobody holds. Never the input's key.
 */
function pickKey(naive: NaiveTree, world: World, pick: number): string {
	const loaded = naive.nodes().map((node) => node.key);
	const reach = pick % 20;
	if (reach < 15 && loaded.length > 0) return loaded[pick % loaded.length] ?? 'missing';
	if (reach === 19) return 'missing';
	const items = world.items().filter((item) => item !== world.input);
	return items[pick % Math.max(1, items.length)]?.key ?? 'missing';
}

function sorted(keys: Iterable<string>): string[] {
	return [...keys].sort();
}

function rowKeys(model: VirtualTreeModel<string, Item>): string[] {
	return Array.from({ length: model.rowCount }, (_, index) => model.rowAt(index).key);
}

function splicesOf(events: readonly VirtualTreeEvent<string>[]): number {
	return events.reduce(
		(total, event) => total + (event.kind === 'rows' ? event.splices.length : 0),
		0,
	);
}

function expectSameTree(
	model: VirtualTreeModel<string, Item>,
	naive: NaiveTree,
	before: readonly string[],
	events: readonly VirtualTreeEvent<string>[],
): void {
	const rows = naive.rows();
	const after = rowKeys(model);
	expect(after).toEqual(rows.map((node) => node.key));
	for (const [index, node] of rows.entries()) {
		const row = model.rowAt(index);
		const siblings = node.parent?.children ?? [];
		let depth = 0;
		for (let owner = node.parent; owner !== null; owner = owner.parent) depth += 1;
		let below = 0;
		if (!node.collapsed) {
			const count = (current: NaiveNode): void => {
				for (const child of current.children) {
					below += 1;
					if (!child.collapsed) count(child);
				}
			};
			count(node);
		}
		expect(row.element).toBe(node.item);
		expect(row.depth).toBe(depth);
		expect(row.collapsible).toBe(node.collapsible);
		expect(row.collapsible && !row.collapsed).toBe(node.collapsible && !node.collapsed);
		expect(row.parent?.key ?? null).toBe(
			node.parent === naive.root ? null : (node.parent?.key ?? null),
		);
		expect(row.positionInSet).toBe(siblings.indexOf(node) + 1);
		expect(row.setSize).toBe(siblings.length);
		expect(row.visibleDescendantCount).toBe(below);
	}
	for (const node of naive.nodes()) {
		expect(model.indexOf(node.key)).toBe(rows.indexOf(node));
	}
	expect(model.indexOf('missing')).toBe(-1);
	expect(model.focus).toBe(naive.focus);
	expect(model.anchor).toBe(naive.anchor);
	expect(sorted(model.selection)).toEqual(sorted(naive.selection));
	expect(sorted(model.viewState().expanded)).toEqual(
		sorted(
			naive
				.nodes()
				.filter((node) => node.collapsible && !node.collapsed)
				.map((node) => node.key),
		),
	);

	// Replaying the splices on the old rows, reading inserted rows at their final positions, must give
	// the new rows: no change goes unreported and no unchanged row is touched outside a splice.
	const replay = [...before];
	for (const event of events) {
		if (event.kind !== 'rows') continue;
		let end = 0;
		for (const splice of event.splices) {
			expect(splice.start).toBeGreaterThanOrEqual(end);
			replay.splice(
				splice.start,
				splice.deleteCount,
				...after.slice(splice.start, splice.start + splice.insertCount),
			);
			end = splice.start + splice.insertCount;
		}
	}
	expect(replay).toEqual(after);
}

/**
 * Applies a command to the model and the naive tree. Returns the splices a repeated refresh reported,
 * which must be none, and zero for every other command.
 */
function run(
	command: Command,
	world: World,
	model: VirtualTreeModel<string, Item>,
	naive: NaiveTree,
): number {
	switch (command.kind) {
		case 'mutate':
			for (const edit of command.edits) {
				world.mutate(world.folderFor(pickKey(naive, world, edit.target)), edit);
			}
			return 0;
		case 'echo':
			world.echo = !world.echo;
			return 0;
		case 'refresh': {
			const key = command.pick % 5 === 0 ? null : pickKey(naive, world, command.pick);
			void model.refresh(key, { recursive: command.recursive });
			naive.refresh(key, command.recursive);
			return 0;
		}
		case 'refreshTwice': {
			const key = command.pick % 5 === 0 ? null : pickKey(naive, world, command.pick);
			void model.refresh(key);
			naive.refresh(key, false);
			// Nothing changed since the first refresh, so the second must leave the rows alone.
			const events: VirtualTreeEvent<string>[] = [];
			const unsubscribe = model.subscribe((event) => events.push(event));
			void model.refresh(key);
			unsubscribe();
			naive.refresh(key, false);
			return splicesOf(events);
		}
		case 'expand': {
			const key = pickKey(naive, world, command.pick);
			void model.expand(key, { recursive: command.recursive });
			naive.expand(key, command.recursive);
			return 0;
		}
		case 'collapse': {
			const key = pickKey(naive, world, command.pick);
			model.collapse(key, { recursive: command.recursive });
			naive.collapse(key, command.recursive);
			return 0;
		}
		case 'collapseAll':
			model.collapseAll();
			naive.collapseAll();
			return 0;
		case 'reveal': {
			const key = pickKey(naive, world, command.pick);
			model.reveal(key);
			naive.reveal(key);
			return 0;
		}
		case 'focus': {
			const key = command.pick % 4 === 0 ? null : pickKey(naive, world, command.pick);
			model.setFocus(key);
			naive.setFocus(key);
			return 0;
		}
		case 'anchor': {
			const key = command.pick % 4 === 0 ? null : pickKey(naive, world, command.pick);
			model.setAnchor(key);
			naive.setAnchor(key);
			return 0;
		}
		case 'select': {
			const keys = command.picks.map((pick) => pickKey(naive, world, pick));
			model.setSelection(keys);
			naive.setSelection(keys);
			return 0;
		}
		case 'selectAll':
			model.selectAll();
			naive.selectAll();
			return 0;
	}
}

/**
 * The engine's row contract: one splice per expansion change, plus one for each row that a load
 * during the expansion moved away from its old place; any number for other commands.
 */
function spliceLimit(command: Command, moved: number): number {
	switch (command.kind) {
		case 'expand':
			return 1 + moved;
		case 'collapse':
		case 'collapseAll':
		case 'reveal':
			return 1;
		default:
			return Number.POSITIVE_INFINITY;
	}
}

/** Each visible row's parent key. */
function rowParents(model: VirtualTreeModel<string, Item>): Map<string, string | null> {
	const parents = new Map<string, string | null>();
	for (let index = 0; index < model.rowCount; index += 1) {
		const row = model.rowAt(index);
		parents.set(row.key, row.parent?.key ?? null);
	}
	return parents;
}

/** Rows that were visible before a command and now show under another parent or not at all. */
function movedRows(
	model: VirtualTreeModel<string, Item>,
	before: ReadonlyMap<string, string | null>,
): number {
	const after = rowParents(model);
	let moved = 0;
	for (const [key, parent] of before) {
		if (after.get(key) !== parent) moved += 1;
	}
	return moved;
}

function rowSnapshot(tree: VirtualTreeModel<string, Item>): unknown[] {
	return Array.from({ length: tree.rowCount }, (_, index) => {
		const row = tree.rowAt(index);
		return [row.key, row.element, row.depth, row.collapsed];
	});
}

describe('VirtualTreeModel', () => {
	it(
		'matches a naive tree through random edits, refreshes, expansion and traits',
		() => {
			fc.assert(
				fc.property(
					seedArbitrary,
					fc.boolean(),
					fc.array(commandArbitrary, { maxLength: 40, size: 'max' }),
					(seed, defaults, commands) => {
						const world = new World(seed);
						const model = new VirtualTreeModel<string, Item>({
							input: world.input,
							source: createSource(world),
							...(defaults ? { expandedByDefault: (item: Item) => item.expanded } : {}),
						});
						const naive = new NaiveTree(world, defaults);
						const events: VirtualTreeEvent<string>[] = [];
						model.subscribe((event) => events.push(event));
						void model.refresh(null);
						naive.refresh(null, false);
						expectSameTree(model, naive, [], events);
						for (const command of commands) {
							const before = rowKeys(model);
							const parents = rowParents(model);
							events.length = 0;
							expect(run(command, world, model, naive)).toBe(0);
							expectSameTree(model, naive, before, events);
							const limit = spliceLimit(command, movedRows(model, parents));
							expect(splicesOf(events)).toBeLessThanOrEqual(limit);
						}
					},
				),
				{ interruptAfterTimeLimit: SHRINK_TIME_LIMIT_MS, numRuns: 300, seed: 2_026_11_27 },
			);
		},
		PROPERTY_TIMEOUT_MS,
	);

	it(
		'restores a recursively refreshed tree from its view state',
		async () => {
			await fc.assert(
				fc.asyncProperty(
					seedArbitrary,
					fc.array(commandArbitrary, { maxLength: 30, size: 'max' }),
					async (seed, commands) => {
						const world = new World(seed);
						const model = new VirtualTreeModel<string, Item>({
							input: world.input,
							source: createSource(world),
						});
						const naive = new NaiveTree(world, false);
						await model.refresh(null);
						naive.refresh(null, false);
						for (const command of commands) run(command, world, model, naive);
						await model.refresh(null, { recursive: true });

						const restored = new VirtualTreeModel<string, Item>({
							input: world.input,
							source: createSource(world),
						});
						await restored.restoreViewState(model.viewState());
						expect(rowSnapshot(restored)).toEqual(rowSnapshot(model));
						expect(restored.focus).toBe(model.focus);
						expect(restored.anchor).toBe(model.anchor);
						expect(sorted(restored.selection)).toEqual(sorted(model.selection));
					},
				),
				{ interruptAfterTimeLimit: SHRINK_TIME_LIMIT_MS, numRuns: 150, seed: 2_026_11_28 },
			);
		},
		PROPERTY_TIMEOUT_MS,
	);

	it('delivers events in the order operations finish, to the listeners subscribed then', () => {
		const children = new Map<string, readonly string[]>([
			['', ['a', 'b']],
			['a', ['a/1', 'a/2']],
			['b', ['b/1']],
		]);
		const model = new VirtualTreeModel<string, string>({
			input: '',
			source: {
				children: (key) => children.get(key) ?? [],
				hasChildren: (key) => children.has(key),
				key: (key) => key,
			},
		});
		void model.refresh(null);
		const heard: (readonly VirtualTreeSplice[])[] = [];
		const late: VirtualTreeEvent<string>[] = [];
		// Hearing the first expansion, this listener starts another and subscribes a late listener.
		model.subscribe((event) => {
			if (event.kind !== 'rows' || heard.length > 0) return;
			void model.expand('a');
			model.subscribe((lateEvent) => late.push(lateEvent));
		});
		model.subscribe((event) => {
			if (event.kind === 'rows') heard.push(event.splices);
		});
		void model.expand('b');
		expect(heard).toEqual([
			[{ deleteCount: 0, insertCount: 1, start: 2 }],
			[{ deleteCount: 0, insertCount: 2, start: 1 }],
		]);
		expect(late).toEqual([]);
	});
});
