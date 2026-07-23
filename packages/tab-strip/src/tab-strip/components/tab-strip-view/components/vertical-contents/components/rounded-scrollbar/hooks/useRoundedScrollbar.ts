import { useCallback, useEffect, useRef, useState, type PointerEvent, type RefObject } from 'react';
import {
	calculateRoundedScrollbarThumb,
	ROUNDED_SCROLLBAR_FADE_DELAY_MS,
	type RoundedScrollbarThumbGeometry,
} from '../utils/rounded-scrollbar';

interface DragStart {
	pointerId: number;
	clientY: number;
	scrollTop: number;
}

interface UseRoundedScrollbarOptions {
	viewportRef: RefObject<HTMLElement | null>;
	isAnimatingSize: boolean;
}

interface RoundedScrollbarView {
	geometry: RoundedScrollbarThumbGeometry;
	shown: boolean;
	scrollTop: number;
	show: () => void;
	startHideCountdown: () => void;
	onPointerDown: (event: PointerEvent<HTMLDivElement>) => void;
	onPointerMove: (event: PointerEvent<HTMLDivElement>) => void;
	finishDrag: () => void;
}

export function useRoundedScrollbar({
	viewportRef,
	isAnimatingSize,
}: UseRoundedScrollbarOptions): RoundedScrollbarView {
	const [geometry, setGeometry] = useState<RoundedScrollbarThumbGeometry>({
		visible: false,
		top: 0,
		height: 0,
		maximumScroll: 0,
	});
	const [shown, setShown] = useState(true);
	const hideTimer = useRef<number | null>(null);
	const drag = useRef<DragStart | null>(null);

	const clearHideTimer = useCallback(() => {
		if (hideTimer.current !== null) window.clearTimeout(hideTimer.current);
		hideTimer.current = null;
	}, []);
	const show = useCallback(() => {
		clearHideTimer();
		if (!isAnimatingSize) setShown(true);
	}, [clearHideTimer, isAnimatingSize]);
	const startHideCountdown = useCallback(() => {
		clearHideTimer();
		hideTimer.current = window.setTimeout(() => setShown(false), ROUNDED_SCROLLBAR_FADE_DELAY_MS);
	}, [clearHideTimer]);

	useEffect(() => {
		const viewport = viewportRef.current;
		if (!viewport) return;
		const update = () => {
			setGeometry(
				calculateRoundedScrollbarThumb({
					viewportHeight: viewport.clientHeight,
					contentHeight: viewport.scrollHeight,
					scrollTop: viewport.scrollTop,
				}),
			);
		};
		const onScroll = () => {
			update();
			show();
			startHideCountdown();
		};
		update();
		startHideCountdown();
		viewport.addEventListener('scroll', onScroll, { passive: true });
		const observer = new ResizeObserver(update);
		observer.observe(viewport);
		if (viewport.firstElementChild) observer.observe(viewport.firstElementChild);
		return () => {
			viewport.removeEventListener('scroll', onScroll);
			observer.disconnect();
			clearHideTimer();
		};
	}, [clearHideTimer, show, startHideCountdown, viewportRef]);

	useEffect(() => {
		if (isAnimatingSize) {
			clearHideTimer();
			setShown(false);
		}
	}, [clearHideTimer, isAnimatingSize]);

	return {
		geometry,
		shown,
		scrollTop: viewportRef.current?.scrollTop ?? 0,
		show,
		startHideCountdown,
		onPointerDown: (event) => {
			const viewport = viewportRef.current;
			if (!viewport) return;
			event.preventDefault();
			event.currentTarget.setPointerCapture?.(event.pointerId);
			drag.current = {
				pointerId: event.pointerId,
				clientY: event.clientY,
				scrollTop: viewport.scrollTop,
			};
			show();
		},
		onPointerMove: (event) => {
			const viewport = viewportRef.current;
			const current = drag.current;
			if (!viewport || !current || current.pointerId !== event.pointerId) return;
			const travel = Math.max(1, viewport.clientHeight - geometry.height);
			viewport.scrollTop =
				current.scrollTop + ((event.clientY - current.clientY) / travel) * geometry.maximumScroll;
		},
		finishDrag: () => {
			drag.current = null;
			startHideCountdown();
		},
	};
}
