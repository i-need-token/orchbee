const EMPTY: ArrayLike<number> = [];

/** Same-length replacements up to this many rows update the tree row by row instead of rebuilding it. */
const POINT_UPDATE_LIMIT = 32;

const MIN_CAPACITY = 16;

/**
 * The heights of a list's rows and the offsets they add up to, for lists whose rows differ in height.
 * Finding the row at an offset, the offset of a row, and changing one height each cost O(log n); a
 * splice costs O(log² n) plus one pass over the rows after its start. Heights are CSS pixels, and whole
 * pixels keep every offset exact. A list whose rows share one height needs none of this: its offsets
 * are the index times the height.
 */
export class HeightIndex {
	private heights: Float64Array;
	/** A Fenwick tree, one-based: node `i` holds the heights of rows `i - lowbit(i)` to `i - 1`. */
	private tree: Float64Array;
	private size = 0;
	private total = 0;

	constructor(heights: ArrayLike<number> = EMPTY) {
		const capacity = Math.max(MIN_CAPACITY, heights.length);
		this.heights = new Float64Array(capacity);
		this.tree = new Float64Array(capacity + 1);
		this.splice(0, 0, heights);
	}

	get count(): number {
		return this.size;
	}

	get totalHeight(): number {
		return this.total;
	}

	heightOf(index: number): number {
		this.checkIndex(index, this.size - 1);
		return this.heights[index] ?? 0;
	}

	/** The offset of a row's top edge; `count` gives the offset just past the last row. */
	positionOf(index: number): number {
		this.checkIndex(index, this.size);
		return this.prefix(index);
	}

	/**
	 * The row whose extent holds an offset: the first row for an offset above the list, the last for
	 * one past its end, and -1 when there are no rows. A row without height is returned only at the
	 * end of the list, since the row after it holds the same offset.
	 */
	indexAt(position: number): number {
		const size = this.size;
		if (size === 0) return -1;
		const tree = this.tree;
		// Binary lifting finds how many rows end at or above the offset; the next one holds it.
		let index = 0;
		let remaining = position;
		for (let step = 1 << (31 - Math.clz32(size)); step > 0; step >>= 1) {
			const next = index + step;
			if (next > size) continue;
			const height = tree[next] ?? 0;
			if (height <= remaining) {
				index = next;
				remaining -= height;
			}
		}
		return Math.min(index, size - 1);
	}

	setHeight(index: number, height: number): void {
		this.checkIndex(index, this.size - 1);
		this.checkHeight(height);
		const delta = height - (this.heights[index] ?? 0);
		if (delta === 0) return;
		this.heights[index] = height;
		const tree = this.tree;
		for (let node = index + 1; node <= this.size; node += node & -node) {
			tree[node] = (tree[node] ?? 0) + delta;
		}
		this.total += delta;
	}

	/** Removes `deleteCount` rows at `start` and inserts rows of the given heights in their place. */
	splice(start: number, deleteCount: number, heights: ArrayLike<number> = EMPTY): void {
		const size = this.size;
		this.checkIndex(start, size);
		if (!Number.isInteger(deleteCount) || deleteCount < 0 || start + deleteCount > size) {
			throw new RangeError(`Cannot remove ${deleteCount} rows at ${start} of ${size}`);
		}
		const insertCount = heights.length;
		for (let offset = 0; offset < insertCount; offset += 1) this.checkHeight(heights[offset] ?? 0);
		if (insertCount === deleteCount && insertCount <= POINT_UPDATE_LIMIT) {
			for (let offset = 0; offset < insertCount; offset += 1) {
				this.setHeight(start + offset, heights[offset] ?? 0);
			}
			return;
		}
		const nextSize = size - deleteCount + insertCount;
		this.reserve(nextSize);
		const rows = this.heights;
		if (insertCount !== deleteCount)
			rows.copyWithin(start + insertCount, start + deleteCount, size);
		for (let offset = 0; offset < insertCount; offset += 1)
			rows[start + offset] = heights[offset] ?? 0;
		this.size = nextSize;
		this.rebuildFrom(start);
	}

	/** The total height of the first `count` rows. */
	private prefix(count: number): number {
		const tree = this.tree;
		let sum = 0;
		for (let node = count; node > 0; node -= node & -node) sum += tree[node] ?? 0;
		return sum;
	}

	/**
	 * Recomputes every tree node from row `start` on, in one pass and without touching the nodes
	 * before it, which hold only rows before `start`.
	 */
	private rebuildFrom(start: number): void {
		const tree = this.tree;
		const rows = this.heights;
		const size = this.size;
		const first = start + 1;
		for (let node = first; node <= size; node += 1) tree[node] = rows[node - 1] ?? 0;
		for (let node = first; node <= size; node += 1) {
			const parent = node + (node & -node);
			if (parent <= size) tree[parent] = (tree[parent] ?? 0) + (tree[node] ?? 0);
		}
		// The rebuilt nodes whose rows begin before `start` lie on the update path of the row before
		// it; each still lacks those earlier rows, which the untouched nodes sum.
		const last = first - 1;
		if (last > 0) {
			const before = this.prefix(last);
			for (let node = last + (last & -last); node <= size; node += node & -node) {
				tree[node] = (tree[node] ?? 0) + before - this.prefix(node - (node & -node));
			}
		}
		this.total = this.prefix(size);
	}

	private reserve(size: number): void {
		if (size <= this.heights.length) return;
		const capacity = Math.max(size, this.heights.length * 2);
		const heights = new Float64Array(capacity);
		heights.set(this.heights.subarray(0, this.size));
		const tree = new Float64Array(capacity + 1);
		tree.set(this.tree.subarray(0, this.size + 1));
		this.heights = heights;
		this.tree = tree;
	}

	private checkIndex(index: number, max: number): void {
		if (!Number.isInteger(index) || index < 0 || index > max) {
			throw new RangeError(`Row ${index} is outside 0 to ${max}`);
		}
	}

	private checkHeight(height: number): void {
		if (!Number.isFinite(height) || height < 0)
			throw new RangeError(`Invalid row height ${height}`);
	}
}
