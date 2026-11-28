import { CodeEditorMultiDiffLayout } from '../../CodeEditorMultiDiffLayout';
import { CodeEditorMultiDiffModels } from '../../CodeEditorMultiDiffModels';
import type { CodeEditorTextDocument } from '../../CodeEditorTextDocument';
import type { editor } from '../../monaco-entry';
import {
	MULTI_DIFF_BOTTOM_PADDING,
	MULTI_DIFF_ESTIMATED_FILE_HEIGHT,
	MULTI_DIFF_HEADER_HEIGHT,
} from './constants/multi-diff-geometry';

/** A diff editor's selections in its modified side. */
type MultiDiffSelections = Parameters<editor.ICodeEditor['setSelections']>[0];

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

/** How one file of a multi-diff was left. */
interface MultiDiffFileState {
	readonly collapsed: boolean;
	/** The measured height of the file's diff or placeholder, which places the files below it. */
	readonly contentHeight: number | undefined;
	readonly selections: MultiDiffSelections | undefined;
}

/**
 * Where a multi-diff was scrolled and how its files were left, as VS Code's multi-diff editor keeps
 * its view state: the offsets, the file at the top of the view and how far into it, and each file's
 * collapsed state and selections. A file's measured height comes along, as VS Code's view model keeps
 * it, so the view returns to the same place without measuring the files above it again.
 */
export interface CodeEditorMultiDiffViewState {
	/** The file at the top of the view and how far into it, which outlasts changes to the list. */
	readonly anchor: { readonly key: string; readonly offset: number } | null;
	readonly files: ReadonlyMap<string, MultiDiffFileState>;
	readonly scrollLeft: number;
	readonly scrollTop: number;
}

/** The part of a slot's diff editor that the controller drives. */
export interface MultiDiffSlotEditor {
	/** The file the editor shows. */
	readonly key: string;
	/** Scrolls the editor to the part of its file the view's offsets reach. */
	scrollTo(scrollTop: number, scrollLeft: number): void;
	selections(): MultiDiffSelections | null;
}

/**
 * A mounted multi-diff: its files' heights and the offsets the view shows, which files the view
 * shows, and the pool of slots that shows them. A file keeps its slot, and the slot's diff editor,
 * while it stays in sight; a file scrolling in takes a slot that a file scrolling out left, so
 * there are only as many editors as files in sight at once.
 *
 * All files share one sideways offset, as in VS Code's multi-diff, which the files in sight bound:
 * the view scrolls as far as the widest of them overflows it.
 *
 * Views read it through `subscribe` and the snapshot getters, which return the same value until
 * what they describe changes. Diff editors, which the view cannot redraw, take their offsets
 * through the slot editors they lend it.
 */
export class MultiDiffController {
	readonly models = new CodeEditorMultiDiffModels();
	private readonly layout = new CodeEditorMultiDiffLayout();
	private entries: readonly MultiDiffEntry[] = [];
	private readonly indexes = new Map<string, number>();
	private readonly collapsedKeys = new Set<string>();
	/** The measured height of each file's diff or placeholder, while the file is listed. */
	private readonly contentHeights = new Map<string, number>();
	/** Each file's selections from when it last left its editor, while the file is listed. */
	private readonly keptSelections = new Map<string, MultiDiffSelections>();
	private readonly slotKeys: (string | null)[] = [];
	private slotsValue: readonly (MultiDiffSlot | null)[] = [];
	private viewWidth = 0;
	/** How far each slot's diff overflows the view sideways, while it shows one. */
	private readonly overflows = new Map<number, number>();
	private maxScrollLeftValue = 0;
	private scrollLeftValue = 0;
	/** A restored sideways offset that waits for files wide enough to reach it. */
	private pendingScrollLeft: number | null = null;
	/** The sizes the views last heard of. */
	private announcedHeight = 0;
	private announcedScrollWidth = 0;
	private element: HTMLElement | null = null;
	private horizontalElement: HTMLElement | null = null;
	private overflowWidgetsValue: HTMLElement | null = null;
	/** Whether an offset moved by the controller's own choice and its scroller has yet to follow. */
	private scrollPending = false;
	private scrollLeftPending = false;
	private readonly listeners = new Set<() => void>();
	private readonly slotEditors = new Map<number, MultiDiffSlotEditor>();

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

	get scrollLeft(): number {
		return this.scrollLeftValue;
	}

	/** How far the view scrolls sideways: as far as the widest file in sight overflows it. */
	get maxScrollLeft(): number {
		return this.maxScrollLeftValue;
	}

	/** The width of the view, which the sideways scrollbar spans. */
	get viewportWidth(): number {
		return this.viewWidth;
	}

	/** Whether every file is collapsed, which turns VS Code's Collapse All Diffs into Expand All. */
	get allCollapsed(): boolean {
		return this.entries.every((entry) => this.collapsedKeys.has(entry.key));
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

	clampScrollLeft(offset: number): number {
		return Math.max(0, Math.min(offset, this.maxScrollLeftValue));
	}

	/**
	 * Where the diff editors put widgets such as the hover, outside every file's frame, so a widget
	 * shows above the files after its own, as VS Code's multi-diff hosts them.
	 */
	get overflowWidgets(): HTMLElement | null {
		return this.overflowWidgetsValue;
	}

	/**
	 * The elements that scroll, which follow offsets the controller chooses: the list, and the
	 * sideways scrollbar under it. The widgets' host goes with them.
	 */
	attach(
		element: HTMLElement | null,
		horizontalElement: HTMLElement | null,
		overflowWidgets: HTMLElement | null,
	): void {
		this.element = element;
		this.horizontalElement = horizontalElement;
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
		for (const key of this.keptSelections.keys()) {
			if (!this.indexes.has(key)) this.keptSelections.delete(key);
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

	/** Follows the list's offset after the person scrolled it. */
	scrollTo(offset: number): void {
		// The list's own echo of an offset the controller chose leaves a restored offset waiting.
		if (Math.abs(offset - this.layout.scrollTop) >= 1) this.pendingScrollLeft = null;
		this.layout.scrollTo(offset);
		this.update();
	}

	/**
	 * Follows sideways scrolling: the person's, from the wheel or the scrollbar, or a diff editor's that
	 * scrolled itself, as to reveal its cursor, which every file then follows.
	 */
	scrollLeftTo(offset: number): void {
		this.pendingScrollLeft = null;
		const next = this.clampScrollLeft(offset);
		if (next === this.scrollLeftValue) return;
		this.scrollLeftValue = next;
		this.scrollLeftPending = true;
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

	/** VS Code's Collapse All Diffs. */
	collapseAll(): void {
		this.setAllCollapsed(true);
	}

	/** VS Code's Expand All Diffs. */
	expandAll(): void {
		this.setAllCollapsed(false);
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

	/** Records how far a slot's diff overflows the view sideways; 0 once it shows none. */
	setOverflow(slot: number, overflow: number): void {
		const value = Math.max(0, overflow);
		if ((this.overflows.get(slot) ?? 0) === value) return;
		if (value === 0) this.overflows.delete(slot);
		else this.overflows.set(slot, value);
		let widest = 0;
		for (const each of this.overflows.values()) widest = Math.max(widest, each);
		// The scrollbar shows again at the shared offset.
		if (this.maxScrollLeftValue === 0 && widest > 0) this.scrollLeftPending = true;
		this.maxScrollLeftValue = widest;
		this.placeScrollLeft(this.pendingScrollLeft ?? this.scrollLeftValue);
		this.update();
	}

	/**
	 * Lends the controller a slot's diff editor, or takes it back with null. The editor scrolls to the
	 * offsets of the file it shows at once, and then as the view scrolls.
	 */
	setSlotEditor(slot: number, slotEditor: MultiDiffSlotEditor | null): void {
		if (slotEditor === null) {
			const lent = this.slotEditors.get(slot);
			if (lent !== undefined) this.keepSelections(lent);
			this.slotEditors.delete(slot);
			return;
		}
		this.slotEditors.set(slot, slotEditor);
		slotEditor.scrollTo(this.innerScrollTop(slotEditor.key), this.scrollLeftValue);
	}

	/** The selections a file had when it last left its editor, for the next editor that shows it. */
	selectionsOf(key: string): MultiDiffSelections | undefined {
		return this.keptSelections.get(key);
	}

	/**
	 * Follows a diff editor that scrolled itself, as to reveal its cursor: the view moves so that the
	 * file sits at its top, scrolled as its editor is.
	 */
	followInnerScroll(key: string, scrollTop: number): void {
		const index = this.indexes.get(key);
		if (index === undefined) return;
		const geometry = this.layout.geometry(index);
		if (Math.abs(scrollTop - geometry.innerScrollTop) < 1) return;
		this.layout.scrollTo(
			geometry.top + Math.min(Math.max(0, scrollTop), geometry.height - geometry.frameHeight),
		);
		this.scrollPending = true;
		this.update();
	}

	/**
	 * Moves the scrollers to offsets the controller chose, once the content they need has rendered:
	 * the view passes the content height and the scroll width it rendered last.
	 */
	flushScroll(rendered: { readonly contentHeight: number; readonly scrollWidth: number }): void {
		if (
			this.scrollPending &&
			this.element !== null &&
			rendered.contentHeight === this.layout.totalHeight
		) {
			this.scrollPending = false;
			this.element.scrollTop = this.layout.scrollTop;
		}
		if (
			this.scrollLeftPending &&
			this.horizontalElement !== null &&
			rendered.scrollWidth === this.viewWidth + this.maxScrollLeftValue
		) {
			this.scrollLeftPending = false;
			this.horizontalElement.scrollLeft = this.scrollLeftValue;
		}
	}

	/** The view state to keep as the view lets go of its files. */
	viewState(): CodeEditorMultiDiffViewState {
		for (const slotEditor of this.slotEditors.values()) this.keepSelections(slotEditor);
		const files = new Map<string, MultiDiffFileState>();
		for (const { key } of this.entries) {
			files.set(key, {
				collapsed: this.collapsedKeys.has(key),
				contentHeight: this.contentHeights.get(key),
				selections: this.keptSelections.get(key),
			});
		}
		const anchor = this.layout.anchor();
		const anchorKey = anchor === null ? undefined : this.entries[anchor.index]?.key;
		return {
			anchor:
				anchor === null || anchorKey === undefined
					? null
					: { key: anchorKey, offset: anchor.offset },
			files,
			scrollLeft: this.scrollLeftValue,
			scrollTop: this.layout.scrollTop,
		};
	}

	/**
	 * Shows the listed files as a view state left them, or from the start without one. The file at
	 * the top returns to the top; a sideways offset waits until files wide enough to reach it show.
	 */
	restore(state: CodeEditorMultiDiffViewState | undefined): void {
		this.collapsedKeys.clear();
		this.keptSelections.clear();
		for (const [key, file] of state?.files ?? []) {
			if (!this.indexes.has(key)) continue;
			if (file.collapsed) this.collapsedKeys.add(key);
			if (file.selections !== undefined) this.keptSelections.set(key, file.selections);
			// A height measured since stays: it is the file's diff as it is now.
			if (file.contentHeight !== undefined && !this.contentHeights.has(key)) {
				this.contentHeights.set(key, file.contentHeight);
			}
		}
		this.layout.reset(this.entries.map((entry) => this.fileHeight(entry.key)));
		const anchor = state?.anchor ?? null;
		const anchorIndex = anchor === null ? undefined : this.indexes.get(anchor.key);
		if (anchor === null || anchorIndex === undefined) this.layout.scrollTo(state?.scrollTop ?? 0);
		else this.placeAnchor(anchorIndex, anchor.offset);
		this.scrollPending = true;
		this.scrollLeftValue = 0;
		this.scrollLeftPending = true;
		const scrollLeft = state?.scrollLeft ?? 0;
		this.pendingScrollLeft = scrollLeft > 0 ? scrollLeft : null;
		this.placeScrollLeft(scrollLeft);
		this.update();
	}

	dispose(): void {
		this.models.dispose();
	}

	private keepSelections(slotEditor: MultiDiffSlotEditor): void {
		if (!this.indexes.has(slotEditor.key)) return;
		const selections = slotEditor.selections();
		if (selections !== null) this.keptSelections.set(slotEditor.key, selections);
	}

	/** Collapses or expands every file, keeping the file at the top of the view at the top. */
	private setAllCollapsed(collapsed: boolean): void {
		const anchor = this.layout.anchor();
		if (collapsed) {
			for (const { key } of this.entries) this.collapsedKeys.add(key);
		} else {
			this.collapsedKeys.clear();
		}
		this.layout.reset(this.entries.map((entry) => this.fileHeight(entry.key)));
		if (anchor !== null) this.placeAnchor(anchor.index, anchor.offset);
		this.scrollPending = true;
		this.update();
	}

	/**
	 * Scrolls a file to the top of the view, as far into it as before; an offset past the file's end
	 * shows the file from its start, as VS Code maps the view's top through a file that shrank.
	 */
	private placeAnchor(index: number, offset: number): void {
		const { height, top } = this.layout.geometry(index);
		this.layout.scrollTo(top + (offset < height ? offset : 0));
	}

	/** Moves the sideways offset toward `wanted` as far as the files in sight reach. */
	private placeScrollLeft(wanted: number): void {
		const next = this.clampScrollLeft(wanted);
		if (this.pendingScrollLeft !== null && next >= this.pendingScrollLeft) {
			this.pendingScrollLeft = null;
		}
		if (next === this.scrollLeftValue) return;
		this.scrollLeftValue = next;
		this.scrollLeftPending = true;
	}

	/** How far the diff editor of a file is scrolled for the view's offset. */
	private innerScrollTop(key: string): number {
		const index = this.indexes.get(key);
		return index === undefined ? 0 : this.layout.geometry(index).innerScrollTop;
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
		const scrollWidth = this.viewWidth + this.maxScrollLeftValue;
		const widthChanged = scrollWidth !== this.announcedScrollWidth;
		this.announcedScrollWidth = scrollWidth;
		for (const slotEditor of this.slotEditors.values()) {
			slotEditor.scrollTo(this.innerScrollTop(slotEditor.key), this.scrollLeftValue);
		}
		for (const listener of this.listeners) listener();
		// Offsets that wait for no new content can move the scrollers now.
		if (!changed && !heightChanged && !widthChanged) {
			this.flushScroll({ contentHeight: this.layout.totalHeight, scrollWidth });
		}
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
