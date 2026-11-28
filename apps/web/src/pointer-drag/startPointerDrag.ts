import { holdDragCursor } from '@orchbee/ui/components';

import { attachCargo, type PointerDragCargo } from './pointer-drag-cargo';
import styles from './PointerDragLabel.module.css';
import type {
	PointerDrag,
	PointerDragModifiers,
	PointerDragPoint,
	PointerDropTarget,
} from './pointer-drag-types';
import { dropTargetAt } from './pointer-drop-targets';

/** The Tab Strip's threshold: a press turns into a drag once the pointer travels farther. */
const DRAG_THRESHOLD = 10;

/** How far right of and below the pointer the label sits, as VS Code places its drag image. */
const LABEL_OFFSET = 10;

/** The bit of `PointerEvent.buttons` for the primary button. */
const PRIMARY_BUTTON = 1;

/** A drag's own record, which its targets see as a `PointerDrag`. */
class ActivePointerDrag implements PointerDrag {
	readonly label: string;
	modifiers: PointerDragModifiers;

	constructor(label: string, modifiers: PointerDragModifiers) {
		this.label = label;
		this.modifiers = modifiers;
	}
}

/** Keeps a click from its element and from the browser's default action. */
function swallowEvent(event: MouseEvent): void {
	event.preventDefault();
	event.stopPropagation();
}

function modifiersOf(event: KeyboardEvent | PointerEvent): PointerDragModifiers {
	return {
		altKey: event.altKey,
		ctrlKey: event.ctrlKey,
		metaKey: event.metaKey,
		shiftKey: event.shiftKey,
	};
}

/**
 * Follows a press of the primary button on `holder` and starts a drag once the pointer travels past
 * the Tab Strip's 10 px. `start` then names what the drag carries, or returns null to leave the press
 * a click. While the drag runs, `holder` keeps the pointer, the `grabbing` cursor shows everywhere,
 * the cargo's label follows the pointer, and the drop target under the pointer, found through
 * `elementFromPoint`, hears the drag on every frame. Every key belongs to the drag: the modifiers
 * reach the target, and Esc cancels, as the window losing focus or the browser taking the pointer
 * does. A release drops on a target that accepts, and the click the press would end in never arrives.
 */
export function startPointerDrag(
	press: PointerEvent,
	holder: Element,
	start: () => PointerDragCargo | null,
): void {
	const ownerDocument = holder.ownerDocument;
	const view = ownerDocument.defaultView;
	if (view === null || press.button !== 0 || !press.isPrimary) return;
	const { pointerId } = press;
	const originX = press.clientX;
	const originY = press.clientY;
	let clientX = originX;
	let clientY = originY;
	let drag: ActivePointerDrag | null = null;
	let label: HTMLElement | null = null;
	let captured = false;
	let releaseCursor: (() => void) | null = null;
	let frame: number | null = null;
	let target: PointerDropTarget | null = null;
	/** Where the target under the pointer last accepted the drag; null while it refuses. */
	let accepted: PointerDragPoint | null = null;

	const placeLabel = () => {
		label?.style.setProperty(
			'--ob-comp-pointer-drag-label-inline-start',
			`${clientX + LABEL_OFFSET}px`,
		);
		label?.style.setProperty(
			'--ob-comp-pointer-drag-label-block-start',
			`${clientY + LABEL_OFFSET}px`,
		);
	};
	/** Finds the target under the pointer and lets it hear the drag. */
	const follow = (active: ActivePointerDrag) => {
		const hit = dropTargetAt(ownerDocument, clientX, clientY);
		if (target !== null && target !== hit?.target) {
			const left = target;
			target = null;
			accepted = null;
			left.leave(active);
		}
		if (hit === null) return;
		target = hit.target;
		const point = { clientX, clientY, element: hit.element };
		accepted = hit.target.over(active, point) ? point : null;
	};
	const tick = () => {
		frame = view.requestAnimationFrame(tick);
		if (drag !== null) follow(drag);
	};
	/** Swallows the click, or double click, that the release of this press is about to cause. */
	const swallowClick = () => {
		view.addEventListener('click', swallowEvent, true);
		view.addEventListener('dblclick', swallowEvent, true);
		view.setTimeout(() => {
			view.removeEventListener('click', swallowEvent, true);
			view.removeEventListener('dblclick', swallowEvent, true);
		}, 0);
	};
	/** Swallows the click that ends a cancelled drag's press, unless the pointer presses again first. */
	const awaitRelease = () => {
		const settle = (event: PointerEvent) => {
			if (event.pointerId !== pointerId) return;
			view.removeEventListener('pointerup', settle, true);
			view.removeEventListener('pointerdown', settle, true);
			view.removeEventListener('pointercancel', settle, true);
			if (event.type === 'pointerup') swallowClick();
		};
		view.addEventListener('pointerup', settle, true);
		view.addEventListener('pointerdown', settle, true);
		view.addEventListener('pointercancel', settle, true);
	};

	const onPointerMove = (event: PointerEvent) => {
		if (event.pointerId !== pointerId) return;
		// Without the primary button the press is over, though its release went unseen.
		if ((event.buttons & PRIMARY_BUTTON) === 0) {
			cancel(false);
			return;
		}
		clientX = event.clientX;
		clientY = event.clientY;
		if (drag !== null) {
			event.preventDefault();
			drag.modifiers = modifiersOf(event);
			placeLabel();
			return;
		}
		if (Math.hypot(clientX - originX, clientY - originY) <= DRAG_THRESHOLD) return;
		const cargo = start();
		if (cargo === null) {
			stop();
			return;
		}
		begin(cargo, event);
	};
	const onPointerUp = (event: PointerEvent) => {
		if (event.pointerId !== pointerId) return;
		const active = drag;
		if (active === null) {
			stop();
			return;
		}
		clientX = event.clientX;
		clientY = event.clientY;
		active.modifiers = modifiersOf(event);
		follow(active);
		const landed = target;
		const point = accepted;
		target = null;
		stop();
		swallowClick();
		if (landed === null) return;
		if (point === null) landed.leave(active);
		else landed.drop(active, point);
	};
	const onKey = (event: KeyboardEvent) => {
		const active = drag;
		if (active === null) return;
		event.preventDefault();
		event.stopPropagation();
		active.modifiers = modifiersOf(event);
		if (event.type === 'keydown' && event.key === 'Escape') cancel(true);
		else follow(active);
	};
	/** The browser took the pointer, as for a touch that scrolls: no release follows. */
	const onPointerCancel = (event: PointerEvent) => {
		if (event.pointerId === pointerId) cancel(false);
	};
	/** Another press of the pointer means its release went unseen: the press is over. */
	const onPointerDown = (event: PointerEvent) => {
		if (event.pointerId === pointerId) cancel(false);
	};
	/** The window lost focus with the button still down. */
	const onBlur = () => {
		cancel(true);
	};
	/** A link or an image in the holder would start the browser's own drag instead. */
	const onNativeDragStart = (event: DragEvent) => {
		if (event.target instanceof view.Node && holder.contains(event.target)) event.preventDefault();
	};
	const onSelectStart = (event: Event) => {
		if (drag !== null) event.preventDefault();
	};

	const begin = (cargo: PointerDragCargo, event: PointerEvent) => {
		const active = new ActivePointerDrag(cargo.label, modifiersOf(event));
		drag = active;
		attachCargo(cargo, active);
		try {
			holder.setPointerCapture(pointerId);
			captured = true;
		} catch {
			// The pointer is gone, or the host keeps no capture; the window still hears its release.
		}
		releaseCursor = holdDragCursor(ownerDocument);
		label = ownerDocument.createElement('div');
		label.className = styles['label'] ?? '';
		label.setAttribute('aria-hidden', 'true');
		label.textContent = cargo.label;
		placeLabel();
		ownerDocument.body.append(label);
		view.addEventListener('keydown', onKey, true);
		view.addEventListener('keyup', onKey, true);
		view.addEventListener('blur', onBlur);
		follow(active);
		frame = view.requestAnimationFrame(tick);
	};
	/**
	 * Ends the press without dropping. A drag cancelled with the button still down ends its press
	 * without a click.
	 */
	const cancel = (pressed: boolean) => {
		const active = drag;
		const left = target;
		target = null;
		stop();
		if (active === null) return;
		left?.leave(active);
		if (pressed) awaitRelease();
	};
	const stop = () => {
		view.removeEventListener('pointermove', onPointerMove, true);
		view.removeEventListener('pointerup', onPointerUp, true);
		view.removeEventListener('pointercancel', onPointerCancel, true);
		view.removeEventListener('pointerdown', onPointerDown, true);
		view.removeEventListener('dragstart', onNativeDragStart, true);
		view.removeEventListener('selectstart', onSelectStart, true);
		view.removeEventListener('keydown', onKey, true);
		view.removeEventListener('keyup', onKey, true);
		view.removeEventListener('blur', onBlur);
		if (frame !== null) view.cancelAnimationFrame(frame);
		frame = null;
		if (captured) {
			captured = false;
			try {
				holder.releasePointerCapture(pointerId);
			} catch {
				// The capture already ended with the pointer.
			}
		}
		releaseCursor?.();
		releaseCursor = null;
		label?.remove();
		label = null;
		drag = null;
		accepted = null;
	};

	view.addEventListener('pointermove', onPointerMove, true);
	view.addEventListener('pointerup', onPointerUp, true);
	view.addEventListener('pointercancel', onPointerCancel, true);
	view.addEventListener('pointerdown', onPointerDown, true);
	view.addEventListener('dragstart', onNativeDragStart, true);
	view.addEventListener('selectstart', onSelectStart, true);
}
