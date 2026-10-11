import { useMemoizedFn } from '@orchbee/foundation/react';
import { useEffect, useRef, useState, type RefObject } from 'react';
import type { TabModelSnapshot } from '../../../../../model/tab-model';
import {
	TabDragScrollHandler,
	createDomTabDragScrollable,
} from '../../../../../dragging/tab-drag-scroll-handler';
import { useElementSize } from '../../../../../hooks/useElementSize';
import {
	TAB_BOUNDS_ANIMATION_DURATION_MS,
	type DragState,
} from '../../../../../utils/tab-strip-internals';
import { calculateVerticalDragCompositeBounds } from '../utils/calculateVerticalDragCompositeBounds';

interface VerticalContentsState {
	isAnimatingSize: boolean;
	pinnedScrollRef: RefObject<HTMLDivElement | null>;
	regionsRef: RefObject<HTMLDivElement | null>;
	regionsSize: Readonly<{ width: number; height: number }>;
	scrollRef: RefObject<HTMLDivElement | null>;
}

export function useVerticalContents({
	availableWidth,
	collapsed,
	drag,
	onDragScrolled,
	snapshot,
}: {
	availableWidth: number;
	collapsed: boolean;
	drag: DragState | null;
	onDragScrolled: () => void;
	snapshot: TabModelSnapshot;
}): Readonly<VerticalContentsState> {
	const scrollRef = useRef<HTMLDivElement>(null);
	const pinnedScrollRef = useRef<HTMLDivElement>(null);
	const [regionsRef, regionsSize] = useElementSize<HTMLDivElement>();
	const previousAvailableWidth = useRef(availableWidth);
	const sizeAnimationTimer = useRef<number | null>(null);
	const [isAnimatingSize, setIsAnimatingSize] = useState(false);
	const notifyDragScrolled = useMemoizedFn(onDragScrolled);
	const dragScrollHandler = useRef<TabDragScrollHandler | null>(null);
	if (dragScrollHandler.current === null) {
		dragScrollHandler.current = new TabDragScrollHandler(notifyDragScrolled);
	}
	useEffect(() => () => dragScrollHandler.current?.destroy(), []);
	useEffect(() => {
		if (previousAvailableWidth.current === availableWidth) return;
		previousAvailableWidth.current = availableWidth;
		setIsAnimatingSize(true);
		if (sizeAnimationTimer.current !== null) window.clearTimeout(sizeAnimationTimer.current);
		sizeAnimationTimer.current = window.setTimeout(() => {
			sizeAnimationTimer.current = null;
			setIsAnimatingSize(false);
		}, TAB_BOUNDS_ANIMATION_DURATION_MS);
	}, [availableWidth]);
	useEffect(
		() => () => {
			if (sizeAnimationTimer.current !== null) window.clearTimeout(sizeAnimationTimer.current);
		},
		[],
	);
	useEffect(() => {
		const handler = dragScrollHandler.current;
		const sourceTab = drag?.tabIds.length
			? snapshot.tabs.find((tab) => tab.id === drag.tabIds[0])
			: null;
		const element = sourceTab?.pinned ? pinnedScrollRef.current : scrollRef.current;
		if (!sourceTab || !element || !handler || !drag?.started) {
			handler?.stopScrolling();
			return;
		}
		const rect = element.getBoundingClientRect();
		handler.onDraggedTabPositionUpdated(
			createDomTabDragScrollable(element),
			calculateVerticalDragCompositeBounds({
				collapsed,
				drag,
				sourcePinned: sourceTab.pinned,
				viewportBounds: {
					x: rect.left,
					y: rect.top,
					width: rect.width,
					height: rect.height,
				},
			}),
		);
	}, [collapsed, drag, snapshot.revision, snapshot.tabs]);

	return { isAnimatingSize, pinnedScrollRef, regionsRef, regionsSize, scrollRef } as const;
}
