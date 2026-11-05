import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';
import { flushSync } from 'react-dom';

import {
	createNavigationReorderDragState,
	transitionNavigationReorderDrag,
	type NavigationReorderDragState,
} from '../model/navigation-reorder-drag-state';
import { getNavigationReorderBeforeId } from '../utils/getNavigationReorderBeforeId';

interface NavigationReorderDragSession {
	readonly captureElement: HTMLDivElement;
	readonly clientX: number;
	readonly clientY: number;
	readonly draggedItemMidpointOffsetY: number;
	readonly pointerId: number;
	readonly itemId: string;
	readonly scrollElement: HTMLElement | null;
}

interface NavigationReorderItem {
	readonly id: string;
}

interface NavigationReorderDragView {
	readonly displacedItemIds: readonly string[];
	/** Off once released: a row that animated while the list changed under it would slide from a place it never was. */
	readonly displacementAnimated: boolean;
	readonly displacementDirection: 'down' | 'up' | null;
	readonly draggingItemId: string | null;
	readonly onItemPointerDown: (itemId: string, event: React.PointerEvent<HTMLDivElement>) => void;
	readonly listRef: React.RefObject<HTMLUListElement | null>;
}

/**
 * The slot a dragged row leaves, and how far crossed siblings squeeze into it: the row's own height,
 * since rows differ, plus the spacing to a neighbour. A Project folds while it is dragged, so its
 * row, not its whole item, is what leaves.
 */
function measureReorderSlot(row: HTMLElement): {
	readonly blockSize: number;
	readonly shift: number;
} {
	const blockSize = row.getBoundingClientRect().height;
	const item = row.closest('[data-navigation-reorder-id]');
	if (item === null) return { blockSize, shift: blockSize };
	const bounds = item.getBoundingClientRect();
	const next = item.nextElementSibling;
	const previous = item.previousElementSibling;
	let spacing = 0;
	if (next !== null) spacing = next.getBoundingClientRect().top - bounds.bottom;
	else if (previous !== null) spacing = bounds.top - previous.getBoundingClientRect().bottom;
	return { blockSize, shift: blockSize + Math.max(0, spacing) };
}

/** Holding the list's top or bottom edge scrolls it by about one row this often. */
const EDGE_SCROLL_INTERVAL_MS = 120;

/** A released row holds the place it was dropped until `reorderItem` settles, so the list never shows the old order in between. */
export function useNavigationReorderDrag(
	items: readonly NavigationReorderItem[],
	reorderItem: (itemId: string, beforeItemId: string | null) => Promise<unknown>,
	onDragStarted?: ((itemId: string) => void) | undefined,
): NavigationReorderDragView {
	const listRef = React.useRef<HTMLUListElement>(null);
	const itemsRef = React.useRef(items);
	const reorderItemRef = React.useRef(reorderItem);
	const onDragStartedRef = React.useRef(onDragStarted);
	const finishRef = React.useRef<
		((commit: boolean, synchronous?: boolean | undefined) => void) | null
	>(null);
	const [dragState, setDragState] = React.useState<NavigationReorderDragState>(
		createNavigationReorderDragState,
	);
	const [session, setSession] = React.useState<NavigationReorderDragSession | null>(null);
	itemsRef.current = items;
	reorderItemRef.current = reorderItem;
	onDragStartedRef.current = onDragStarted;

	const onItemPointerDown = useMemoizedFn(
		(itemId: string, event: React.PointerEvent<HTMLDivElement>) => {
			const target = event.target;
			if (
				session !== null ||
				items.length < 2 ||
				event.button !== 0 ||
				(event.pointerType !== 'mouse' && event.pointerType !== 'pen') ||
				(target instanceof Element && target.closest('[data-navigation-reorder-excluded]') !== null)
			) {
				return;
			}
			const transition = transitionNavigationReorderDrag(createNavigationReorderDragState(), {
				clientX: event.clientX,
				clientY: event.clientY,
				pointerId: event.pointerId,
				itemId,
				type: 'pointer-armed',
			});
			const scrollElement = event.currentTarget.closest<HTMLElement>(
				'[data-navigation-reorder-scroll]',
			);
			const sourceBounds = event.currentTarget.getBoundingClientRect();
			setDragState(transition.state);
			setSession({
				captureElement: event.currentTarget,
				clientX: event.clientX,
				clientY: event.clientY,
				draggedItemMidpointOffsetY: sourceBounds.top + sourceBounds.height / 2 - event.clientY,
				pointerId: event.pointerId,
				itemId,
				scrollElement,
			});
		},
	);

	React.useEffect(() => {
		if (session === null) return;
		const ownerDocument = session.captureElement.ownerDocument;
		let semanticState: NavigationReorderDragState = {
			clientX: session.clientX,
			clientY: session.clientY,
			kind: 'armed',
			pointerId: session.pointerId,
			itemId: session.itemId,
		};
		let animationFrame: number | null = null;
		let lastEdgeScroll = Number.NEGATIVE_INFINITY;
		let finished = false;
		let latestClientX = session.clientX;
		let latestClientY = session.clientY;
		let refreshAfterActivation = false;
		/** The list holding the measured slot, cleared with the rest of the drag visual. */
		let slotList: HTMLUListElement | null = null;
		const initialDocumentCursor = ownerDocument.documentElement.style.cursor;
		const clearDragVisual = () => {
			slotList?.style.removeProperty('--ob-comp-workbench-navigation-reorder-block-size');
			slotList?.style.removeProperty('--ob-comp-workbench-navigation-reorder-shift');
			slotList = null;
			session.captureElement.style.removeProperty(
				'--ob-comp-workbench-navigation-reorder-block-start',
			);
			session.captureElement.style.removeProperty(
				'--ob-comp-workbench-navigation-reorder-inline-size',
			);
			session.captureElement.style.removeProperty(
				'--ob-comp-workbench-navigation-reorder-inline-start',
			);
			session.captureElement.style.removeProperty(
				'--ob-comp-workbench-navigation-reorder-translate-y',
			);
			ownerDocument.documentElement.style.cursor = initialDocumentCursor;
		};
		const releaseCapture = () => {
			if (
				typeof session.captureElement.hasPointerCapture === 'function' &&
				typeof session.captureElement.releasePointerCapture === 'function' &&
				session.captureElement.hasPointerCapture(session.pointerId)
			) {
				try {
					session.captureElement.releasePointerCapture(session.pointerId);
				} catch {
					// Pointer capture can already be released implicitly by the browser.
				}
			}
		};
		const suppressTrailingClick = () => {
			let consumed = false;
			const suppress = (event: MouseEvent) => {
				consumed = true;
				event.preventDefault();
				event.stopPropagation();
			};
			ownerDocument.addEventListener('click', suppress, { capture: true, once: true });
			ownerDocument.defaultView?.setTimeout(() => {
				if (!consumed) ownerDocument.removeEventListener('click', suppress, true);
			}, 0);
		};
		const finish = (commit: boolean, synchronous = true) => {
			if (finished) return;
			finished = true;
			const wasDragging = semanticState.kind === 'dragging';
			const transition = transitionNavigationReorderDrag(
				semanticState,
				commit
					? { pointerId: session.pointerId, type: 'pointer-released' }
					: { pointerId: session.pointerId, type: 'pointer-cancelled' },
			);
			semanticState = transition.state;
			if (animationFrame !== null) {
				ownerDocument.defaultView?.cancelAnimationFrame(animationFrame);
				animationFrame = null;
			}
			ownerDocument.documentElement.style.cursor = initialDocumentCursor;
			const settle = () => {
				semanticState = transitionNavigationReorderDrag(semanticState, {
					type: 'reorder-settled',
				}).state;
				clearDragVisual();
				const clearState = () => {
					setDragState(semanticState);
					setSession(null);
				};
				if (synchronous) flushSync(clearState);
				else clearState();
			};
			if (wasDragging) suppressTrailingClick();
			const command = transition.command;
			const reorderAdmitted =
				command?.type === 'commit-navigation-reorder' &&
				itemsRef.current.some((item) => item.id === command.itemId) &&
				(command.beforeItemId === null ||
					itemsRef.current.some((item) => item.id === command.beforeItemId));
			if (reorderAdmitted) {
				setDragState(semanticState);
				void reorderItemRef.current(command.itemId, command.beforeItemId).then(settle, settle);
			} else {
				settle();
			}
			releaseCapture();
		};
		finishRef.current = finish;
		const updateFromPointer = (clientX: number, clientY: number) => {
			const listElement = listRef.current;
			if (listElement === null) {
				finish(false);
				return;
			}
			const previousState = semanticState;
			const transition = transitionNavigationReorderDrag(semanticState, {
				beforeItemId: getNavigationReorderBeforeId(
					listElement,
					session.itemId,
					clientY + session.draggedItemMidpointOffsetY,
				),
				clientX,
				clientY,
				pointerId: session.pointerId,
				type: 'pointer-moved',
			});
			semanticState = transition.state;
			if (semanticState !== previousState) setDragState(semanticState);
			if (transition.command?.type === 'navigation-drag-started') {
				onDragStartedRef.current?.(transition.command.itemId);
			}
			if (semanticState.kind !== 'dragging') return;
			if (previousState.kind !== 'dragging') {
				if (typeof session.captureElement.setPointerCapture === 'function') {
					try {
						session.captureElement.setPointerCapture(session.pointerId);
					} catch {
						// Document listeners still own completion when capture is unavailable.
					}
				}
				const sourceBounds = session.captureElement.getBoundingClientRect();
				session.captureElement.style.setProperty(
					'--ob-comp-workbench-navigation-reorder-block-start',
					`${sourceBounds.top}px`,
				);
				session.captureElement.style.setProperty(
					'--ob-comp-workbench-navigation-reorder-inline-size',
					`${sourceBounds.width}px`,
				);
				session.captureElement.style.setProperty(
					'--ob-comp-workbench-navigation-reorder-inline-start',
					`${sourceBounds.left}px`,
				);
				const slot = measureReorderSlot(session.captureElement);
				listElement.style.setProperty(
					'--ob-comp-workbench-navigation-reorder-block-size',
					`${slot.blockSize}px`,
				);
				listElement.style.setProperty(
					'--ob-comp-workbench-navigation-reorder-shift',
					`${slot.shift}px`,
				);
				slotList = listElement;
				ownerDocument.documentElement.style.cursor = 'grabbing';
				refreshAfterActivation = true;
			}
			session.captureElement.style.setProperty(
				'--ob-comp-workbench-navigation-reorder-translate-y',
				`${clientY - session.clientY}px`,
			);
		};
		const runAutoScroll = () => {
			animationFrame = null;
			if (finished || semanticState.kind !== 'dragging') return;
			const scrollElement = session.scrollElement;
			let didScroll = false;
			if (scrollElement !== null) {
				const bounds = scrollElement.getBoundingClientRect();
				// One row, as the source's own height, every interval while the pointer holds an edge.
				const rowSize = session.captureElement.getBoundingClientRect().height;
				const edgeSize = Math.min(rowSize, bounds.height / 4);
				const maxScrollTop = Math.max(0, scrollElement.scrollHeight - scrollElement.clientHeight);
				const now = ownerDocument.defaultView?.performance.now() ?? 0;
				let scrollStep = 0;
				if (edgeSize > 0 && latestClientY < bounds.top + edgeSize) scrollStep = -rowSize;
				else if (edgeSize > 0 && latestClientY > bounds.bottom - edgeSize) scrollStep = rowSize;
				if (scrollStep === 0) lastEdgeScroll = Number.NEGATIVE_INFINITY;
				else if (now - lastEdgeScroll < EDGE_SCROLL_INTERVAL_MS) scrollStep = 0;
				else lastEdgeScroll = now;
				if (scrollStep !== 0) {
					const previousScrollTop = scrollElement.scrollTop;
					scrollElement.scrollTop = Math.max(
						0,
						Math.min(maxScrollTop, previousScrollTop + scrollStep),
					);
					didScroll = scrollElement.scrollTop !== previousScrollTop;
				}
			}
			if (didScroll || refreshAfterActivation) {
				refreshAfterActivation = false;
				updateFromPointer(latestClientX, latestClientY);
			}
			animationFrame = ownerDocument.defaultView?.requestAnimationFrame(runAutoScroll) ?? null;
		};
		const handlePointerMove = (event: PointerEvent) => {
			if (event.pointerId !== session.pointerId) return;
			latestClientX = event.clientX;
			latestClientY = event.clientY;
			updateFromPointer(event.clientX, event.clientY);
			if (semanticState.kind === 'dragging') {
				event.preventDefault();
				if (animationFrame === null) {
					animationFrame = ownerDocument.defaultView?.requestAnimationFrame(runAutoScroll) ?? null;
				}
			}
		};
		const handlePointerUp = (event: PointerEvent) => {
			if (event.pointerId !== session.pointerId) return;
			finish(true);
		};
		const handlePointerCancel = (event: PointerEvent) => {
			if (event.pointerId === session.pointerId) finish(false);
		};
		const handleKeyDown = (event: KeyboardEvent) => {
			if (finished || event.key !== 'Escape') return;
			event.preventDefault();
			finish(false);
		};
		const handleLostPointerCapture = () => finish(false);
		ownerDocument.addEventListener('pointermove', handlePointerMove, { passive: false });
		ownerDocument.addEventListener('pointerup', handlePointerUp);
		ownerDocument.addEventListener('pointercancel', handlePointerCancel);
		ownerDocument.addEventListener('keydown', handleKeyDown);
		session.captureElement.addEventListener('lostpointercapture', handleLostPointerCapture);
		return () => {
			ownerDocument.removeEventListener('pointermove', handlePointerMove);
			ownerDocument.removeEventListener('pointerup', handlePointerUp);
			ownerDocument.removeEventListener('pointercancel', handlePointerCancel);
			ownerDocument.removeEventListener('keydown', handleKeyDown);
			session.captureElement.removeEventListener('lostpointercapture', handleLostPointerCapture);
			if (finishRef.current === finish) finishRef.current = null;
			if (animationFrame !== null) {
				ownerDocument.defaultView?.cancelAnimationFrame(animationFrame);
			}
			if (!finished && semanticState.kind === 'dragging') suppressTrailingClick();
			clearDragVisual();
			releaseCapture();
		};
	}, [session]);

	React.useEffect(() => {
		if (dragState.kind === 'idle') return;
		const sourceExists = items.some((item) => item.id === dragState.itemId);
		const targetExists =
			dragState.kind !== 'dragging' ||
			dragState.beforeItemId === null ||
			items.some((item) => item.id === dragState.beforeItemId);
		if (sourceExists && targetExists) return;
		const finish = finishRef.current;
		if (finish !== null) {
			finish(false, false);
			return;
		}
		setDragState(transitionNavigationReorderDrag(dragState, { type: 'item-invalidated' }).state);
		setSession(null);
	}, [dragState, items]);
	const displacement = React.useMemo(() => {
		if (dragState.kind !== 'dragging' && dragState.kind !== 'settling') {
			return { direction: null, itemIds: [] };
		}
		const sourceIndex = items.findIndex((item) => item.id === dragState.itemId);
		if (sourceIndex < 0) return { direction: null, itemIds: [] };
		if (dragState.beforeItemId === null) {
			return {
				direction: 'up' as const,
				itemIds: items.slice(sourceIndex + 1).map((item) => item.id),
			};
		}
		const targetIndex = items.findIndex((item) => item.id === dragState.beforeItemId);
		if (targetIndex < sourceIndex) {
			return {
				direction: 'down' as const,
				itemIds: items.slice(targetIndex, sourceIndex).map((item) => item.id),
			};
		}
		if (targetIndex > sourceIndex) {
			return {
				direction: 'up' as const,
				itemIds: items.slice(sourceIndex + 1, targetIndex).map((item) => item.id),
			};
		}
		return { direction: null, itemIds: [] };
	}, [dragState, items]);

	return {
		displacedItemIds: displacement.itemIds,
		displacementAnimated: dragState.kind === 'dragging',
		displacementDirection: displacement.direction,
		draggingItemId:
			dragState.kind === 'dragging' || dragState.kind === 'settling' ? dragState.itemId : null,
		onItemPointerDown,
		listRef,
	};
}
