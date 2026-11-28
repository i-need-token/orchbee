import { HeightIndex } from '../list-scrolling';

/** Where one file of a multi-diff sits, in CSS pixels. */
export interface CodeEditorMultiDiffGeometry {
	/** The file's top edge in the content. */
	readonly top: number;
	/** The file's whole height: its header, and its diff unless it is collapsed. */
	readonly height: number;
	/**
	 * The height the file renders at: its whole height, up to the view's. While the view scrolls
	 * through a taller file, the file stays in sight and its diff editor scrolls instead.
	 */
	readonly frameHeight: number;
	/** How far the file's diff editor is scrolled, so it shows the part the view's offset reaches. */
	readonly innerScrollTop: number;
}

/**
 * A multi-diff's vertical layout as VS Code's multi-diff editor computes it: files follow one
 * another without gaps, and none renders taller than the view, so a long diff never needs an editor
 * taller than the screen. The offset counts every file's whole height, so the scrollbar measures the
 * whole list.
 */
export class CodeEditorMultiDiffLayout {
	private readonly heights = new HeightIndex();
	private viewHeight = 0;
	private offset = 0;

	get totalHeight(): number {
		return this.heights.totalHeight;
	}

	get scrollTop(): number {
		return this.offset;
	}

	/** Replaces every file's height, as when the list changes. */
	reset(heights: ArrayLike<number>): void {
		this.heights.splice(0, this.heights.count, heights);
		this.offset = this.clampScrollTop(this.offset);
	}

	setViewportHeight(height: number): void {
		this.viewHeight = Math.max(0, height);
		this.offset = this.clampScrollTop(this.offset);
	}

	scrollTo(offset: number): void {
		this.offset = this.clampScrollTop(offset);
	}

	clampScrollTop(offset: number): number {
		return Math.max(0, Math.min(offset, this.heights.totalHeight - this.viewHeight));
	}

	/**
	 * Changes one file's height. A file wholly above the view moves the offset by the change, so the
	 * files in sight stay where they are; any other file grows or shrinks in place.
	 */
	setHeight(index: number, height: number): void {
		const previous = this.heights.heightOf(index);
		if (previous === height) return;
		const above = this.heights.positionOf(index) + previous <= this.offset;
		this.heights.setHeight(index, height);
		this.offset = this.clampScrollTop(above ? this.offset + height - previous : this.offset);
	}

	/** The file at the top of the view and how far into it the view is scrolled; null without files. */
	anchor(): { readonly index: number; readonly offset: number } | null {
		if (this.heights.count === 0) return null;
		const index = this.heights.indexAt(this.offset);
		return { index, offset: this.offset - this.heights.positionOf(index) };
	}

	/** The first and last files in sight, or null while the view shows none. */
	visibleRange(): { readonly first: number; readonly last: number } | null {
		if (this.heights.count === 0 || this.viewHeight === 0) return null;
		return {
			first: this.heights.indexAt(this.offset),
			last: this.heights.indexAt(this.offset + this.viewHeight - 1),
		};
	}

	geometry(index: number): CodeEditorMultiDiffGeometry {
		const top = this.heights.positionOf(index);
		const height = this.heights.heightOf(index);
		const frameHeight = Math.min(height, this.viewHeight);
		return {
			frameHeight,
			height,
			innerScrollTop: Math.min(Math.max(0, this.offset - top), height - frameHeight),
			top,
		};
	}
}
