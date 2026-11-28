/** The modifier keys held during a drag, which a target reads to choose between moving and copying. */
export interface PointerDragModifiers {
	readonly altKey: boolean;
	readonly ctrlKey: boolean;
	readonly metaKey: boolean;
	readonly shiftKey: boolean;
}

/** A drag in progress, as its targets see it. The kind of its cargo reads what it carries. */
export interface PointerDrag {
	/** The text beside the pointer. */
	readonly label: string;
	/** The modifier keys held now. */
	readonly modifiers: PointerDragModifiers;
}

/** Where the pointer is during a drag. */
export interface PointerDragPoint {
	readonly clientX: number;
	readonly clientY: number;
	/** The topmost element under the pointer, inside the target. */
	readonly element: Element;
}

/**
 * A place a drag can drop on. While a drag is over it, `over` runs on every frame, as the browser
 * repeats `dragover`, so a target that scrolls or expands beneath a still pointer follows the
 * change. Exactly one of `leave` and `drop` ends each stay; either clears what `over` showed.
 */
export interface PointerDropTarget {
	/** Whether a drop at `point` would land; the target shows where it would. */
	over(drag: PointerDrag, point: PointerDragPoint): boolean;
	/** The drag left the target, was cancelled, or was released where `over` refused it. */
	leave(drag: PointerDrag): void;
	/** The drag was released at `point`, which `over` accepted. */
	drop(drag: PointerDrag, point: PointerDragPoint): void;
}
