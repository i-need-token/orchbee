import type { ReactNode, RefObject } from 'react';

import { useRoundedScrollbar } from './hooks/useRoundedScrollbar';

export interface RoundedScrollbarProps {
	viewportRef: RefObject<HTMLElement | null>;
	isAnimatingSize?: boolean;
	shouldHaveRightMargin?: boolean;
}

export function RoundedScrollbar(props: RoundedScrollbarProps): ReactNode {
	return useRoundedScrollbar(props);
}
