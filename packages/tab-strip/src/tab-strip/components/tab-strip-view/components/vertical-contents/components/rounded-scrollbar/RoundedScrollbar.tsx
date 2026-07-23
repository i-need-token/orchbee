import styles from './RoundedScrollbar.module.css';
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
			className={styles['ct-rounded-scrollbar']}
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
				className={styles['ct-rounded-scrollbar__thumb']}
				style={{
					'--ob-comp-tab-strip-block-start': `${view.geometry.top}px`,
					'--ob-comp-tab-strip-block-size': `${view.geometry.height}px`,
				}}
				onPointerDown={view.onPointerDown}
				onPointerMove={view.onPointerMove}
				onPointerUp={view.finishDrag}
				onPointerCancel={view.finishDrag}
			/>
		</div>
	);
}
