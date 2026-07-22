import {
	type PointerEvent as ReactPointerEvent,
	type ReactNode,
	type RefObject,
	useCallback,
	useEffect,
	useRef,
	useState,
} from 'react';
import {
	calculateRoundedScrollbarThumb,
	ROUNDED_SCROLLBAR_FADE_DELAY_MS,
	type RoundedScrollbarThumbGeometry,
} from '../../../../../shared';
interface DragStart {
	pointerId: number;
	clientY: number;
	scrollTop: number;
}
interface UseRoundedScrollbarOptions {
	viewportRef: RefObject<HTMLElement | null>;
	isAnimatingSize?: boolean;
	shouldHaveRightMargin?: boolean;
}
export function useRoundedScrollbar({
	viewportRef,
	isAnimatingSize = false,
	shouldHaveRightMargin = true,
}: UseRoundedScrollbarOptions): ReactNode {
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
		viewport.addEventListener('scroll', onScroll, {
			passive: true,
		});
		const observer = new ResizeObserver(update);
		observer.observe(viewport);
		if (viewport.firstElementChild) observer.observe(viewport.firstElementChild);
		return () => {
			viewport.removeEventListener('scroll', onScroll);
			observer.disconnect();
			clearHideTimer();
		};
	}, [isAnimatingSize, viewportRef, clearHideTimer, show, startHideCountdown]);
	useEffect(() => {
		if (isAnimatingSize) {
			clearHideTimer();
			setShown(false);
		}
	}, [clearHideTimer, isAnimatingSize]);
	const onPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
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
	};
	const onPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
		const viewport = viewportRef.current;
		const current = drag.current;
		if (!viewport || !current || current.pointerId !== event.pointerId) return;
		const travel = Math.max(1, viewport.clientHeight - geometry.height);
		viewport.scrollTop =
			current.scrollTop + ((event.clientY - current.clientY) / travel) * geometry.maximumScroll;
	};
	const finishDrag = () => {
		drag.current = null;
		startHideCountdown();
	};
	if (!geometry.visible) return null;
	return (
		<div
			className='ct-rounded-scrollbar'
			role='scrollbar'
			aria-orientation='vertical'
			aria-valuemin={0}
			aria-valuemax={geometry.maximumScroll}
			aria-valuenow={viewportRef.current?.scrollTop ?? 0}
			data-shown={shown || undefined}
			data-right-margin={shouldHaveRightMargin || undefined}
			onMouseEnter={show}
			onMouseLeave={startHideCountdown}
		>
			<div
				className='ct-rounded-scrollbar__thumb'
				style={{
					top: geometry.top,
					height: geometry.height,
				}}
				onPointerDown={onPointerDown}
				onPointerMove={onPointerMove}
				onPointerUp={finishDrag}
				onPointerCancel={finishDrag}
			/>
		</div>
	);
}
