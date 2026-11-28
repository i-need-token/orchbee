import { holdDragCursor } from '@orchbee/ui/components';

import { squeezeFileTabs, type FileTabSlot } from './squeezeFileTabs';

/** The Tab Strip's threshold: a press turns into a drag once the pointer travels farther. */
const DRAG_THRESHOLD = 10;

/** The bit of `PointerEvent.buttons` for the primary button. */
const PRIMARY_BUTTON = 1;

/** How far a tab stands from its slot while a drag reorders the row. */
const OFFSET_PROPERTY = '--ob-comp-file-tab-reorder-offset';

/** One press on a tab, from the press until the row shows where its drag put the tab. */
export interface FileTabGesture {
	/** Ends the gesture at once and puts every tab in its slot, as when the row goes. */
	readonly dispose: () => void;
	/** The row shows other tabs or another order: a drag lets go of them, and a released one lands. */
	readonly rowChanged: () => void;
	/** Whether the drag is over and its tab only lands, so that a new press may end it at once. */
	readonly settling: () => boolean;
}

interface FileTabPressOptions {
	/** The tab list, whose children are the row's tabs in its order. */
	readonly list: HTMLElement;
	/** Called once the gesture is over and every tab stands in its slot. */
	readonly onEnd: () => void;
	/** Asks to move the tab at `from` to `to`, which the release of a drag does. */
	readonly onRelease: (from: number, to: number) => void;
	readonly press: PointerEvent;
	/** The tab pressed, one of the list's children. */
	readonly pressed: HTMLElement;
	/** The row, which scrolls the list sideways. */
	readonly row: HTMLElement;
}

/** Keeps an event from its target and from the browser's default action. */
function swallowEvent(event: Event): void {
	event.preventDefault();
	event.stopPropagation();
}

/**
 * Follows a press of the primary button on one of the row's tabs, and reorders the row once the
 * pointer travels past the Tab Strip's threshold, with the squeeze motion of the direct reorder
 * vocabulary. The held tab follows the pointer within the row and the others slide out of its way;
 * the row scrolls while the pointer holds one of its ends, and the `grabbing` cursor shows
 * everywhere. Every key belongs to the drag, and Escape puts the tabs back, as the window losing
 * focus or the browser taking the pointer does. A release asks for the move, and the held tab lands
 * in its new place once the row shows it.
 */
export function followFileTabPress({
	list,
	onEnd,
	onRelease,
	press,
	pressed,
	row,
}: FileTabPressOptions): FileTabGesture | null {
	const ownerDocument = list.ownerDocument;
	const ownerView = ownerDocument.defaultView;
	if (ownerView === null) return null;
	// Typed apart from the check, so that the handlers declared below see it as present.
	const view: Window = ownerView;
	const { pointerId } = press;
	const originX = press.clientX;
	const originY = press.clientY;
	let phase: 'armed' | 'dragging' | 'ended' | 'landing' | 'released' = 'armed';
	let clientX = originX;
	// Where the press holds the tab, which stays under the pointer while the drag lasts.
	let grab = 0;
	let tabs: readonly HTMLElement[] = [];
	let slots: readonly FileTabSlot[] = [];
	let held = -1;
	let gap = 0;
	let index = -1;
	// Where the held tab shows in the window, and where the list stood as it did.
	let heldLeft = 0;
	let shownListLeft = 0;
	let frame: number | null = null;
	let captured = false;
	let releaseCursor: (() => void) | null = null;

	const setOffset = (tab: HTMLElement, offset: number): void => {
		tab.style.setProperty(OFFSET_PROPERTY, `${offset}px`);
	};
	const cancelFrame = (): void => {
		if (frame !== null) view.cancelAnimationFrame(frame);
		frame = null;
	};
	/** Moves every tab to where the pointer puts the held one. */
	const show = (): void => {
		shownListLeft = list.getBoundingClientRect().left;
		const squeeze = squeezeFileTabs(slots, held, clientX - grab - shownListLeft, gap);
		index = squeeze.index;
		heldLeft = shownListLeft + (slots[held]?.start ?? 0) + (squeeze.offsets[held] ?? 0);
		for (const [place, tab] of tabs.entries()) setOffset(tab, squeeze.offsets[place] ?? 0);
	};
	/** Scrolls the row while the pointer holds one of its ends, the faster the farther in it goes. */
	const tick = (): void => {
		frame = view.requestAnimationFrame(tick);
		const bounds = row.getBoundingClientRect();
		const edge = Math.min(bounds.height, bounds.width / 4);
		const before = Math.min(edge, bounds.left + edge - clientX);
		const after = Math.min(edge, clientX - (bounds.right - edge));
		if (before > 0) row.scrollLeft -= Math.ceil(before / 4);
		else if (after > 0) row.scrollLeft += Math.ceil(after / 4);
		// The row scrolled, by this or by the wheel: the held tab stays under the pointer.
		if (list.getBoundingClientRect().left !== shownListLeft) show();
	};
	/** Lets go of the pointer, the keys and the cursor. */
	const stopFollowing = (): void => {
		view.removeEventListener('pointermove', onPointerMove, true);
		view.removeEventListener('pointerup', onPointerUp, true);
		view.removeEventListener('pointercancel', onPointerCancel, true);
		view.removeEventListener('pointerdown', onPointerDown, true);
		view.removeEventListener('keydown', onKey, true);
		view.removeEventListener('keypress', onKey, true);
		view.removeEventListener('keyup', onKey, true);
		view.removeEventListener('blur', cancel);
		list.removeEventListener('lostpointercapture', cancel);
		cancelFrame();
		if (captured && list.hasPointerCapture(pointerId)) list.releasePointerCapture(pointerId);
		captured = false;
		releaseCursor?.();
		releaseCursor = null;
	};
	/** Ends the gesture with every tab in its slot. */
	const finish = (): void => {
		if (phase === 'ended') return;
		phase = 'ended';
		cancelFrame();
		for (const tab of tabs) {
			tab.removeAttribute('data-reorder');
			tab.style.removeProperty(OFFSET_PROPERTY);
		}
		onEnd();
	};
	/** Slides the held tab from where it shows into its slot, then ends the gesture. */
	const land = (): void => {
		phase = 'landing';
		const heldTab = tabs[held];
		if (heldTab === undefined) {
			finish();
			return;
		}
		heldTab.setAttribute('data-reorder', 'landing');
		setOffset(heldTab, 0);
		const motion = heldTab.getAnimations().map((animation) => animation.finished);
		void Promise.allSettled(motion).then(() => {
			if (phase === 'landing') finish();
		});
	};
	/** Swallows the click, or double click, that the release of the pointer is about to cause. */
	const swallowClick = (): void => {
		view.addEventListener('click', swallowEvent, true);
		view.addEventListener('dblclick', swallowEvent, true);
		view.setTimeout(() => {
			view.removeEventListener('click', swallowEvent, true);
			view.removeEventListener('dblclick', swallowEvent, true);
		}, 0);
	};
	/** Swallows the click of a cancelled drag's release, unless the pointer presses again first. */
	const swallowReleaseClick = (): void => {
		const settle = (event: PointerEvent): void => {
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
	/** Ends the press without a move: a drag puts its tabs back, and a press stays a click. */
	function cancel(): void {
		if (phase === 'armed') {
			stopFollowing();
			finish();
			return;
		}
		if (phase !== 'dragging') return;
		stopFollowing();
		for (const tab of tabs) setOffset(tab, 0);
		land();
		swallowReleaseClick();
	}
	/** Starts the drag, unless the row has nothing to reorder. */
	const latch = (): boolean => {
		tabs = [...list.querySelectorAll<HTMLElement>(':scope > [role="tab"]')];
		held = tabs.indexOf(pressed);
		if (held < 0 || tabs.length < 2) return false;
		// Measured now, as the row may have scrolled to show the tab once the press made it active.
		grab = originX - pressed.getBoundingClientRect().left;
		const listLeft = list.getBoundingClientRect().left;
		slots = tabs.map((tab) => {
			const bounds = tab.getBoundingClientRect();
			return { size: bounds.width, start: bounds.left - listLeft };
		});
		const columnGap = Number.parseFloat(view.getComputedStyle(list).columnGap);
		gap = Number.isFinite(columnGap) ? columnGap : 0;
		for (const tab of tabs) {
			tab.setAttribute('data-reorder', tab === pressed ? 'held' : 'shifted');
			setOffset(tab, 0);
		}
		try {
			list.setPointerCapture(pointerId);
			captured = true;
		} catch {
			// The pointer is no longer active; the window still hears its release.
		}
		releaseCursor = holdDragCursor(ownerDocument);
		view.addEventListener('keydown', onKey, true);
		view.addEventListener('keypress', onKey, true);
		view.addEventListener('keyup', onKey, true);
		view.addEventListener('blur', cancel);
		list.addEventListener('lostpointercapture', cancel);
		frame = view.requestAnimationFrame(tick);
		phase = 'dragging';
		return true;
	};

	function onPointerMove(event: PointerEvent): void {
		if (event.pointerId !== pointerId) return;
		// Without the primary button the press is over, though its release went unseen.
		if ((event.buttons & PRIMARY_BUTTON) === 0) {
			cancel();
			return;
		}
		clientX = event.clientX;
		if (phase === 'armed') {
			if (Math.hypot(clientX - originX, event.clientY - originY) <= DRAG_THRESHOLD) return;
			if (!latch()) {
				cancel();
				return;
			}
		}
		event.preventDefault();
		show();
	}
	function onPointerUp(event: PointerEvent): void {
		if (event.pointerId !== pointerId) return;
		if (phase !== 'dragging') {
			cancel();
			return;
		}
		clientX = event.clientX;
		show();
		stopFollowing();
		swallowClick();
		if (index === held) {
			land();
			return;
		}
		phase = 'released';
		onRelease(held, index);
		// The row shows the move before its next frame; when it does not, the move was refused.
		frame = view.requestAnimationFrame(() => {
			frame = null;
			if (phase !== 'released') return;
			for (const tab of tabs) setOffset(tab, 0);
			land();
		});
	}
	/** The browser took the pointer, as for a touch that scrolls: no release follows. */
	function onPointerCancel(event: PointerEvent): void {
		if (event.pointerId === pointerId) cancel();
	}
	/** The pointer pressed again, so its release went unseen: this press is over. */
	function onPointerDown(event: PointerEvent): void {
		if (event.pointerId === pointerId) cancel();
	}
	// While the drag holds the pointer, every key is its own, as the Sidebar's drag takes them.
	function onKey(event: KeyboardEvent): void {
		swallowEvent(event);
		const plain = !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
		if (event.type === 'keydown' && event.key === 'Escape' && plain) cancel();
	}

	view.addEventListener('pointermove', onPointerMove, true);
	view.addEventListener('pointerup', onPointerUp, true);
	view.addEventListener('pointercancel', onPointerCancel, true);
	view.addEventListener('pointerdown', onPointerDown, true);

	return {
		dispose: () => {
			stopFollowing();
			finish();
		},
		rowChanged: () => {
			// A press that has not become a drag reads the row only once it does.
			if (phase === 'dragging') {
				// The tabs it measured changed: they go back at once, and the press ends quietly.
				stopFollowing();
				finish();
				swallowReleaseClick();
				return;
			}
			if (phase !== 'released') return;
			// The row shows the move: the other tabs already stand in their new slots, and the held tab
			// lands from where it was let go, so nothing replays the squeeze against the new order.
			cancelFrame();
			const heldTab = tabs[held];
			for (const tab of tabs) {
				if (tab === heldTab) continue;
				tab.removeAttribute('data-reorder');
				tab.style.removeProperty(OFFSET_PROPERTY);
			}
			if (heldTab?.isConnected !== true) {
				finish();
				return;
			}
			setOffset(heldTab, 0);
			setOffset(heldTab, heldLeft - heldTab.getBoundingClientRect().left);
			// A frame shows the tab where it was let go before it lands, so that its landing starts
			// there: the first callback runs before that frame's styles, the second after them.
			frame = view.requestAnimationFrame(() => {
				frame = view.requestAnimationFrame(() => {
					frame = null;
					land();
				});
			});
		},
		settling: () => phase === 'released' || phase === 'landing',
	};
}
