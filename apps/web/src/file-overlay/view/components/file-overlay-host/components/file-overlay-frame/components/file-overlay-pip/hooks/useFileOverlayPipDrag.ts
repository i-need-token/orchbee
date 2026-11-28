import { useMemoizedFn } from '@orchbee/foundation/react';
import { holdDragCursor } from '@orchbee/ui/components';
import {
	useEffect,
	useLayoutEffect,
	useRef,
	type PointerEvent as ReactPointerEvent,
	type RefObject,
} from 'react';

import {
	fileOverlayPipPosition,
	moveFileOverlayPip,
	type FileOverlayBox,
	type FileOverlayPipPosition,
	type FileOverlayRegions,
} from '../../../../../../../../model';

/** The Tab Strip's threshold: a press turns into a drag once the pointer travels farther. */
const DRAG_THRESHOLD = 10;

/** The bit of `PointerEvent.buttons` for the primary button. */
const PRIMARY_BUTTON = 1;

/** The frame's own copy of picture-in-picture's corner, which follows the pointer while it drags. */
const LEFT_PROPERTY = '--ob-comp-file-overlay-pip-inline-start';
const TOP_PROPERTY = '--ob-comp-file-overlay-pip-block-start';

interface FileOverlayPipDragOptions {
	/** Where picture-in-picture shows, which a drag starts from. */
	readonly box: FileOverlayBox;
	/** The frame, which a drag moves until the release keeps where it landed. */
	readonly frameReference: RefObject<HTMLElement | null>;
	/** Keeps where a drag left picture-in-picture. */
	readonly move: (position: FileOverlayPipPosition) => void;
	/** What the overlay lies over, which bounds a drag. */
	readonly regions: FileOverlayRegions;
}

interface FileOverlayPipDrag {
	/** Whether the click that just arrived ends a drag, which moved picture-in-picture instead. */
	readonly moved: () => boolean;
	readonly onPointerDown: (event: ReactPointerEvent<HTMLElement>) => void;
}

/**
 * Follows a press of the primary button on picture-in-picture, which leaves the keys where they
 * are. Once the pointer travels past the Tab Strip's 10 px, picture-in-picture follows it inside the
 * window below the 36px row under the grabbing cursor, and the release keeps where it landed; the
 * click the press ends in restores nothing. Every key belongs to the drag, and Esc puts
 * picture-in-picture back, as the window losing focus or the browser taking the pointer does.
 */
export function useFileOverlayPipDrag({
	box,
	frameReference,
	move,
	regions,
}: FileOverlayPipDragOptions): FileOverlayPipDrag {
	// A drag reads the layout as it is when the pointer moves, as a resize may change it.
	const latest = useRef({ box, regions });
	useLayoutEffect(() => {
		latest.current = { box, regions };
	});
	const stopDrag = useRef<(() => void) | null>(null);
	useEffect(() => () => stopDrag.current?.(), []);
	const movedReference = useRef(false);
	const keep = useMemoizedFn(move);

	const onPointerDown = useMemoizedFn((press: ReactPointerEvent<HTMLElement>) => {
		if (press.button !== 0 || !press.isPrimary || stopDrag.current !== null) return;
		const holder = press.currentTarget;
		const frame = frameReference.current;
		const view = holder.ownerDocument.defaultView;
		if (frame === null || view === null) return;
		// The press leaves the keys and the selection where they are; only a click restores.
		press.preventDefault();
		const { pointerId } = press;
		const originX = press.clientX;
		const originY = press.clientY;
		const origin = latest.current.box;
		let releaseCursor: (() => void) | null = null;
		movedReference.current = false;
		const boxAt = (event: PointerEvent): FileOverlayBox =>
			moveFileOverlayPip(
				latest.current.regions,
				origin,
				event.clientX - originX,
				event.clientY - originY,
			);

		const stop = (): void => {
			view.removeEventListener('pointermove', onPointerMove, true);
			view.removeEventListener('pointerup', onPointerUp, true);
			view.removeEventListener('pointercancel', onPointerEnd, true);
			view.removeEventListener('pointerdown', onPointerEnd, true);
			view.removeEventListener('keydown', onKey, true);
			view.removeEventListener('keypress', onKey, true);
			view.removeEventListener('keyup', onKey, true);
			view.removeEventListener('blur', stop);
			holder.removeEventListener('lostpointercapture', stop);
			if (holder.hasPointerCapture(pointerId)) holder.releasePointerCapture(pointerId);
			releaseCursor?.();
			frame.style.removeProperty(LEFT_PROPERTY);
			frame.style.removeProperty(TOP_PROPERTY);
			stopDrag.current = null;
			// The click a drag ends in arrives with its release; a later one restores again.
			if (movedReference.current) {
				view.setTimeout(() => {
					movedReference.current = false;
				});
			}
		};
		const onPointerMove = (event: PointerEvent): void => {
			if (event.pointerId !== pointerId) return;
			// Without the primary button the press is over, though its release went unseen.
			if ((event.buttons & PRIMARY_BUTTON) === 0) {
				stop();
				return;
			}
			if (releaseCursor === null) {
				if (Math.hypot(event.clientX - originX, event.clientY - originY) <= DRAG_THRESHOLD) {
					return;
				}
				movedReference.current = true;
				releaseCursor = holdDragCursor(holder.ownerDocument);
				try {
					holder.setPointerCapture(pointerId);
				} catch {
					// A pointer the browser no longer tracks cannot be held; the window's listeners follow it.
				}
			}
			event.preventDefault();
			const next = boxAt(event);
			frame.style.setProperty(LEFT_PROPERTY, `${next.left}px`);
			frame.style.setProperty(TOP_PROPERTY, `${next.top}px`);
		};
		const onPointerUp = (event: PointerEvent): void => {
			if (event.pointerId !== pointerId) return;
			const dragged = releaseCursor !== null;
			const landed = boxAt(event);
			stop();
			if (dragged) keep(fileOverlayPipPosition(latest.current.regions, landed));
		};
		/** The browser took the pointer, or the pointer pressed again after an unseen release. */
		const onPointerEnd = (event: PointerEvent): void => {
			if (event.pointerId === pointerId) stop();
		};
		const onKey = (event: KeyboardEvent): void => {
			if (releaseCursor === null) return;
			event.preventDefault();
			event.stopPropagation();
			if (event.type === 'keydown' && event.key === 'Escape') stop();
		};

		view.addEventListener('pointermove', onPointerMove, true);
		view.addEventListener('pointerup', onPointerUp, true);
		view.addEventListener('pointercancel', onPointerEnd, true);
		view.addEventListener('pointerdown', onPointerEnd, true);
		view.addEventListener('keydown', onKey, true);
		view.addEventListener('keypress', onKey, true);
		view.addEventListener('keyup', onKey, true);
		view.addEventListener('blur', stop);
		holder.addEventListener('lostpointercapture', stop);
		stopDrag.current = stop;
	});
	return { moved: () => movedReference.current, onPointerDown };
}
