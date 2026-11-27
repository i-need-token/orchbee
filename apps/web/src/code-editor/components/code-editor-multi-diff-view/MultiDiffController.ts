import { CodeEditorMultiDiffLayout } from '../../CodeEditorMultiDiffLayout';
import { CodeEditorMultiDiffModels } from '../../CodeEditorMultiDiffModels';
import type { CodeEditorTextDocument } from '../../CodeEditorTextDocument';
import type { editor } from '../../monaco-entry';
import {
	MULTI_DIFF_BOTTOM_PADDING,
	MULTI_DIFF_ESTIMATED_FILE_HEIGHT,
	MULTI_DIFF_HEADER_HEIGHT,
} from './constants/multi-diff-geometry';

/** One file of a multi-diff. */
export interface MultiDiffEntry {
	/** The file's identity in the list: unique within it, and stable as the list changes. */
	readonly key: string;
	/** The file's name, which its header shows and a refusal quotes. */
	readonly name: string;
	/** Where the file is, such as its folder, which its header shows after the name. */
	readonly description: string;
	readonly original: CodeEditorTextDocument<editor.ITextModel>;
	readonly modified: CodeEditorTextDocument<editor.ITextModel>;
}

/** What one slot of the pool shows. A slot keeps the same object while nothing it shows changes. */
export interface MultiDiffSlot {
	readonly entry: MultiDiffEntry;
	readonly collapsed: boolean;
	/** The file's top edge in the content, in CSS pixels. */
	readonly top: number;
	readonly height: number;
	readonly frameHeight: number;
	/** The width of the view, which every diff editor takes. */
	readonly width: number;
}

/**
 * A mounted multi-diff: its files' heights and the offset the view shows, which files the view
 * shows, and the pool of slots that shows them. A file keeps its slot, and the slot's diff editor,
 * while it stays in sight; a file scrolling in takes a slot that a file scrolling out left, so
 * there are only as many editors as files in sight at once.
 *
 * Views read it through `subscribe` and the snapshot getters, which return the same object until
 * what they describe changes. Diff editors, which the view cannot redraw, take their offsets
 * through the scrollers their slots lend it.
 */
export class MultiDiffController {
	readonly models = new CodeEditorMultiDiffModels();
	private readonly layout = new CodeEditorMultiDiffLayout();
	private entries: readonly MultiDiffEntry[] = [];
	private readonly indexes = new Map<string, number>();
	private readonly collapsedKeys = new Set<string>();
	/** The measured height of each file's diff or placeholder, while the file is listed. */
	private readonly contentHeights = new Map<string, number>();
	private readonly slotKeys: (string | null)[] = [];
	private slotsValue: readonly (MultiDiffSlot | null)[] = [];
	private viewWidth = 0;
	/** The content height the views last heard of. */
	private announcedHeight = 0;
	private element: HTMLElement | null = null;
	private overflowWidgetsValue: HTMLElement | null = null;
	/** Whether the offset moved by the controller's own choice and the element has yet to follow. */
	private scrollPending = false;
	private readonly listeners = new Set<() => void>();
	private readonly innerScrollers = new Map<number, (scrollTop: number) => void>();

	readonly subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	};

	get contentHeight(): number {
		return this.layout.totalHeight;
	}

	get scrollTop(): number {
		return this.layout.scrollTop;
	}

	/** One entry per slot of the pool: what it shows, or null while it waits for a file. */
	get slots(): readonly (MultiDiffSlot | null)[] {
		return this.slotsValue;
	}

	slot(slot: number): MultiDiffSlot | null {
		return this.slotsValue[slot] ?? null;
	}

	clampScrollTop(offset: number): number {
		return this.layout.clampScrollTop(offset);
	}

	/**
	 * Where the diff editors put widgets such as the hover, outside every file's frame, so a widget
	 * shows above the files after its own, as VS Code's multi-diff hosts them.
	 */
	get overflowWidgets(): HTMLElement | null {
		return this.overflowWidgetsValue;
	}

	/** The element that scrolls, which follows offsets the controller chooses, and the widgets' host. */
	attach(element: HTMLElement | null, overflowWidgets: HTMLElement | null): void {
		this.element = element;
		this.overflowWidgetsValue = overflowWidgets;
	}

	setEntries(entries: readonly MultiDiffEntry[]): void {
		if (entries === this.entries) return;
		this.entries = entries;
		this.indexes.clear();
		for (const [index, entry] of entries.entries()) this.indexes.set(entry.key, index);
		for (const key of this.collapsedKeys) {
			if (!this.indexes.has(key)) this.collapsedKeys.delete(key);
		}
		for (const key of this.contentHeights.keys()) {
			if (!this.indexes.has(key)) this.contentHeights.delete(key);
		}
		this.models.retain(new Set(this.indexes.keys()));
		this.layout.reset(entries.map((entry) => this.fileHeight(entry.key)));
		this.update();
	}

	setViewport(width: number, height: number): void {
		this.viewWidth = Math.max(0, Math.floor(width));
		this.layout.setViewportHeight(Math.max(0, Math.floor(height)));
		this.update();
	}

	/** Follows the element's offset after the person scrolled it. */
	scrollTo(offset: number): void {
		this.layout.scrollTo(offset);
		this.update();
	}

	/** Expands or collapses a file; one collapsed from its header at the top keeps the header there. */
	toggle(key: string): void {
		const index = this.indexes.get(key);
		if (index === undefined) return;
		if (!this.collapsedKeys.delete(key)) this.collapsedKeys.add(key);
		const { top } = this.layout.geometry(index);
		this.layout.setHeight(index, this.fileHeight(key));
		if (top < this.layout.scrollTop) {
			this.layout.scrollTo(top);
			this.scrollPending = true;
		}
		this.update();
	}

	/** Records the height of a file's diff, or of the placeholder in its place. */
	setContentHeight(key: string, height: number): void {
		if (this.contentHeights.get(key) === height) return;
		this.contentHeights.set(key, height);
		const index = this.indexes.get(key);
		if (index === undefined || this.collapsedKeys.has(key)) return;
		const offset = this.layout.scrollTop;
		this.layout.setHeight(index, this.fileHeight(key));
		if (this.layout.scrollTop !== offset) this.scrollPending = true;
		this.update();
	}

	/** Lends the controller the scroller of a slot's diff editor, or takes it back with null. */
	setInnerScroller(slot: number, scroller: ((scrollTop: number) => void) | null): void {
		if (scroller === null) this.innerScrollers.delete(slot);
		else this.innerScrollers.set(slot, scroller);
	}

	/** How far the diff editor of a slot is scrolled for the view's offset. */
	innerScrollTop(slot: number): number {
		const index = this.slotIndex(slot);
		return index === undefined ? 0 : this.layout.geometry(index).innerScrollTop;
	}

	/**
	 * Follows a diff editor that scrolled itself, as to reveal its cursor: the view moves so that the
	 * file sits at its top, scrolled as its editor is.
	 */
	followInnerScroll(slot: number, scrollTop: number): void {
		const index = this.slotIndex(slot);
		if (index === undefined) return;
		const geometry = this.layout.geometry(index);
		if (Math.abs(scrollTop - geometry.innerScrollTop) < 1) return;
		this.layout.scrollTo(
			geometry.top + Math.min(Math.max(0, scrollTop), geometry.height - geometry.frameHeight),
		);
		this.scrollPending = true;
		this.update();
	}

	/** Moves the element to an offset the controller chose, once the content it needs has rendered. */
	flushScroll(): void {
		if (!this.scrollPending || this.element === null) return;
		this.scrollPending = false;
		this.element.scrollTop = this.layout.scrollTop;
	}

	dispose(): void {
		this.models.dispose();
	}

	private slotIndex(slot: number): number | undefined {
		const key = this.slotKeys[slot];
		return key === null || key === undefined ? undefined : this.indexes.get(key);
	}

	private fileHeight(key: string): number {
		if (this.collapsedKeys.has(key)) return MULTI_DIFF_HEADER_HEIGHT;
		const content = this.contentHeights.get(key);
		return content === undefined
			? MULTI_DIFF_ESTIMATED_FILE_HEIGHT
			: MULTI_DIFF_HEADER_HEIGHT + content + MULTI_DIFF_BOTTOM_PADDING;
	}

	/** Lends the slots to the files in sight, scrolls their editors, and tells the views. */
	private update(): void {
		const range = this.layout.visibleRange();
		const visible = new Set<string>();
		if (range !== null) {
			for (const entry of this.entries.slice(range.first, range.last + 1)) visible.add(entry.key);
		}
		const placed = new Set<string>();
		for (const [slot, key] of this.slotKeys.entries()) {
			if (key === null) continue;
			if (visible.has(key)) placed.add(key);
			else this.slotKeys[slot] = null;
		}
		for (const key of visible) {
			if (placed.has(key)) continue;
			const free = this.slotKeys.indexOf(null);
			if (free === -1) this.slotKeys.push(key);
			else this.slotKeys[free] = key;
		}
		let changed = this.slotsValue.length !== this.slotKeys.length;
		const slots = this.slotKeys.map((key, slot) => {
			const previous = this.slotsValue[slot] ?? null;
			const next = key === null ? null : this.describe(key, previous);
			if (next !== previous) changed = true;
			return next;
		});
		if (changed) this.slotsValue = slots;
		const heightChanged = this.layout.totalHeight !== this.announcedHeight;
		this.announcedHeight = this.layout.totalHeight;
		for (const [slot, scroller] of this.innerScrollers) scroller(this.innerScrollTop(slot));
		for (const listener of this.listeners) listener();
		// An offset that waits for no new content can move the element now.
		if (!changed && !heightChanged) this.flushScroll();
	}

	private describe(key: string, previous: MultiDiffSlot | null): MultiDiffSlot | null {
		const index = this.indexes.get(key);
		const entry = index === undefined ? undefined : this.entries[index];
		if (index === undefined || entry === undefined) return null;
		const { frameHeight, height, top } = this.layout.geometry(index);
		const collapsed = this.collapsedKeys.has(key);
		if (
			previous?.entry === entry &&
			previous.collapsed === collapsed &&
			previous.top === top &&
			previous.height === height &&
			previous.frameHeight === frameHeight &&
			previous.width === this.viewWidth
		) {
			return previous;
		}
		return { collapsed, entry, frameHeight, height, top, width: this.viewWidth };
	}
}
