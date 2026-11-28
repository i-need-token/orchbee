import type { PointerDropTarget } from './pointer-drag-types';

/** Marks an element that a drag can drop on, so the element under the pointer finds it. */
const DROP_TARGET_ATTRIBUTE = 'data-pointer-drop-target';
const DROP_TARGET_SELECTOR = `[${DROP_TARGET_ATTRIBUTE}]`;

const targets = new WeakMap<Element, PointerDropTarget>();

/** A drop target under the pointer, and the topmost element there. */
interface DropTargetHit {
	readonly target: PointerDropTarget;
	readonly element: Element;
}

/**
 * Lets drags drop on `element` through `target` until the returned function runs. The innermost
 * registered element under the pointer takes the drag.
 */
export function registerPointerDropTarget(element: Element, target: PointerDropTarget): () => void {
	targets.set(element, target);
	element.setAttribute(DROP_TARGET_ATTRIBUTE, '');
	return () => {
		if (targets.get(element) !== target) return;
		targets.delete(element);
		element.removeAttribute(DROP_TARGET_ATTRIBUTE);
	};
}

/** The drop target under a point of the document, found through `elementFromPoint`. */
export function dropTargetAt(
	ownerDocument: Document,
	clientX: number,
	clientY: number,
): DropTargetHit | null {
	const element = ownerDocument.elementFromPoint(clientX, clientY);
	if (element === null) return null;
	for (
		let candidate = element.closest(DROP_TARGET_SELECTOR);
		candidate !== null;
		candidate = candidate.parentElement?.closest(DROP_TARGET_SELECTOR) ?? null
	) {
		const target = targets.get(candidate);
		if (target !== undefined) return { element, target };
	}
	return null;
}
