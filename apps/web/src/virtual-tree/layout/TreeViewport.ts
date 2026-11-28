import { HeightIndex } from '../../list-scrolling';
import type { VirtualTreeModel } from '../model/VirtualTreeModel';
import type {
	VirtualTreeEvent,
	VirtualTreeRow,
	VirtualTreeSplice,
} from '../model/virtual-tree-types';

/** A row's height in CSS pixels: one height for every row, or a height read from each row. */
export type VirtualTreeRowHeight<K, T> = number | ((row: VirtualTreeRow<K, T>) => number);

/** What one slot shows. A slot keeps the same object while nothing it shows changes. */
interface RowSlot<K, T> {
	readonly index: number;
	readonly row: VirtualTreeRow<K, T>;
	readonly revision: number;
	readonly positionInSet: number;
	readonly setSize: number;
	/** The row's indentation, which changes when a compact row above it joins or splits. */
	readonly depth: number;
	/** The row's top edge in the content, in CSS pixels. */
	readonly top: number;
	readonly height: number;
	readonly focused: boolean;
	/** The index among the row's segments of the focused one while the row has focus; -1 otherwise. */
	readonly focusedSegment: number;
	readonly selected: boolean;
	/** Whether a drag would drop into this row or the row it belongs to. */
	readonly dropTarget: boolean;
	/** The index of the joined item a drag would drop on, in a compact row; -1 otherwise. */
	readonly dropSegment: number;
}

/**
 * Where a drag would drop: into a row and the rows below it that belong to it, into an item a compact
 * row joins, alone, or into the whole tree.
 */
export type TreeDropTarget<K> = { readonly key: K } | 'tree' | null;

/**
 * The indexes of the rows a drag would drop into, first and last, -1 for none, or the row and the
 * segment of a joined item it would drop on.
 */
interface DropRows {
	readonly start: number;
	readonly end: number;
	readonly segmentRow: number;
	readonly segment: number;
}

const NO_DROP_ROWS: DropRows = { end: -1, segment: -1, segmentRow: -1, start: -1 };

/** How far the content scrolls and how far it is scrolled, in CSS pixels. */
interface ContentGeometry {
	/** The rows' height and the room left below the last row. */
	readonly height: number;
	readonly offset: number;
}

/** A row that sticks at the top of the view while its descendants scroll beneath it. */
interface StickyRow<K, T> {
	readonly row: VirtualTreeRow<K, T>;
	readonly revision: number;
	readonly depth: number;
	/** Its top edge in the view, in CSS pixels; negative while its last descendant pushes it out. */
	readonly top: number;
	readonly height: number;
}

/** VS Code's tree row height, in CSS pixels. */
const DEFAULT_ROW_HEIGHT = 22;

/** VS Code's `workbench.tree.stickyScrollMaxItemCount` default. */
const DEFAULT_MAX_STICKY_ROWS = 7;

/** The share of the view's height that sticky rows may cover, as in VS Code. */
const STICKY_HEIGHT_RATIO = 0.4;

const EMPTY_GEOMETRY: ContentGeometry = { height: 0, offset: 0 };

/**
 * The window a mounted tree shows onto its model: the viewport's size, the rows' geometry, and which
 * row each slot shows. The model keeps the scroll offset; the viewport keeps it within the rows and
 * the room below them.
 *
 * Slot `s` shows the row whose index leaves remainder `s` modulo the slot count, among the slot
 * count rows from the first visible one. The slot count covers every row the viewport can show at
 * once, so visible rows land in different slots, and a row that scrolls in takes the slot of the row
 * that scrolled out: a scroll by one row changes one slot.
 *
 * Rows whose descendants scroll beneath the top of the view stick there, as VS Code's sticky scroll
 * keeps them: each expanded ancestor of the row under the sticky rows, outermost first, until the
 * last descendant of one scrolls out and pushes it up, never more rows than the limit or more than
 * 40% of the view's height. A reveal keeps its row below the sticky rows its ancestors would show.
 *
 * Views read it through `subscribe` and the snapshot getters, which return the same object until what
 * it describes changes. It hears the model only while something listens, and catches up when the
 * first listener arrives.
 */
export class TreeViewport<K, T> {
	readonly model: VirtualTreeModel<K, T>;
	private rowHeight: VirtualTreeRowHeight<K, T>;
	/** Heights of rows that differ in height; rows that share one are placed by arithmetic. */
	private heights: HeightIndex | null;
	private width = 0;
	private height = 0;
	private readonly listeners = new Set<() => void>();
	private disconnect: (() => void) | null = null;
	private slotCountValue = 0;
	private readonly slots: (RowSlot<K, T> | null)[] = [];
	private focusedSlot: RowSlot<K, T> | null = null;
	private focusIndexValue = -1;
	private focusedSegmentValue = -1;
	private geometry: ContentGeometry = EMPTY_GEOMETRY;
	/** The rows' height alone, without the room below them. */
	private rowsHeight = 0;
	private paddingBottomValue = 0;
	private maxStickyRowsValue = DEFAULT_MAX_STICKY_ROWS;
	private readonly stickyRows: StickyRow<K, T>[] = [];
	private stickyHeightValue = 0;
	/** A reveal that arrived before the view had a height, for its first one. */
	private pendingReveal: { readonly key: K; readonly relativeTop: number | null } | null = null;
	private dropTargetValue: TreeDropTarget<K> = null;
	private dropOnTreeValue = false;

	constructor(model: VirtualTreeModel<K, T>) {
		this.model = model;
		this.rowHeight = DEFAULT_ROW_HEIGHT;
		this.heights = null;
		this.update();
	}

	/** How many slots the view renders; each shows a row or nothing. */
	get slotCount(): number {
		return this.slotCountValue;
	}

	/** The focused row's index, or -1 when nothing is focused or a collapsed parent hides it. */
	get focusIndex(): number {
		return this.focusIndexValue;
	}

	/**
	 * The index among the focused row's segments of the focused one while that row is compact; -1
	 * while it shows one item or nothing is focused.
	 */
	get focusedSegment(): number {
		return this.focusedSegmentValue;
	}

	get content(): ContentGeometry {
		return this.geometry;
	}

	/** The offset the view shows: the model's, kept within the content. */
	get scrollTop(): number {
		return this.geometry.offset;
	}

	get viewportHeight(): number {
		return this.height;
	}

	get viewportWidth(): number {
		return this.width;
	}

	/** A slot's row, or null when it shows nothing. */
	slot(slot: number): RowSlot<K, T> | null {
		return this.slots[slot] ?? null;
	}

	/** The focused row while it is outside the visible rows, so its element stays for assistive tools. */
	get focusedRow(): RowSlot<K, T> | null {
		return this.focusedSlot;
	}

	/** How many rows stick at the top of the view. */
	get stickyCount(): number {
		return this.stickyRows.length;
	}

	/** How far down the view the sticky rows reach, in CSS pixels; 0 while none sticks. */
	get stickyHeight(): number {
		return this.stickyHeightValue;
	}

	/** Whether a drag would drop into the whole tree. */
	get dropOnTree(): boolean {
		return this.dropOnTreeValue;
	}

	/** The sticky row at a depth among them, outermost first, or null when fewer stick. */
	stickyRow(index: number): StickyRow<K, T> | null {
		return this.stickyRows[index] ?? null;
	}

	readonly subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		if (this.disconnect === null) {
			this.disconnect = this.model.subscribe(this.hear);
			// The rows may have changed while nothing listened.
			if (this.heights !== null) this.heights = this.readHeights();
			this.update();
		}
		return () => {
			this.listeners.delete(listener);
			if (this.listeners.size > 0 || this.disconnect === null) return;
			this.disconnect();
			this.disconnect = null;
		};
	};

	setSize(width: number, height: number): void {
		const nextWidth = Math.max(0, Math.floor(width));
		const nextHeight = Math.max(0, Math.floor(height));
		if (nextWidth === this.width && nextHeight === this.height) return;
		// Rows of different heights fit a different number of slots in another height.
		if (nextHeight !== this.height && this.heights !== null) this.slotCountValue = 0;
		this.width = nextWidth;
		this.height = nextHeight;
		this.update(true);
		const reveal = this.pendingReveal;
		if (reveal !== null && nextHeight > 0) this.reveal(reveal.key, reveal.relativeTop);
	}

	/** Changes how tall rows are, VS Code's 22 px by default; a height function is read for every row. */
	setRowHeight(rowHeight: VirtualTreeRowHeight<K, T> = DEFAULT_ROW_HEIGHT): void {
		if (rowHeight === this.rowHeight) return;
		this.rowHeight = checkRowHeight(rowHeight);
		this.heights = this.readHeights();
		this.slotCountValue = 0;
		this.update();
	}

	/**
	 * Leaves room below the last row that the view scrolls into, in CSS pixels, as VS Code's lists'
	 * `paddingBottom` does; none by default.
	 */
	setPaddingBottom(padding = 0): void {
		if (!(Number.isFinite(padding) && padding >= 0)) {
			throw new RangeError(`Invalid bottom padding ${padding}`);
		}
		if (padding === this.paddingBottomValue) return;
		this.paddingBottomValue = padding;
		this.update();
	}

	/** Limits how many rows stick at the top, VS Code's 7 by default; 0 turns sticky scroll off. */
	setMaxStickyRows(count: number = DEFAULT_MAX_STICKY_ROWS): void {
		if (!Number.isInteger(count) || count < 0) {
			throw new RangeError(`Invalid sticky row limit ${count}`);
		}
		if (count === this.maxStickyRowsValue) return;
		this.maxStickyRowsValue = count;
		this.update();
	}

	/**
	 * Whether a row shows whole between the sticky rows and the bottom of the view, where VS Code's
	 * `getRelativeTop` answers with a place rather than null.
	 */
	inSight(key: K): boolean {
		const index = this.model.indexOf(key);
		if (index < 0 || this.height === 0) return false;
		const top = this.rowTop(index);
		const scrollTop = this.geometry.offset;
		return (
			top >= scrollTop + this.stickyHeightValue &&
			top + this.heightAt(index) <= scrollTop + this.height
		);
	}

	/**
	 * Scrolls a row to just below the sticky rows of its ancestors, where clicking its sticky row
	 * takes it in VS Code.
	 */
	scrollBelowStickyRows(key: K): void {
		const index = this.model.indexOf(key);
		if (index < 0) return;
		this.scrollTo(this.rowTop(index) - this.stickyPadding(this.model.rowAt(index)));
	}

	/**
	 * The row holding a content offset, counted as VS Code's lists count: -1 above the rows and the
	 * row count below them.
	 */
	indexAtOffset(offset: number): number {
		if (offset < 0) return -1;
		const count = this.model.rowCount;
		return offset < this.rowsHeight ? this.indexAt(offset, count) : count;
	}

	/** The nearest offset the rows and the room below them allow, in whole pixels. */
	clampScrollTop(offset: number): number {
		const max = Math.max(0, this.geometry.height - this.height);
		return Math.min(Math.max(0, Math.round(offset)), max);
	}

	scrollTo(offset: number): void {
		this.model.setScrollTop(this.clampScrollTop(offset));
	}

	/**
	 * Shows where a drag would drop: into a row, with the rows below it that belong to it as they
	 * expand or collapse, into an item a compact row joins, which lights up alone, into the whole
	 * tree, or nowhere.
	 */
	setDropTarget(target: TreeDropTarget<K>): void {
		const current = this.dropTargetValue;
		const sameRow =
			target !== null &&
			target !== 'tree' &&
			current !== null &&
			current !== 'tree' &&
			target.key === current.key;
		if (target === current || sameRow) return;
		this.dropTargetValue = target;
		this.update();
	}

	private readonly hear = (event: VirtualTreeEvent<K>): void => {
		switch (event.kind) {
			case 'rows':
				if (this.heights !== null) this.spliceHeights(this.heights, event.splices);
				break;
			case 'reveal':
				this.reveal(event.key, event.relativeTop);
				return;
			case 'collapse':
			case 'anchor':
				return;
			case 'focus':
			case 'selection':
			case 'scroll':
				break;
		}
		this.update();
	};

	/**
	 * VS Code's tree reveal: the row at `relativeTop` of the view, or scrolled just into sight, below
	 * the sticky rows its ancestors would show.
	 */
	private reveal(key: K, relativeTop: number | null): void {
		this.pendingReveal = null;
		if (this.height === 0) {
			this.pendingReveal = { key, relativeTop };
			return;
		}
		const index = this.model.indexOf(key);
		if (index < 0) return;
		const top = this.rowTop(index);
		const height = this.heightAt(index);
		const padding = this.stickyPadding(this.model.rowAt(index));
		if (relativeTop !== null) {
			const placement = Math.min(Math.max(relativeTop, 0), 1);
			this.scrollTo((height - this.height + padding) * placement + top - padding);
			return;
		}
		const scrollTop = this.geometry.offset;
		const bottom = top + height;
		const viewBottom = scrollTop + this.height;
		// A row taller than the view that already overflows it in both directions stays.
		if (top < scrollTop + padding && bottom >= viewBottom) return;
		if (top < scrollTop + padding || (bottom >= viewBottom && height >= this.height)) {
			this.scrollTo(top - padding);
		} else if (bottom >= viewBottom) {
			this.scrollTo(bottom - this.height);
		}
	}

	/**
	 * How far down the view the sticky rows of a row's ancestors reach once it sits right below them:
	 * the outermost ones, within the sticky limits. VS Code pads by the nearest ancestors up to the row
	 * limit alone, which can push a revealed row out of a short view.
	 */
	private stickyPadding(row: VirtualTreeRow<K, T>): number {
		const limit = this.maxStickyRowsValue;
		if (limit === 0 || row.parent === null) return 0;
		const ancestors: VirtualTreeRow<K, T>[] = [];
		for (let parent: VirtualTreeRow<K, T> | null = row.parent; parent !== null;) {
			ancestors.push(parent);
			parent = parent.parent;
		}
		const maxHeight = this.height * STICKY_HEIGHT_RATIO;
		let padding = 0;
		for (let depth = 0; depth < limit && depth < ancestors.length; depth += 1) {
			const ancestor = ancestors[ancestors.length - 1 - depth];
			if (ancestor === undefined) break;
			const height = this.heightAt(this.model.indexOf(ancestor.key));
			if (padding + height > maxHeight) break;
			padding += height;
		}
		return padding;
	}

	/** Recomputes the snapshots and tells listeners when one changed or `force` asks. */
	private update(force = false): void {
		const model = this.model;
		const count = model.rowCount;
		const rowsHeight =
			this.heights === null ? count * this.fixedHeight() : this.heights.totalHeight;
		this.rowsHeight = rowsHeight;
		const contentHeight = rowsHeight + this.paddingBottomValue;
		const stored = model.scrollTop;
		const offset = Math.min(stored, Math.max(0, contentHeight - this.height));
		let changed = force;
		if (this.geometry.height !== contentHeight || this.geometry.offset !== offset) {
			this.geometry = { height: contentHeight, offset };
			changed = true;
		}

		let start = 0;
		let end = 0;
		if (count > 0 && this.height > 0) {
			start = this.indexAt(offset, count);
			end = this.indexAt(offset + this.height - 1, count) + 1;
		}
		const slotCount = this.slotCountFor(end - start);
		if (slotCount !== this.slotCountValue) {
			this.slotCountValue = slotCount;
			changed = true;
		}
		this.slots.length = slotCount;

		const focusKey = model.focus;
		const focusIndex = focusKey === null ? -1 : model.indexOf(focusKey);
		if (focusIndex !== this.focusIndexValue) {
			this.focusIndexValue = focusIndex;
			changed = true;
		}
		const focusRow = focusIndex < 0 ? null : model.rowAt(focusIndex);
		const focusSegment = focusRow === null ? -1 : segmentOf(focusRow, focusKey);
		const focusedSegment = focusRow !== null && focusRow.segments.length > 1 ? focusSegment : -1;
		if (focusedSegment !== this.focusedSegmentValue) {
			this.focusedSegmentValue = focusedSegment;
			changed = true;
		}
		const selection = model.selection;
		const drop = this.dropTargetValue;
		const dropOnTree = drop === 'tree';
		if (dropOnTree !== this.dropOnTreeValue) {
			this.dropOnTreeValue = dropOnTree;
			changed = true;
		}
		const dropRows = drop === null || drop === 'tree' ? NO_DROP_ROWS : this.dropRowsOf(drop.key);
		const first = slotCount === 0 ? 0 : start % slotCount;
		for (let slot = 0; slot < slotCount; slot += 1) {
			const index = start + ((slot - first + slotCount) % slotCount);
			const previous = this.slots[slot] ?? null;
			const next =
				index < end
					? this.slotFor(index, previous, focusIndex, focusSegment, selection, dropRows)
					: null;
			if (next === previous) continue;
			this.slots[slot] = next;
			changed = true;
		}
		const focusOutside = focusIndex >= 0 && (focusIndex < start || focusIndex >= end);
		const focused = focusOutside
			? this.slotFor(focusIndex, this.focusedSlot, focusIndex, focusSegment, selection, dropRows)
			: null;
		if (focused !== this.focusedSlot) {
			this.focusedSlot = focused;
			changed = true;
		}
		if (this.updateStickyRows(offset, count)) changed = true;

		if (changed) {
			for (const listener of this.listeners) listener();
		}
		// Rows that shrank below the offset pull it back, once a measured view shows them.
		if (offset !== stored && this.disconnect !== null && this.height > 0) {
			model.setScrollTop(offset);
		}
	}

	/** Where a drop on the item with `key` shows. */
	private dropRowsOf(key: K): DropRows {
		const model = this.model;
		const index = model.indexOf(key);
		if (index < 0) return NO_DROP_ROWS;
		const row = model.rowAt(index);
		// An item a compact row joins takes the drop alone, as VS Code's Explorer shows it on its name.
		if (row.key !== key) {
			return { end: -1, segment: segmentOf(row, key), segmentRow: index, start: -1 };
		}
		return { end: index + row.visibleDescendantCount, segment: -1, segmentRow: -1, start: index };
	}

	private slotFor(
		index: number,
		previous: RowSlot<K, T> | null,
		focusIndex: number,
		focusSegment: number,
		selection: ReadonlySet<K>,
		dropRows: DropRows,
	): RowSlot<K, T> {
		const row = this.model.rowAt(index);
		const { depth, positionInSet, revision, setSize } = row;
		const top = this.rowTop(index);
		const height = this.heightAt(index);
		const focused = index === focusIndex;
		const focusedSegment = focused ? focusSegment : -1;
		const selected = selection.has(row.key);
		const dropTarget = dropRows.start >= 0 && index >= dropRows.start && index <= dropRows.end;
		const dropSegment = index === dropRows.segmentRow ? dropRows.segment : -1;
		if (
			previous !== null &&
			previous.index === index &&
			previous.row === row &&
			previous.revision === revision &&
			previous.positionInSet === positionInSet &&
			previous.setSize === setSize &&
			previous.depth === depth &&
			previous.top === top &&
			previous.height === height &&
			previous.focused === focused &&
			previous.focusedSegment === focusedSegment &&
			previous.selected === selected &&
			previous.dropTarget === dropTarget &&
			previous.dropSegment === dropSegment
		) {
			return previous;
		}
		return {
			depth,
			dropSegment,
			dropTarget,
			focused,
			focusedSegment,
			height,
			index,
			positionInSet,
			revision,
			row,
			selected,
			setSize,
			top,
		};
	}

	/**
	 * Finds the sticky rows as VS Code's sticky scroll does, keeping the entries that did not change,
	 * and reports whether any did. Starting from the row at the top of the view, each step takes the
	 * ancestor of the row under the sticky rows that sits directly below the previous sticky row, while
	 * that ancestor's own row has scrolled under them and it has visible descendants. A sticky row
	 * stacks below the ones above it unless its last descendant has scrolled that high, which pushes it
	 * up; the rows end at the first one past the row limit or 40% of the view's height.
	 */
	private updateStickyRows(offset: number, count: number): boolean {
		const sticky = this.stickyRows;
		const limit = this.maxStickyRowsValue;
		const model = this.model;
		const maxHeight = this.height * STICKY_HEIGHT_RATIO;
		let changed = false;
		let depth = 0;
		let stacked = 0;
		let reach = 0;
		let previous: VirtualTreeRow<K, T> | null = null;
		let under = offset > 0 && this.height > 0 ? this.rowIndexAt(offset, count) : -1;
		while (depth < limit && under >= 0) {
			const below = model.rowAt(under);
			const row: VirtualTreeRow<K, T> | null = ancestorBelow(below, previous);
			if (row === null || row.visibleDescendantCount === 0) break;
			const index = row === below ? under : model.indexOf(row.key);
			// Its own row has not yet scrolled under the sticky rows above it.
			if (index < 0 || offset + stacked <= this.rowTop(index)) break;
			const height = this.heightAt(index);
			const last = index + row.visibleDescendantCount;
			const lastBottom = this.rowTop(last) + this.heightAt(last) - offset;
			const top =
				stacked + height > lastBottom && stacked <= lastBottom ? lastBottom - height : stacked;
			if (top + height > maxHeight) break;
			const entry = sticky[depth];
			const rowDepth = row.depth;
			if (
				entry === undefined ||
				entry.row !== row ||
				entry.revision !== row.revision ||
				entry.depth !== rowDepth ||
				entry.top !== top ||
				entry.height !== height
			) {
				sticky[depth] = { depth: rowDepth, height, revision: row.revision, row, top };
				changed = true;
			}
			depth += 1;
			stacked += height;
			reach = top + height;
			previous = row;
			under = this.rowIndexAt(offset + reach, count);
		}
		if (sticky.length !== depth) {
			sticky.length = depth;
			changed = true;
		}
		if (reach !== this.stickyHeightValue) {
			this.stickyHeightValue = reach;
			changed = true;
		}
		return changed;
	}

	/**
	 * Every row the view can show at once: a fixed height needs one slot per row height the view
	 * spans, plus one for a row cut at each edge; varying heights keep the most rows seen since the
	 * view last changed height.
	 */
	private slotCountFor(visible: number): number {
		if (this.height === 0) return 0;
		if (this.heights !== null) return Math.max(this.slotCountValue, visible);
		return Math.ceil((this.height - 1) / this.fixedHeight()) + 1;
	}

	private indexAt(offset: number, count: number): number {
		if (this.heights !== null) return this.heights.indexAt(offset);
		return Math.min(count - 1, Math.max(0, Math.floor(offset / this.fixedHeight())));
	}

	/** The row holding a content offset, or -1 past the last row. */
	private rowIndexAt(offset: number, count: number): number {
		return count > 0 && offset < this.rowsHeight ? this.indexAt(offset, count) : -1;
	}

	private rowTop(index: number): number {
		return this.heights === null ? index * this.fixedHeight() : this.heights.positionOf(index);
	}

	private heightAt(index: number): number {
		return this.heights === null ? this.fixedHeight() : this.heights.heightOf(index);
	}

	private fixedHeight(): number {
		return typeof this.rowHeight === 'number' ? this.rowHeight : 0;
	}

	/** Reads every row's height when they differ; null when they share one. */
	private readHeights(): HeightIndex | null {
		const rowHeight = this.rowHeight;
		if (typeof rowHeight === 'number') return null;
		const model = this.model;
		const heights = new Float64Array(model.rowCount);
		for (let index = 0; index < heights.length; index += 1)
			heights[index] = rowHeight(model.rowAt(index));
		return new HeightIndex(heights);
	}

	private spliceHeights(heights: HeightIndex, splices: readonly VirtualTreeSplice[]): void {
		const rowHeight = this.rowHeight;
		if (typeof rowHeight === 'number') return;
		// Rows that changed in place may have changed height too.
		if (splices.length === 0) {
			this.heights = this.readHeights();
			return;
		}
		const model = this.model;
		for (const splice of splices) {
			const inserted = new Float64Array(splice.insertCount);
			for (let offset = 0; offset < inserted.length; offset += 1) {
				inserted[offset] = rowHeight(model.rowAt(splice.start + offset));
			}
			heights.splice(splice.start, splice.deleteCount, inserted);
		}
	}
}

/** The index of the segment holding `key` among a row's segments; -1 when none does. */
function segmentOf<K, T>(row: VirtualTreeRow<K, T>, key: K | null): number {
	const { segments } = row;
	for (let index = segments.length - 1; index >= 0; index -= 1) {
		if (segments[index]?.key === key) return index;
	}
	return -1;
}

/**
 * The ancestor of a row, or the row itself, whose parent is `previous`; the top-level one when
 * `previous` is null, and null when the row is not below `previous`.
 */
function ancestorBelow<K, T>(
	row: VirtualTreeRow<K, T>,
	previous: VirtualTreeRow<K, T> | null,
): VirtualTreeRow<K, T> | null {
	let current = row;
	for (let parent = current.parent; parent !== null; parent = current.parent) {
		if (parent === previous) return current;
		current = parent;
	}
	return previous === null ? current : null;
}

function checkRowHeight<K, T>(rowHeight: VirtualTreeRowHeight<K, T>): VirtualTreeRowHeight<K, T> {
	if (typeof rowHeight === 'number' && !(Number.isFinite(rowHeight) && rowHeight > 0)) {
		throw new RangeError(`Invalid row height ${rowHeight}`);
	}
	return rowHeight;
}
