import type { VirtualTreeSplice } from './virtual-tree-types';

/**
 * Unchanged rows between two changes that a merged splice sends again. Re-sending a few rows costs a
 * consumer less than handling another splice.
 */
const MERGE_GAP = 8;

/**
 * Splices one refreshed range may describe one by one. Past this, a single replacement of the range
 * is cheaper for every consumer than applying each change.
 */
const RANGE_SPLICE_LIMIT = 16;

/** Inserting more rows than this goes through one copy instead of spread arguments. */
const SPREAD_LIMIT = 4_096;

/**
 * Splices in final row positions: ordered, each starting more than a short gap after the rows the one
 * before it inserted, so they apply in order and every inserted row reads back at its final position.
 * A splice in current positions composes into the list wherever it lands. Past `limit` splices the list
 * stops tracking them and reports one replacement of its whole range instead.
 */
export class SpliceList {
	private readonly limit: number;
	private splices: VirtualTreeSplice[] = [];
	private overflowed = false;

	constructor(limit: number) {
		this.limit = limit;
	}

	add(start: number, deleteCount: number, insertCount: number): void {
		if (this.overflowed || (deleteCount === 0 && insertCount === 0)) return;
		composeSplice(this.splices, start, deleteCount, insertCount);
		if (this.splices.length > this.limit) {
			this.overflowed = true;
			this.splices = [];
		}
	}

	/**
	 * Hands out the splices and starts over. Past the limit they are one replacement of the `oldLength`
	 * rows at `base` by `newLength` rows.
	 */
	take(base: number, oldLength: number, newLength: number): readonly VirtualTreeSplice[] {
		const splices = this.overflowed
			? [{ deleteCount: oldLength, insertCount: newLength, start: base }]
			: this.splices;
		this.splices = [];
		this.overflowed = false;
		return splices;
	}
}

/** Composes a splice in current positions into ordered splices, merging it with any it touches. */
function composeSplice(
	splices: VirtualTreeSplice[],
	start: number,
	deleteCount: number,
	insertCount: number,
): void {
	const end = start + deleteCount;
	// The first splice whose inserted rows end within a merge gap before this one.
	let first = 0;
	let high = splices.length;
	while (first < high) {
		const middle = (first + high) >>> 1;
		const splice = splices[middle];
		if (splice !== undefined && splice.start + splice.insertCount + MERGE_GAP < start)
			first = middle + 1;
		else high = middle;
	}
	let mergedStart = start;
	let mergedEnd = end;
	let replaced = 0;
	let inserted = 0;
	let next = first;
	for (let splice = splices[next]; splice !== undefined; splice = splices[next]) {
		if (splice.start > end + MERGE_GAP) break;
		mergedStart = Math.min(mergedStart, splice.start);
		mergedEnd = Math.max(mergedEnd, splice.start + splice.insertCount);
		replaced += splice.deleteCount;
		inserted += splice.insertCount;
		next += 1;
	}
	// The merged range spans `mergedEnd - mergedStart` current rows: rows the merged splices inserted,
	// and old rows they left between them.
	const span = mergedEnd - mergedStart;
	const mergedDelete = replaced + span - inserted;
	const mergedInsert = span - deleteCount + insertCount;
	let after = first;
	if (mergedDelete === 0 && mergedInsert === 0) {
		// Rows inserted and removed again within one batch leave nothing to report.
		splices.splice(first, next - first);
	} else {
		splices.splice(first, next - first, {
			deleteCount: mergedDelete,
			insertCount: mergedInsert,
			start: mergedStart,
		});
		after += 1;
	}
	const shift = insertCount - deleteCount;
	if (shift === 0) return;
	for (let index = after; index < splices.length; index += 1) {
		const splice = splices[index];
		if (splice !== undefined) splices[index] = { ...splice, start: splice.start + shift };
	}
}

/**
 * Turns a walk over a refreshed range into splices. Kept rows advance the cursor; removed and
 * inserted rows accumulate into the splice at the cursor.
 */
export class RowSpliceBuilder {
	private readonly base: number;
	private cursor: number;
	private start = -1;
	private deleteCount = 0;
	private insertCount = 0;
	private readonly splices = new SpliceList(RANGE_SPLICE_LIMIT);

	constructor(base: number) {
		this.base = base;
		this.cursor = base;
	}

	keep(count: number): void {
		if (count === 0) return;
		this.flush();
		this.cursor += count;
	}

	remove(count: number): void {
		if (count === 0) return;
		if (this.start < 0) this.start = this.cursor;
		this.deleteCount += count;
	}

	insert(count: number): void {
		if (count === 0) return;
		if (this.start < 0) this.start = this.cursor;
		this.insertCount += count;
	}

	/** The splices that turn the old range of `oldLength` rows into the new one of `newLength`. */
	finish(oldLength: number, newLength: number): readonly VirtualTreeSplice[] {
		this.flush();
		return this.splices.take(this.base, oldLength, newLength);
	}

	private flush(): void {
		if (this.start < 0) return;
		this.splices.add(this.start, this.deleteCount, this.insertCount);
		this.cursor = this.start + this.insertCount;
		this.start = -1;
		this.deleteCount = 0;
		this.insertCount = 0;
	}
}

/**
 * Replaces rows and returns the array that holds them afterwards: the same array changed in place for
 * a small insertion or a removal, or a new one built by one copy for a large insertion. Replacing
 * every row adopts `insert`, so the caller hands it over.
 */
export function replaceRows<R>(rows: R[], start: number, deleteCount: number, insert: R[]): R[] {
	const end = start + deleteCount;
	if (insert.length === 0) {
		if (end === rows.length) rows.length = start;
		else if (deleteCount > 0) rows.splice(start, deleteCount);
		return rows;
	}
	if (start === 0 && end === rows.length) return insert;
	if (insert.length <= SPREAD_LIMIT) {
		rows.splice(start, deleteCount, ...insert);
		return rows;
	}
	return rows.slice(0, start).concat(insert, rows.slice(end));
}
