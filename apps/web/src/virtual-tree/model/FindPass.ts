import { findTester } from './find-score';
import type { TreeNode } from './tree-node';
import type { VirtualTreeFindQuery } from './virtual-tree-types';

/** Nodes matched between two reads of the clock. */
const CLOCK_INTERVAL = 32;

/** One children list of a walk over a subtree, and how far the walk has come through it. */
interface WalkLevel<K, T> {
	readonly nodes: readonly TreeNode<K, T>[];
	index: number;
}

/** What a pass has left to match: listed nodes, then whole subtrees. */
export interface FindPassWork<K, T> {
	readonly nodes: TreeNode<K, T>[];
	readonly walk: WalkLevel<K, T>[];
}

let passCount = 0;

/**
 * One find's look at the nodes it must match, made in slices. Each node keeps the verdict this pass
 * gave it in its own pass fields, so the rows go on showing the applied find until the model commits
 * the pass. A walk reads each children list as it found it: the model replaces a list rather than
 * change it, and every node an operation brings in comes to the pass through `add`.
 */
export class FindPass<K, T> {
	readonly id: number;
	/** The query this pass matches; one that differs only in how rows show takes its place. */
	query: VirtualTreeFindQuery<K, T>;
	readonly test: (label: string) => boolean;
	/** The task that runs the next slice, while one waits. */
	timer: ReturnType<typeof setTimeout> | null = null;
	private readonly nodes: TreeNode<K, T>[];
	private cursor = 0;
	private readonly walk: WalkLevel<K, T>[];
	/** The nodes this pass found a match in, which a longer pattern matches again. */
	private readonly matched: TreeNode<K, T>[] = [];

	/** Takes over the work's lists. */
	constructor(query: VirtualTreeFindQuery<K, T>, work: FindPassWork<K, T>) {
		passCount += 1;
		this.id = passCount;
		this.query = query;
		this.test = findTester(query.pattern, query.matchType);
		this.nodes = work.nodes;
		this.walk = work.walk;
	}

	/** The work of a pass over every node below `root`. */
	static walking<K, T>(root: TreeNode<K, T>): FindPassWork<K, T> {
		return { nodes: [], walk: [{ index: 0, nodes: root.children }] };
	}

	/** Whether the pass has matched every node it holds. */
	get finished(): boolean {
		const { walk } = this;
		for (let level = walk[walk.length - 1]; level !== undefined; level = walk[walk.length - 1]) {
			if (level.index < level.nodes.length) return false;
			walk.pop();
		}
		return this.cursor >= this.nodes.length;
	}

	/** Adds a node to match, such as one a refresh brought in while the pass runs. */
	add(node: TreeNode<K, T>): void {
		this.nodes.push(node);
	}

	/** Matches nodes until the clock passes `deadline`; true once it has matched every node. */
	advance(deadline: number): boolean {
		const { id, matched, test } = this;
		const label = this.query.label;
		let batch = 0;
		for (let node = this.next(); node !== undefined; node = this.next()) {
			if (node.attached) {
				node.findPass = id;
				// A merged node shows in its chain's row, which the find matches as a whole by its tail.
				const found = !node.merged && test(label(node));
				node.findPassMatched = found;
				if (found) matched.push(node);
			}
			batch += 1;
			if (batch === CLOCK_INTERVAL) {
				batch = 0;
				if (performance.now() >= deadline) return this.finished;
			}
		}
		return true;
	}

	/**
	 * What a longer pattern has to match: the nodes this pass matched and those it has not reached,
	 * since a label that misses a pattern misses every pattern that extends it.
	 */
	narrowed(): FindPassWork<K, T> {
		const { id } = this;
		const nodes = this.matched.filter((node) => node.findPass === id && node.findPassMatched);
		for (let index = this.cursor; index < this.nodes.length; index += 1) {
			const node = this.nodes[index];
			if (node !== undefined) nodes.push(node);
		}
		return { nodes, walk: this.walk.map(({ index, nodes: level }) => ({ index, nodes: level })) };
	}

	/** The next node to match: listed nodes first, then the walk's, depth first. */
	private next(): TreeNode<K, T> | undefined {
		while (this.cursor < this.nodes.length) {
			const node = this.nodes[this.cursor];
			this.cursor += 1;
			if (node !== undefined) return node;
		}
		const { walk } = this;
		for (let level = walk[walk.length - 1]; level !== undefined; level = walk[walk.length - 1]) {
			const node = level.nodes[level.index];
			if (node === undefined) {
				walk.pop();
				continue;
			}
			level.index += 1;
			// A detached node's subtree has left the tree with it.
			if (node.attached && node.children.length > 0) walk.push({ index: 0, nodes: node.children });
			return node;
		}
		return undefined;
	}
}
