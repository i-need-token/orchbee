import { HeightIndex } from './HeightIndex';
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
	/** The row's top edge in the content, in CSS pixels. */
	readonly top: number;
	readonly height: number;
	readonly focused: boolean;
	readonly selected: boolean;
}

/** The content's height and how far it is scrolled, in CSS pixels. */
interface ContentGeometry {
	readonly height: number;
	readonly offset: number;
}

/** VS Code's tree row height, in CSS pixels. */
const DEFAULT_ROW_HEIGHT = 22;

const EMPTY_GEOMETRY: ContentGeometry = { height: 0, offset: 0 };

/**
 * The window a mounted tree shows onto its model: the viewport's size, the rows' geometry, and which
 * row each slot shows. The model keeps the scroll offset; the viewport keeps it within the rows.
 *
 * Slot `s` shows the row whose index leaves remainder `s` modulo the slot count, among the slot
 * count rows from the first visible one. The slot count covers every row the viewport can show at
 * once, so visible rows land in different slots, and a row that scrolls in takes the slot of the row
 * that scrolled out: a scroll by one row changes one slot.
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
	private geometry: ContentGeometry = EMPTY_GEOMETRY;
	/** A reveal that arrived before the view had a height, for its first one. */
	private pendingReveal: { readonly key: K; readonly relativeTop: number | null } | null = null;

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

	get content(): ContentGeometry {
		return this.geometry;
	}

	/** The offset the view shows: the model's, kept within the rows. */
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

	/** The nearest offset the rows allow, in whole pixels. */
	clampScrollTop(offset: number): number {
		const max = Math.max(0, this.geometry.height - this.height);
		return Math.min(Math.max(0, Math.round(offset)), max);
	}

	scrollTo(offset: number): void {
		this.model.setScrollTop(this.clampScrollTop(offset));
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

	/** VS Code's list reveal: the row at `relativeTop` of the view, or scrolled just into sight. */
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
		if (relativeTop !== null) {
			const placement = Math.min(Math.max(relativeTop, 0), 1);
			this.scrollTo((height - this.height) * placement + top);
			return;
		}
		const scrollTop = this.geometry.offset;
		const bottom = top + height;
		const viewBottom = scrollTop + this.height;
		// A row taller than the view that already overflows it in both directions stays.
		if (top < scrollTop && bottom >= viewBottom) return;
		if (top < scrollTop || (bottom >= viewBottom && height >= this.height)) this.scrollTo(top);
		else if (bottom >= viewBottom) this.scrollTo(bottom - this.height);
	}

	/** Recomputes the snapshots and tells listeners when one changed or `force` asks. */
	private update(force = false): void {
		const model = this.model;
		const count = model.rowCount;
		const contentHeight =
			this.heights === null ? count * this.fixedHeight() : this.heights.totalHeight;
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
		const selection = model.selection;
		const first = slotCount === 0 ? 0 : start % slotCount;
		for (let slot = 0; slot < slotCount; slot += 1) {
			const index = start + ((slot - first + slotCount) % slotCount);
			const previous = this.slots[slot] ?? null;
			const next = index < end ? this.slotFor(index, previous, focusIndex, selection) : null;
			if (next === previous) continue;
			this.slots[slot] = next;
			changed = true;
		}
		const focusOutside = focusIndex >= 0 && (focusIndex < start || focusIndex >= end);
		const focused = focusOutside
			? this.slotFor(focusIndex, this.focusedSlot, focusIndex, selection)
			: null;
		if (focused !== this.focusedSlot) {
			this.focusedSlot = focused;
			changed = true;
		}

		if (changed) {
			for (const listener of this.listeners) listener();
		}
		// Rows that shrank below the offset pull it back, once a measured view shows them.
		if (offset !== stored && this.disconnect !== null && this.height > 0) {
			model.setScrollTop(offset);
		}
	}

	private slotFor(
		index: number,
		previous: RowSlot<K, T> | null,
		focusIndex: number,
		selection: ReadonlySet<K>,
	): RowSlot<K, T> {
		const row = this.model.rowAt(index);
		const { positionInSet, revision, setSize } = row;
		const top = this.rowTop(index);
		const height = this.heightAt(index);
		const focused = index === focusIndex;
		const selected = selection.has(row.key);
		if (
			previous !== null &&
			previous.index === index &&
			previous.row === row &&
			previous.revision === revision &&
			previous.positionInSet === positionInSet &&
			previous.setSize === setSize &&
			previous.top === top &&
			previous.height === height &&
			previous.focused === focused &&
			previous.selected === selected
		) {
			return previous;
		}
		return { focused, height, index, positionInSet, revision, row, selected, setSize, top };
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

function checkRowHeight<K, T>(rowHeight: VirtualTreeRowHeight<K, T>): VirtualTreeRowHeight<K, T> {
	if (typeof rowHeight === 'number' && !(Number.isFinite(rowHeight) && rowHeight > 0)) {
		throw new RangeError(`Invalid row height ${rowHeight}`);
	}
	return rowHeight;
}
