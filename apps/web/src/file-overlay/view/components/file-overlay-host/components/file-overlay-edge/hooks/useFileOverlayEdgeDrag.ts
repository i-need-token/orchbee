import { useMemoizedFn } from '@orchbee/foundation/react';
import {
	useEffect,
	useLayoutEffect,
	useRef,
	useState,
	type PointerEvent as ReactPointerEvent,
} from 'react';

import {
	fileOverlayEdgeTier,
	type FileOverlayNarrowTier,
	type FileOverlayRegions,
} from '../../../../../../model';

/** The bit of `PointerEvent.buttons` for the primary button. */
const PRIMARY_BUTTON = 1;

interface FileOverlayEdgeDragOptions {
	/** Takes the tier a release chose, which may be the tier the overlay has. */
	readonly choose: (tier: FileOverlayNarrowTier) => void;
	/** What the overlay lies over, which places the Sidebar's midline. */
	readonly regions: FileOverlayRegions;
	/** The overlay's tier, which a press previews until the pointer crosses the midline. */
	readonly tier: FileOverlayNarrowTier;
}

interface FileOverlayEdgeDrag {
	readonly onPointerDown: (event: ReactPointerEvent<HTMLElement>) => void;
	/** The tier a release would choose while the edge is held; null while it is not. */
	readonly preview: FileOverlayNarrowTier | null;
}

/**
 * Follows a press of the primary button on the frame's left edge. The edge keeps the pointer while
 * the press lasts and previews the tier on the pointer's side of the Sidebar's midline; the release
 * chooses it. Every key belongs to the drag, and Esc ends it choosing nothing, as the window losing
 * focus or the browser taking the pointer does.
 */
export function useFileOverlayEdgeDrag({
	choose,
	regions,
	tier,
}: FileOverlayEdgeDragOptions): FileOverlayEdgeDrag {
	const [preview, setPreview] = useState<FileOverlayNarrowTier | null>(null);
	// A drag reads the regions as they are when the pointer moves, as a resize may change them.
	const latestRegions = useRef(regions);
	useLayoutEffect(() => {
		latestRegions.current = regions;
	});
	const stopDrag = useRef<(() => void) | null>(null);
	useEffect(() => () => stopDrag.current?.(), []);
	const chooseTier = useMemoizedFn(choose);

	const onPointerDown = useMemoizedFn((press: ReactPointerEvent<HTMLElement>) => {
		if (press.button !== 0 || !press.isPrimary || stopDrag.current !== null) return;
		const edge = press.currentTarget;
		const origin = edge.offsetParent;
		const view = edge.ownerDocument.defaultView;
		if (origin === null || view === null) return;
		// The press leaves the keys and the selection where they are.
		press.preventDefault();
		const { pointerId } = press;
		let shown = tier;
		const tierAt = (clientX: number): FileOverlayNarrowTier =>
			fileOverlayEdgeTier(latestRegions.current, clientX - origin.getBoundingClientRect().left);

		const stop = (): void => {
			view.removeEventListener('pointermove', onPointerMove, true);
			view.removeEventListener('pointerup', onPointerUp, true);
			view.removeEventListener('pointercancel', onPointerEnd, true);
			view.removeEventListener('pointerdown', onPointerEnd, true);
			view.removeEventListener('keydown', onKey, true);
			view.removeEventListener('keypress', onKey, true);
			view.removeEventListener('keyup', onKey, true);
			view.removeEventListener('blur', stop);
			edge.removeEventListener('lostpointercapture', stop);
			if (edge.hasPointerCapture(pointerId)) edge.releasePointerCapture(pointerId);
			stopDrag.current = null;
			setPreview(null);
		};
		const onPointerMove = (event: PointerEvent): void => {
			if (event.pointerId !== pointerId) return;
			// Without the primary button the press is over, though its release went unseen.
			if ((event.buttons & PRIMARY_BUTTON) === 0) {
				stop();
				return;
			}
			event.preventDefault();
			const next = tierAt(event.clientX);
			if (next === shown) return;
			shown = next;
			setPreview(next);
		};
		const onPointerUp = (event: PointerEvent): void => {
			if (event.pointerId !== pointerId) return;
			const chosen = tierAt(event.clientX);
			stop();
			chooseTier(chosen);
		};
		/** The browser took the pointer, or the pointer pressed again after an unseen release. */
		const onPointerEnd = (event: PointerEvent): void => {
			if (event.pointerId === pointerId) stop();
		};
		const onKey = (event: KeyboardEvent): void => {
			event.preventDefault();
			event.stopPropagation();
			if (event.type === 'keydown' && event.key === 'Escape') stop();
		};

		try {
			edge.setPointerCapture(pointerId);
		} catch {
			// A pointer the browser no longer tracks cannot be held; the window's listeners follow it.
		}
		view.addEventListener('pointermove', onPointerMove, true);
		view.addEventListener('pointerup', onPointerUp, true);
		view.addEventListener('pointercancel', onPointerEnd, true);
		view.addEventListener('pointerdown', onPointerEnd, true);
		view.addEventListener('keydown', onKey, true);
		view.addEventListener('keypress', onKey, true);
		view.addEventListener('keyup', onKey, true);
		view.addEventListener('blur', stop);
		edge.addEventListener('lostpointercapture', stop);
		stopDrag.current = stop;
		setPreview(tier);
	});
	return { onPointerDown, preview };
}
