import fc from 'fast-check';
import { describe, expect, it, vi } from 'vitest';

import { VirtualTreeModel } from '../VirtualTreeModel';
import type {
	VirtualTreeEvent,
	VirtualTreeFindQuery,
	VirtualTreeHighlight,
	VirtualTreeRow,
	VirtualTreeSource,
	VirtualTreeSplice,
} from '../virtual-tree-types';

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

	/** Adds more files than one slice of a find matches. */
	grow(folder: Item): void {
		for (let index = 0; index < 70; index += 1)
			this.list(folder.key).push(this.create(false, false));
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

/**
 * Whether an item may join its parent's row: two items in three, and an update can change it, so a
 * refresh both forms and splits compact rows.
 */
function isCompressible(item: Item): boolean {
	return (Number(item.key.slice(1)) + item.version) % 3 !== 0;
}

/**
 * The names find patterns match: a refresh that bumps an item's version can rename it. The long ones
 * keep matching as typing extends a pattern.
 */
const NAMES = [
	'ab',
	'ba',
	'abc',
	'ca',
	'b',
	'aab',
	'cb',
	'c',
	'abcab',
	'bcabca',
	'cabbac',
	'aabbcc',
];

function nameOf(item: Item): string {
	return NAMES[(Number(item.key.slice(1)) + item.version) % NAMES.length] ?? '';
}

/** A find label, computed from a row by the model and from a compact row's items by the naive tree. */
interface Labeling {
	readonly row: (row: VirtualTreeRow<string, Item>) => string;
	readonly items: (items: readonly Item[]) => string;
}

function joinedNames(items: readonly Item[]): string {
	return items.map(nameOf).join('/');
}

function reversedNames(items: readonly Item[]): string {
	return items.map(nameOf).reverse().join('\\').toUpperCase();
}

const NAME_LABEL: Labeling = {
	items: joinedNames,
	row: (row) => joinedNames(row.segments.map(({ element }) => element)),
};

/** Another label, so a find that switches to it must match every item again. */
const REVERSED_LABEL: Labeling = {
	items: reversedNames,
	row: (row) => reversedNames(row.segments.map(({ element }) => element)),
};

/** Which rows a find provider leaves visible, as the model and the naive tree ask it. */
interface Visibility {
	readonly row: (row: VirtualTreeRow<string, Item>) => boolean;
	readonly key: (key: string) => boolean;
}

function keyVisibility(keeps: (index: number) => boolean): Visibility {
	return {
		key: (key) => keeps(Number(key.slice(1))),
		row: (row) => keeps(Number(row.key.slice(1))),
	};
}

const VISIBILITIES: readonly (Visibility | undefined)[] = [
	undefined,
	keyVisibility((index) => index % 3 !== 0),
	keyVisibility((index) => index % 4 !== 1),
];

/** Whether a label matches, by the definition and not the scoring: in order, or together, ignoring case. */
function matchesPattern(query: VirtualTreeFindQuery<string, Item>, label: string): boolean {
	const pattern = query.pattern.toLowerCase();
	const text = label.toLowerCase();
	if (query.matchType === 'contiguous') return text.includes(pattern);
	let next = 0;
	for (let index = 0; index < text.length && next < pattern.length; index += 1) {
		if (text[index] === pattern[next]) next += 1;
	}
	return next === pattern.length;
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
 * no counts, no index caches, no splices, and compact rows derived from the tree whenever asked.
 */
class NaiveTree {
	readonly root: NaiveNode;
	focus: string | null = null;
	anchor: string | null = null;
	selection: string[] = [];
	/** The find the rows show, which the test takes from the model after checking it. */
	query: VirtualTreeFindQuery<string, Item> | null = null;
	/** The find the test asked for last. */
	pending: VirtualTreeFindQuery<string, Item> | null = null;
	private compressed: boolean;
	private focusAssigned = false;
	private focusMergedAtRest = false;
	private readonly world: World;
	private readonly defaults: boolean;

	constructor(world: World, defaults: boolean, compressed: boolean) {
		this.world = world;
		this.defaults = defaults;
		this.compressed = compressed;
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

	/** Whether a node shows in the row of its only child. */
	merged(node: NaiveNode): boolean {
		const only = node.children[0];
		return (
			this.compressed &&
			node !== this.root &&
			node.collapsible &&
			!node.collapsed &&
			node.children.length === 1 &&
			only !== undefined &&
			isCompressible(only.item)
		);
	}

	/** The node whose row a node shows in. */
	tail(node: NaiveNode): NaiveNode {
		let tail = node;
		while (this.merged(tail)) {
			const only = tail.children[0];
			if (only === undefined) break;
			tail = only;
		}
		return tail;
	}

	/** The nodes a row shows, outermost first. */
	segments(node: NaiveNode): NaiveNode[] {
		const chain = [node];
		for (let owner = node.parent; owner !== null && this.merged(owner); owner = owner.parent) {
			chain.unshift(owner);
		}
		return chain;
	}

	rows(): NaiveNode[] {
		const shown = this.shown();
		const result: NaiveNode[] = [];
		const visit = (node: NaiveNode): void => {
			for (const child of node.children) {
				if (!shown.has(child)) continue;
				if (!this.merged(child)) result.push(child);
				if (!child.collapsed) visit(child);
			}
		};
		visit(this.root);
		return result;
	}

	/** The label the applied find reads from a row: the names of every item the row shows. */
	label(node: NaiveNode): string {
		const labeling = this.query?.label === REVERSED_LABEL.row ? REVERSED_LABEL : NAME_LABEL;
		return labeling.items(this.segments(node).map(({ item }) => item));
	}

	/** Whether the applied find matches a row's label; a merged node has no row of its own. */
	matches(node: NaiveNode): boolean {
		const query = this.query;
		return query !== null && !this.merged(node) && matchesPattern(query, this.label(node));
	}

	/**
	 * The nodes that show under the applied find, as VS Code's tree filter decides: while filtering, a
	 * row shows when it matches or a row below it shows, unless `visible` hides it, and a merged node
	 * shows with its only child.
	 */
	shown(): Set<NaiveNode> {
		const query = this.query;
		const shown = new Set<NaiveNode>();
		const visit = (node: NaiveNode): boolean => {
			let below = false;
			for (const child of node.children) {
				if (visit(child)) below = true;
			}
			let visible = true;
			if (query !== null && query.filter) {
				if (this.merged(node)) visible = below;
				else visible = !this.hiddenByProvider(node) && (this.matches(node) || below);
			}
			if (visible) shown.add(node);
			return visible;
		};
		visit(this.root);
		return shown;
	}

	/** The items the applied find matched and looked at: a compact row once, none below a hidden row. */
	findCounts(): { matchCount: number; totalCount: number } {
		let matchCount = 0;
		let totalCount = 0;
		const visit = (node: NaiveNode): void => {
			for (const child of node.children) {
				if (!this.merged(child)) {
					totalCount += 1;
					if (this.matches(child)) matchCount += 1;
					if (this.hiddenByProvider(child)) continue;
				}
				visit(child);
			}
		};
		visit(this.root);
		return { matchCount, totalCount };
	}

	find(key: string): NaiveNode | null {
		return this.nodes().find((node) => node.key === key) ?? null;
	}

	refresh(key: string | null, recursive: boolean): void {
		const node = key === null ? this.root : this.find(key);
		if (node !== null && node !== this.root && node.collapsed) {
			node.collapsible = node.item.folder;
			node.children = [];
			node.stale = node.collapsible;
		} else if (node !== null) {
			this.refreshSubtree(node, recursive);
		}
		this.settle();
	}

	expand(key: string, recursive: boolean): void {
		const found = this.find(key);
		const node = found === null ? null : this.tail(found);
		if (node?.collapsible === true) this.expandNode(node, recursive);
		this.settle();
	}

	collapse(key: string, recursive: boolean): void {
		const found = this.find(key);
		const node = found === null ? null : this.tail(found);
		if (node?.collapsible === true) node.collapsed = true;
		if (node !== null && recursive) this.collapseBelow(node);
		this.settle();
	}

	collapseAll(): void {
		this.collapseBelow(this.root);
		this.settle();
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
		this.settle();
	}

	setCompressed(compressed: boolean): void {
		this.compressed = compressed;
		this.settle();
	}

	setFocus(key: string | null): void {
		if ((key === null || this.find(key) !== null) && key !== this.focus) {
			this.focus = key;
			this.focusAssigned = true;
		}
		this.settle();
	}

	setAnchor(key: string | null): void {
		if (key === null) this.anchor = null;
		const node = key === null ? null : this.find(key);
		if (node !== null) this.anchor = this.tail(node).key;
		this.settle();
	}

	setSelection(keys: readonly string[]): void {
		const next: string[] = [];
		for (const key of keys) {
			const node = this.find(key);
			if (node !== null) next.push(this.tail(node).key);
		}
		this.selection = [...new Set(next)];
		this.settle();
	}

	selectAll(): void {
		this.selection = this.rows().map((node) => node.key);
		this.settle();
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
		this.load(node, recursive);
	}

	/**
	 * Takes the source's children, refreshes the expanded ones it kept, and expands an only child the
	 * node comes to share its row with, fetching that child's children again when it had some.
	 */
	private load(node: NaiveNode, recursive: boolean): void {
		const joined = this.merged(node) ? node.children[0] : undefined;
		const toRefresh = this.merge(node, this.world.childrenOf(node.item), recursive);
		node.stale = false;
		for (const child of toRefresh) {
			if (this.attached(child)) this.refreshSubtree(child, recursive);
		}
		const only = this.merged(node) ? node.children[0] : undefined;
		if (only !== undefined && only !== joined && only.collapsible && only.collapsed) {
			const stale = only.stale;
			this.expandNode(only, false);
			if (!stale) this.refreshSubtree(only, recursive);
		}
	}

	private expandNode(node: NaiveNode, recursive: boolean): void {
		if (node.stale) this.load(node, false);
		if (recursive) this.expandBelow(node);
		node.collapsed = false;
		const only = this.merged(node) ? node.children[0] : undefined;
		if (only?.collapsible === true && only.collapsed) this.expandNode(only, false);
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

	/** Collapses every row below; an item a compact row joins stays expanded. */
	private collapseBelow(node: NaiveNode): void {
		for (const child of node.children) {
			if (child.collapsible && !this.merged(child)) child.collapsed = true;
			this.collapseBelow(child);
		}
	}

	/** Whether the applied find's `visible` hides a row, which it does only while filtering. */
	private hiddenByProvider(node: NaiveNode): boolean {
		const query = this.query;
		if (query === null || !query.filter || this.merged(node)) return false;
		const visibility = VISIBILITIES.find((candidate) => candidate?.row === query.visible);
		return visibility !== undefined && !visibility.key(node.key);
	}

	private attached(node: NaiveNode): boolean {
		for (let current = node; current.parent !== null; current = current.parent) {
			if (!current.parent.children.includes(current)) return false;
		}
		return true;
	}

	/**
	 * What the model does as an operation ends: traits on keys that left the tree go, selection and
	 * anchor move to the row an item joined, and so does focus when the operation did not set it and
	 * it did not rest on a joined item already.
	 */
	private settle(): void {
		const selection: string[] = [];
		for (const key of this.selection) {
			const node = this.find(key);
			if (node !== null) selection.push(this.tail(node).key);
		}
		this.selection = [...new Set(selection)];
		const anchor = this.anchor === null ? null : this.find(this.anchor);
		this.anchor = anchor === null ? null : this.tail(anchor).key;
		const focus = this.focus === null ? null : this.find(this.focus);
		if (focus === null) this.focus = null;
		else if (this.merged(focus) && !this.focusAssigned && !this.focusMergedAtRest) {
			this.focus = this.tail(focus).key;
		}
		this.focusAssigned = false;
		const rested = this.focus === null ? null : this.find(this.focus);
		this.focusMergedAtRest = rested !== null && this.merged(rested);
	}
}

type Command =
	| { readonly kind: 'mutate'; readonly edits: readonly Edit[] }
	/** Adds more top-level files than one slice of a find matches, and refreshes them in. */
	| { readonly kind: 'grow' }
	| { readonly kind: 'echo' }
	| { readonly kind: 'refresh'; readonly pick: number; readonly recursive: boolean }
	| { readonly kind: 'refreshTwice'; readonly pick: number }
	| {
			readonly kind: 'expand';
			/** Focuses the item first, as a click on its twistie or a key on its row does. */
			readonly focus: boolean;
			readonly pick: number;
			readonly recursive: boolean;
	  }
	| { readonly kind: 'collapse'; readonly pick: number; readonly recursive: boolean }
	| { readonly kind: 'collapseAll' }
	| { readonly kind: 'reveal'; readonly pick: number }
	| { readonly kind: 'focus'; readonly pick: number }
	| { readonly kind: 'anchor'; readonly pick: number }
	| { readonly kind: 'select'; readonly picks: readonly number[] }
	| { readonly kind: 'selectAll' }
	| { readonly kind: 'compress' }
	| {
			readonly kind: 'find';
			/** Typed after the pattern last asked for when `extend` holds; the whole pattern otherwise. */
			readonly text: string;
			readonly extend: boolean;
			/** Switches between fuzzy and contiguous matching. */
			readonly switchMatch: boolean;
			/** Switches between the two labels. */
			readonly switchLabel: boolean;
			readonly filter: boolean;
			/** An index into `VISIBILITIES`. */
			readonly visible: number;
			/** Runs every waiting task afterwards, so the find applies. */
			readonly settle: boolean;
	  }
	/** Runs the next waiting task, such as a slice of a find. */
	| { readonly kind: 'tick' };

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
	{ arbitrary: fc.record({ kind: fc.constant('grow') }), weight: 1 },
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
			focus: fc.boolean(),
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
	{ arbitrary: fc.record({ kind: fc.constant('compress') }), weight: 1 },
	{
		// Mostly typing on, as a person does, so most finds look only at the previous one's matches.
		arbitrary: fc.record({
			extend: fc.oneof({ arbitrary: fc.constant(true), weight: 3 }, fc.constant(false)),
			filter: fc.boolean(),
			kind: fc.constant('find'),
			settle: fc.oneof(fc.constant(true), { arbitrary: fc.constant(false), weight: 2 }),
			switchLabel: fc.oneof({ arbitrary: fc.constant(false), weight: 4 }, fc.constant(true)),
			switchMatch: fc.oneof({ arbitrary: fc.constant(false), weight: 4 }, fc.constant(true)),
			text: fc.string({ maxLength: 2, unit: fc.constantFrom('a', 'b', 'c', 'B', '/') }),
			visible: fc.nat({ max: VISIBILITIES.length - 1 }),
		}),
		weight: 5,
	},
	{ arbitrary: fc.record({ kind: fc.constant('tick') }), weight: 2 },
);

/** The commands that leave a find untouched, for tests that compare rows with another model's. */
function findsNothing(command: Command): boolean {
	return command.kind !== 'find' && command.kind !== 'tick';
}

function createSource(world: World): VirtualTreeSource<string, Item> {
	return {
		children: (item) => world.childrenOf(item),
		compressible: isCompressible,
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
	const shown = naive.shown();
	const after = rowKeys(model);
	expect(after).toEqual(rows.map((node) => node.key));
	// Each row's facts gathered into one comparison: one `expect` per row is slow on large trees.
	const actual: unknown[] = [];
	const expected: unknown[] = [];
	for (const [index, node] of rows.entries()) {
		const row = model.rowAt(index);
		const segments = naive.segments(node);
		const head = segments[0] ?? node;
		const siblings = (head.parent?.children ?? []).filter((sibling) => shown.has(sibling));
		let depth = 0;
		let level = 0;
		for (let owner = node.parent; owner !== null; owner = owner.parent) {
			level += 1;
			if (!naive.merged(owner)) depth += 1;
		}
		let below = 0;
		if (!node.collapsed) {
			const count = (current: NaiveNode): void => {
				for (const child of current.children) {
					if (!shown.has(child)) continue;
					if (!naive.merged(child)) below += 1;
					if (!child.collapsed) count(child);
				}
			};
			count(node);
		}
		const label = naive.label(node);
		const contiguous = naive.query?.matchType === 'contiguous';
		actual.push({
			collapsible: row.collapsible,
			depth: row.depth,
			element: row.element === node.item,
			expanded: row.collapsible && !row.collapsed,
			highlights: readHighlights(model.highlightsOf(row), label, contiguous),
			key: row.key,
			level: row.level,
			matched: model.matchesFind(row),
			parent: row.parent?.key ?? null,
			positionInSet: row.positionInSet,
			segments: row.segments.map((segment) => segment.key),
			setSize: row.setSize,
			visibleDescendantCount: row.visibleDescendantCount,
		});
		const matched = naive.matches(node);
		expected.push({
			collapsible: node.collapsible,
			depth,
			element: true,
			expanded: node.collapsible && !node.collapsed,
			highlights: matched ? expectedHighlights(naive, label) : '',
			key: node.key,
			level,
			matched,
			parent: head.parent === naive.root ? null : (head.parent?.key ?? null),
			positionInSet: siblings.indexOf(head) + 1,
			segments: segments.map(({ key }) => key),
			setSize: siblings.length,
			visibleDescendantCount: below,
		});
	}
	expect(actual).toEqual(expected);
	const query = naive.query;
	expect(model.findStatus?.query ?? null).toBe(query);
	expect(model.findStatus).toEqual(query === null ? null : { ...naive.findCounts(), query });
	const rowIndex = new Map(rows.map((node, index) => [node, index]));
	const nodes = naive.nodes();
	expect(nodes.map((node) => model.indexOf(node.key))).toEqual(
		nodes.map((node) => rowIndex.get(naive.tail(node)) ?? -1),
	);
	expect(model.indexOf('missing')).toBe(-1);
	expect(model.focus).toBe(naive.focus);
	expect(model.anchor).toBe(naive.anchor);
	expect(sorted(model.selection)).toEqual(sorted(naive.selection));
	// An expanded row names every item it shows, and a collapsed one names none.
	expect(sorted(model.viewState().expanded)).toEqual(
		sorted(
			nodes
				.filter((node) => !naive.merged(node) && node.collapsible && !node.collapsed)
				.flatMap((node) => naive.segments(node).map(({ key }) => key)),
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
 * What a row's highlights spell, ignoring case, after the position a contiguous match starts at;
 * empty without highlights. Ranges must ascend with gaps between them, as adjacent ones join.
 */
function readHighlights(
	highlights: readonly VirtualTreeHighlight[],
	label: string,
	contiguous: boolean,
): string {
	let spelled = '';
	let end = -1;
	for (const highlight of highlights) {
		if (highlight.start <= end || highlight.end <= highlight.start) return 'misordered ranges';
		spelled += label.slice(highlight.start, highlight.end);
		end = highlight.end;
	}
	const first = highlights[0];
	if (first === undefined) return '';
	return contiguous ? `${first.start}:${spelled.toLowerCase()}` : spelled.toLowerCase();
}

/**
 * How a matched row's highlights must read: the pattern itself, since every character of it is
 * highlighted, and for a contiguous match at the pattern's first occurrence.
 */
function expectedHighlights(naive: NaiveTree, label: string): string {
	const query = naive.query;
	if (query === null) return '';
	const pattern = query.pattern.toLowerCase();
	if (query.matchType !== 'contiguous') return pattern;
	return `${label.toLowerCase().indexOf(pattern)}:${pattern}`;
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
		case 'grow':
			if (naive.nodes().length < 70) world.grow(world.input);
			void model.refresh(null);
			naive.refresh(null, false);
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
			if (command.focus) {
				model.setFocus(key);
				naive.setFocus(key);
			}
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
		case 'compress':
			model.setCompressed(!model.compressed);
			naive.setCompressed(model.compressed);
			return 0;
		case 'find': {
			const previous = naive.pending;
			const base = command.extend ? (previous?.pattern ?? '') : '';
			const pattern = `${base}${command.text}`.slice(0, 6);
			const fuzzy = (previous?.matchType !== 'contiguous') !== command.switchMatch;
			const reversed = (previous?.label === REVERSED_LABEL.row) !== command.switchLabel;
			const query: VirtualTreeFindQuery<string, Item> | null =
				pattern === ''
					? null
					: {
							filter: command.filter,
							label: (reversed ? REVERSED_LABEL : NAME_LABEL).row,
							matchType: fuzzy ? 'fuzzy' : 'contiguous',
							pattern,
							visible: VISIBILITIES[command.visible]?.row,
						};
			naive.pending = query;
			model.find(query);
			if (command.settle) vi.runAllTimers();
			return 0;
		}
		case 'tick':
			vi.advanceTimersToNextTimer();
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
		case 'compress':
			return 1;
		default:
			return Number.POSITIVE_INFINITY;
	}
}

/** What a renderer drew for each row: its revision and the highlights it showed. */
function drawnRows(
	model: VirtualTreeModel<string, Item>,
): Map<VirtualTreeRow<string, Item>, readonly [revision: number, highlights: string]> {
	const drawn = new Map<VirtualTreeRow<string, Item>, readonly [number, string]>();
	for (let index = 0; index < model.rowCount; index += 1) {
		const row = model.rowAt(index);
		drawn.set(row, [row.revision, JSON.stringify(model.highlightsOf(row))]);
	}
	return drawn;
}

/** A row whose highlights changed has a new revision, so a renderer that skips drawn revisions redraws it. */
function expectRedrawn(
	model: VirtualTreeModel<string, Item>,
	drawn: ReadonlyMap<VirtualTreeRow<string, Item>, readonly [revision: number, highlights: string]>,
): void {
	for (let index = 0; index < model.rowCount; index += 1) {
		const row = model.rowAt(index);
		const before = drawn.get(row);
		if (before === undefined || before[1] === JSON.stringify(model.highlightsOf(row))) continue;
		expect(row.revision).not.toBe(before[0]);
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
		return [row.key, row.element, row.depth, row.collapsed, row.segments.length];
	});
}

/** A request the asynchronous source has not answered. */
interface PendingRequest {
	readonly item: Item;
	readonly signal: AbortSignal;
	/** The fake clock when the model asked. */
	readonly askedAt: number;
	/** The children when the model asked; a read may see them as they were then or as they are later. */
	readonly snapshot: readonly Item[];
	readonly resolve: (children: readonly Item[]) => void;
	readonly reject: (error: unknown) => void;
}

type AsyncCommand =
	| { readonly kind: 'mutate'; readonly edits: readonly Edit[] }
	| { readonly kind: 'refresh'; readonly pick: number; readonly recursive: boolean }
	| { readonly kind: 'expand'; readonly pick: number; readonly recursive: boolean }
	| { readonly kind: 'collapse'; readonly pick: number; readonly recursive: boolean }
	| { readonly kind: 'collapseAll' }
	| { readonly kind: 'answer'; readonly pick: number; readonly current: boolean }
	| { readonly kind: 'fail'; readonly pick: number }
	| { readonly kind: 'wait'; readonly milliseconds: number };

const LOADING_DELAY_MS = 800;

const asyncCommandArbitrary: fc.Arbitrary<AsyncCommand> = fc.oneof(
	{
		arbitrary: fc.record({
			edits: fc.array(editArbitrary, { maxLength: 2, minLength: 1 }),
			kind: fc.constant('mutate'),
		}),
		weight: 2,
	},
	{
		arbitrary: fc.record({
			kind: fc.constant('refresh'),
			pick: pickArbitrary,
			recursive: fc.boolean(),
		}),
		weight: 3,
	},
	{
		arbitrary: fc.record({
			kind: fc.constant('expand'),
			pick: pickArbitrary,
			recursive: fc.boolean(),
		}),
		weight: 3,
	},
	{
		arbitrary: fc.record({
			kind: fc.constant('collapse'),
			pick: pickArbitrary,
			recursive: fc.boolean(),
		}),
		weight: 2,
	},
	{ arbitrary: fc.record({ kind: fc.constant('collapseAll') }), weight: 1 },
	{
		arbitrary: fc.record({ current: fc.boolean(), kind: fc.constant('answer'), pick: fc.nat() }),
		weight: 6,
	},
	{ arbitrary: fc.record({ kind: fc.constant('fail'), pick: fc.nat() }), weight: 1 },
	{
		arbitrary: fc.record({
			kind: fc.constant('wait'),
			milliseconds: fc.oneof(
				fc.constantFrom(LOADING_DELAY_MS - 1, LOADING_DELAY_MS, LOADING_DELAY_MS + 1),
				fc.nat({ max: 1_000 }),
			),
		}),
		weight: 2,
	},
);

/** Mostly a visible row's key; sometimes one the model has not loaded, or one nobody holds. */
function pickRowKey(model: VirtualTreeModel<string, Item>, world: World, pick: number): string {
	const reach = pick % 20;
	if (reach < 15 && model.rowCount > 0) return model.rowAt(pick % model.rowCount).key;
	if (reach === 19) return 'missing';
	const items = world.items().filter((item) => item !== world.input);
	return items[pick % Math.max(1, items.length)]?.key ?? 'missing';
}

/** Lets every promise reaction run; the fake timers leave `setImmediate` real. */
function flush(): Promise<void> {
	return new Promise((resolve) => {
		setImmediate(resolve);
	});
}

/** Drives a model whose source answers only when a command says so. */
class AsyncRun {
	readonly world: World;
	readonly model: VirtualTreeModel<string, Item>;
	readonly pending: PendingRequest[] = [];
	readonly events: VirtualTreeEvent<string>[] = [];
	/** Whether each operation the run started has settled. */
	private readonly operations: { settled: boolean }[] = [];
	private poisoned = 0;

	constructor(seed: readonly SeedItem[], compressed: boolean) {
		this.world = new World(seed);
		this.model = new VirtualTreeModel<string, Item>({
			compressed,
			input: this.world.input,
			source: {
				children: (item, signal) =>
					new Promise((resolve, reject) => {
						this.pending.push({
							askedAt: Date.now(),
							item,
							reject,
							resolve,
							signal,
							snapshot: this.world.childrenOf(item),
						});
					}),
				compressible: isCompressible,
				hasChildren: (item) => item.folder,
				key: (item) => item.key,
			},
		});
		this.model.subscribe((event) => this.events.push(event));
	}

	get settled(): boolean {
		return this.operations.every((operation) => operation.settled);
	}

	track(operation: Promise<void>): void {
		const record = { settled: false };
		this.operations.push(record);
		const settle = (): void => {
			record.settled = true;
		};
		operation.then(settle, settle);
	}

	/**
	 * Answers with the children as they were when the model asked, or with `current` as the world holds
	 * them now; a request nobody wants any more gets a poisoned child instead, which must never show.
	 */
	answer(request: PendingRequest, current: boolean): void {
		this.pending.splice(this.pending.indexOf(request), 1);
		if (!request.signal.aborted) {
			request.resolve(current ? this.world.childrenOf(request.item) : request.snapshot);
			return;
		}
		this.poisoned += 1;
		request.resolve([
			{ expanded: false, folder: false, key: `poison-${this.poisoned}`, version: 0 },
		]);
	}

	async run(command: AsyncCommand): Promise<void> {
		const { model, pending, world } = this;
		switch (command.kind) {
			case 'mutate':
				for (const edit of command.edits) {
					world.mutate(world.folderFor(pickRowKey(model, world, edit.target)), edit);
				}
				return;
			case 'refresh': {
				const key = command.pick % 5 === 0 ? null : pickRowKey(model, world, command.pick);
				this.track(model.refresh(key, { recursive: command.recursive }));
				return;
			}
			case 'expand':
				this.track(
					model.expand(pickRowKey(model, world, command.pick), { recursive: command.recursive }),
				);
				return;
			case 'collapse':
				model.collapse(pickRowKey(model, world, command.pick), { recursive: command.recursive });
				return;
			case 'collapseAll':
				model.collapseAll();
				return;
			case 'answer': {
				const request = pending[command.pick % Math.max(1, pending.length)];
				if (request !== undefined) this.answer(request, command.current);
				return;
			}
			case 'fail': {
				const request = pending[command.pick % Math.max(1, pending.length)];
				if (request === undefined) return;
				pending.splice(pending.indexOf(request), 1);
				request.reject(new Error('unreadable'));
				return;
			}
			case 'wait':
				await vi.advanceTimersByTimeAsync(command.milliseconds);
		}
	}
}

/**
 * What must hold after every step: one live request per key, owned by the input or an expanded row;
 * no poisoned row; a row loading exactly while its live request has waited the indicator's delay;
 * consistent row structure; and splices that replay the old rows into the new ones.
 */
function expectAsyncInvariants(state: AsyncRun, before: readonly string[]): void {
	const { model, pending, world } = state;
	const live = pending.filter((request) => !request.signal.aborted);
	const liveKeys = live.map((request) => request.item.key);
	expect(new Set(liveKeys).size).toBe(liveKeys.length);
	const now = Date.now();
	// Items a compact row joins stay expanded while the row collapses, which then names none of them.
	const joined = new Set<string>();
	for (let index = 0; index < model.rowCount; index += 1) {
		const row = model.rowAt(index);
		expect(row.key.startsWith('poison')).toBe(false);
		for (const segment of row.segments) {
			expect(segment.key.startsWith('poison')).toBe(false);
			expect(model.indexOf(segment.key)).toBe(index);
			if (segment !== row) joined.add(segment.key);
		}
		const { parent } = row;
		expect(row.depth).toBe(parent === null ? 1 : parent.depth + 1);
		expect(row.level).toBe((parent === null ? 0 : parent.level) + row.segments.length);
		expect(parent === null ? -1 : model.indexOf(parent.key)).toBeLessThan(index);
		expect(parent?.collapsed ?? false).toBe(false);
		// The rows the descendant count covers lie deeper than this one, and the next row does not.
		const end = index + row.visibleDescendantCount;
		for (let below = index + 1; below <= end; below += 1) {
			expect(model.rowAt(below).depth).toBeGreaterThan(row.depth);
		}
		const next = end + 1 < model.rowCount ? model.rowAt(end + 1).depth : 1;
		expect(next).toBeLessThanOrEqual(row.depth);
		const waited = live.some(
			(request) => request.item.key === row.key && now - request.askedAt >= LOADING_DELAY_MS,
		);
		expect(row.loading).toBe(waited);
	}
	// Collapsing aborts and leaving the tree aborts, so only the input and expanded rows load. The view
	// state leaves out the items of collapsed compact rows, which a collapsed parent may hide too.
	const expanded = new Set(model.viewState().expanded);
	for (const key of liveKeys) {
		const allowed =
			key === world.input.key ||
			expanded.has(key) ||
			joined.has(key) ||
			(model.compressed && model.indexOf(key) < 0);
		expect(allowed).toBe(true);
	}
	expect(replayRows(before, state.events, model)).toEqual(rowKeys(model));
}

/** The rows `before` turns into when the reported splices apply, reading inserts from the model. */
function replayRows(
	before: readonly string[],
	events: readonly VirtualTreeEvent<string>[],
	model: VirtualTreeModel<string, Item>,
): string[] {
	const after = rowKeys(model);
	const replay = [...before];
	for (const event of events) {
		if (event.kind !== 'rows') continue;
		for (const splice of event.splices) {
			replay.splice(
				splice.start,
				splice.deleteCount,
				...after.slice(splice.start, splice.start + splice.insertCount),
			);
		}
	}
	return replay;
}

/**
 * The rows the world shows with the model's expanded items expanded: every item a row shows when the
 * row is expanded, and every one but the last when it is collapsed. With compact rows on, an expanded
 * folder whose only child is compressible shows in that child's row.
 */
function worldRows(world: World, model: VirtualTreeModel<string, Item>): Item[] {
	const expanded = new Set<string>();
	for (let index = 0; index < model.rowCount; index += 1) {
		const row = model.rowAt(index);
		const { segments } = row;
		for (const [position, segment] of segments.entries()) {
			if (position < segments.length - 1 || (row.collapsible && !row.collapsed)) {
				expanded.add(segment.key);
			}
		}
	}
	const childrenOf = (item: Item): Item[] => {
		const seen = new Set<string>();
		return world.childrenOf(item).filter((child) => !seen.has(child.key) && seen.add(child.key));
	};
	const rows: Item[] = [];
	const visit = (item: Item): void => {
		for (const child of childrenOf(item)) {
			const open = child.folder && expanded.has(child.key);
			const own = open ? childrenOf(child) : [];
			const only = own.length === 1 ? own[0] : undefined;
			if (!(model.compressed && only !== undefined && isCompressible(only))) rows.push(child);
			if (open) visit(child);
		}
	};
	visit(world.input);
	return rows;
}

/**
 * Answers every request with the children as they were when asked, picking each from the unanswered
 * ones by the next pick, and the oldest once the picks run out.
 */
async function answerAll(state: AsyncRun, picks: readonly number[]): Promise<void> {
	for (let round = 0; state.pending.length > 0; round += 1) {
		expect(round).toBeLessThan(10_000);
		const before = rowKeys(state.model);
		state.events.length = 0;
		const request = state.pending[(picks[round] ?? 0) % state.pending.length];
		if (request !== undefined) state.answer(request, false);
		await flush();
		expectAsyncInvariants(state, before);
	}
}

/** Every operation has settled, and the rows hold the world's items under the model's expansion. */
function expectWorldRows(state: AsyncRun): void {
	expect(state.settled).toBe(true);
	const expected = worldRows(state.world, state.model);
	expect(rowKeys(state.model)).toEqual(expected.map((item) => item.key));
	for (const [index, item] of expected.entries()) {
		const row = state.model.rowAt(index);
		expect(row.element).toBe(item);
		expect(row.collapsible).toBe(item.folder);
		expect(row.loading).toBe(false);
	}
}

describe('VirtualTreeModel', () => {
	it(
		'matches a naive tree through random edits, refreshes, expansion and traits',
		() => {
			fc.assert(
				fc.property(
					seedArbitrary,
					fc.boolean(),
					fc.boolean(),
					fc.array(commandArbitrary, { maxLength: 40, size: 'max' }),
					(seed, defaults, compressed, commands) => {
						// Every reading of the clock moves it past a find slice, so a slice matches one batch
						// of labels and each find waits for a task before it applies; the commands between
						// meet it while it runs.
						let clock = 0;
						const now = vi.spyOn(performance, 'now').mockImplementation(() => {
							clock += 10;
							return clock;
						});
						vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
						try {
							const world = new World(seed);
							const model = new VirtualTreeModel<string, Item>({
								compressed,
								input: world.input,
								source: createSource(world),
								...(defaults ? { expandedByDefault: (item: Item) => item.expanded } : {}),
							});
							const naive = new NaiveTree(world, defaults, compressed);
							const events: VirtualTreeEvent<string>[] = [];
							model.subscribe((event) => events.push(event));
							void model.refresh(null);
							naive.refresh(null, false);
							expectSameTree(model, naive, [], events);
							for (const command of commands) {
								const before = rowKeys(model);
								const parents = rowParents(model);
								const drawn = drawnRows(model);
								events.length = 0;
								expect(run(command, world, model, naive)).toBe(0);
								// The rows show the find they showed or the latest one asked for, never another,
								// and the latest one once no task waits.
								const shown = model.findStatus?.query ?? null;
								expect(shown === naive.query || shown === naive.pending).toBe(true);
								expect(vi.getTimerCount() > 0 || shown === naive.pending).toBe(true);
								naive.query = shown;
								expectSameTree(model, naive, before, events);
								expectRedrawn(model, drawn);
								const limit = spliceLimit(command, movedRows(model, parents));
								expect(splicesOf(events)).toBeLessThanOrEqual(limit);
							}
							model.dispose();
						} finally {
							vi.useRealTimers();
							now.mockRestore();
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
					// Compact rows stay off: an item that joins a row on restore expands without being saved.
					fc.array(
						commandArbitrary.filter(
							(command) => command.kind !== 'compress' && findsNothing(command),
						),
						{ maxLength: 30, size: 'max' },
					),
					async (seed, commands) => {
						const world = new World(seed);
						const model = new VirtualTreeModel<string, Item>({
							compressed: false,
							input: world.input,
							source: createSource(world),
						});
						const naive = new NaiveTree(world, false, false);
						await model.refresh(null);
						naive.refresh(null, false);
						for (const command of commands) run(command, world, model, naive);
						await model.refresh(null, { recursive: true });

						const restored = new VirtualTreeModel<string, Item>({
							compressed: false,
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

	it(
		'loads asynchronously with one request per row, drops unwanted answers, and rests on the source data',
		async () => {
			await fc.assert(
				fc.asyncProperty(
					seedArbitrary,
					fc.array(asyncCommandArbitrary, { maxLength: 40, size: 'max' }),
					fc.constantFrom('plain', 'recursive', 'recursive behind plain'),
					fc.array(fc.nat(), { maxLength: 60 }),
					fc.boolean(),
					async (seed, commands, last, picks, compressed) => {
						vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'Date'] });
						try {
							const state = new AsyncRun(seed, compressed);
							state.track(state.model.refresh(null));
							await flush();
							for (const command of commands) {
								const before = rowKeys(state.model);
								state.events.length = 0;
								await state.run(command);
								await flush();
								expectAsyncInvariants(state, before);
							}
							// At rest, one more refresh reaches every expanded row. Answering everything, in any
							// order, with the children as they were when asked must settle every operation and
							// leave the rows the world shows: no answer a read began before the refresh may stand.
							// A recursive refresh asked right after a plain one waits for the plain one's reads.
							if (last !== 'recursive') state.track(state.model.refresh(null));
							if (last !== 'plain') state.track(state.model.refresh(null, { recursive: true }));
							await answerAll(state, picks);
							expectWorldRows(state);
							if (last === 'plain') return;
							// A recursive refresh also leaves no collapsed row below holding children read before
							// it, so expanding every row shows the whole world.
							for (let round = 0; ; round += 1) {
								expect(round).toBeLessThan(1_000);
								const collapsed = rowKeys(state.model).filter((key) => {
									const row = state.model.rowAt(state.model.indexOf(key));
									return row.collapsible && row.collapsed;
								});
								if (collapsed.length === 0) break;
								for (const key of collapsed) state.track(state.model.expand(key));
								await answerAll(state, picks);
							}
							expectWorldRows(state);
						} finally {
							vi.useRealTimers();
						}
					},
				),
				{ interruptAfterTimeLimit: SHRINK_TIME_LIMIT_MS, numRuns: 200, seed: 2_026_11_29 },
			);
		},
		PROPERTY_TIMEOUT_MS,
	);
});
