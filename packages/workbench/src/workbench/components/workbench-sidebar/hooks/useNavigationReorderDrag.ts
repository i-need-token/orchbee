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
	readonly displacementDirection: 'down' | 'up' | null;
	readonly draggingItemId: string | null;
	readonly onItemPointerDown: (itemId: string, event: React.PointerEvent<HTMLDivElement>) => void;
	readonly listRef: React.RefObject<HTMLUListElement | null>;
}

export function useNavigationReorderDrag(
	items: readonly NavigationReorderItem[],
	reorderItem: (itemId: string, beforeItemId: string | null) => void,
): NavigationReorderDragView {
	const listRef = React.useRef<HTMLUListElement>(null);
	const itemsRef = React.useRef(items);
	const reorderItemRef = React.useRef(reorderItem);
	const finishRef = React.useRef<
		((commit: boolean, synchronous?: boolean | undefined) => void) | null
	>(null);
	const [dragState, setDragState] = React.useState<NavigationReorderDragState>(
		createNavigationReorderDragState,
	);
	const [session, setSession] = React.useState<NavigationReorderDragSession | null>(null);
	itemsRef.current = items;
	reorderItemRef.current = reorderItem;

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
		let finished = false;
		let latestClientX = session.clientX;
		let latestClientY = session.clientY;
		let refreshAfterActivation = false;
		const initialDocumentCursor = ownerDocument.documentElement.style.cursor;
		const clearDragVisual = () => {
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
			clearDragVisual();
			const clearState = () => {
				setDragState(transition.state);
				setSession(null);
			};
			if (synchronous) flushSync(clearState);
			else clearState();
			if (wasDragging) suppressTrailingClick();
			const command = transition.command;
			if (command !== null) {
				const currentItems = itemsRef.current;
				const sourceExists = currentItems.some((item) => item.id === command.itemId);
				const targetExists =
					command.beforeItemId === null ||
					currentItems.some((item) => item.id === command.beforeItemId);
				if (sourceExists && targetExists) {
					reorderItemRef.current(command.itemId, command.beforeItemId);
				}
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
				const edgeSize = Math.min(48, bounds.height / 4);
				const maxScrollTop = Math.max(0, scrollElement.scrollHeight - scrollElement.clientHeight);
				let scrollStep = 0;
				if (edgeSize > 0 && latestClientY < bounds.top + edgeSize) {
					const pressure = Math.min(1, (bounds.top + edgeSize - latestClientY) / edgeSize);
					scrollStep = -Math.max(2, Math.ceil(12 * pressure));
				} else if (edgeSize > 0 && latestClientY > bounds.bottom - edgeSize) {
					const pressure = Math.min(1, (latestClientY - (bounds.bottom - edgeSize)) / edgeSize);
					scrollStep = Math.max(2, Math.ceil(12 * pressure));
				}
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
			if (event.key !== 'Escape') return;
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
		if (dragState.kind !== 'dragging') {
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
		displacementDirection: displacement.direction,
		draggingItemId: dragState.kind === 'dragging' ? dragState.itemId : null,
		onItemPointerDown,
		listRef,
	};
}
