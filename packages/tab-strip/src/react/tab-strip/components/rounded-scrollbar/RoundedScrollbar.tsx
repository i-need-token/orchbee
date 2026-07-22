import type { ReactNode, RefObject } from 'react';

import { useRoundedScrollbar } from './hooks/useRoundedScrollbar';

export interface RoundedScrollbarProps {
	viewportRef: RefObject<HTMLElement | null>;
	isAnimatingSize?: boolean;
	shouldHaveRightMargin?: boolean;
}

export function RoundedScrollbar({
	viewportRef,
	isAnimatingSize = false,
	shouldHaveRightMargin = true,
}: RoundedScrollbarProps): ReactNode {
	const view = useRoundedScrollbar({ viewportRef, isAnimatingSize });
	if (!view.geometry.visible) return null;

	return (
		<div
			className='ct-rounded-scrollbar'
			role='scrollbar'
			aria-orientation='vertical'
			aria-valuemin={0}
			aria-valuemax={view.geometry.maximumScroll}
			aria-valuenow={view.scrollTop}
			data-shown={view.shown || undefined}
			data-right-margin={shouldHaveRightMargin || undefined}
			onMouseEnter={view.show}
			onMouseLeave={view.startHideCountdown}
		>
			<div
				className='ct-rounded-scrollbar__thumb'
				style={{ top: view.geometry.top, height: view.geometry.height }}
				onPointerDown={view.onPointerDown}
				onPointerMove={view.onPointerMove}
				onPointerUp={view.finishDrag}
				onPointerCancel={view.finishDrag}
			/>
		</div>
	);
}
