/** The document attribute the package stylesheet reads to show `grabbing` on every element. */
const DRAG_CURSOR_ATTRIBUTE = 'data-ob-drag-cursor';

const holds = new WeakMap<Document, number>();

/**
 * Shows the `grabbing` pointer over the whole document while a drag is held, whatever element the
 * pointer crosses, and returns the release. Releases are idempotent, and overlapping holds keep the
 * cursor until the last one ends.
 */
export function holdDragCursor(ownerDocument: Document): () => void {
	const root = ownerDocument.documentElement;
	holds.set(ownerDocument, (holds.get(ownerDocument) ?? 0) + 1);
	root.setAttribute(DRAG_CURSOR_ATTRIBUTE, '');
	let released = false;
	return () => {
		if (released) return;
		released = true;
		const remaining = (holds.get(ownerDocument) ?? 1) - 1;
		if (remaining > 0) {
			holds.set(ownerDocument, remaining);
			return;
		}
		holds.delete(ownerDocument);
		root.removeAttribute(DRAG_CURSOR_ATTRIBUTE);
	};
}
