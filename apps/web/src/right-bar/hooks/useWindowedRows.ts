import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

/** Rows drawn past each edge of the viewport, so a quick scroll shows no blank band. */
const OVERSCAN_ROWS = 8;
/** The row height until a drawn row has been measured. */
const FALLBACK_ROW_HEIGHT = 24;

export interface WindowedRows {
	readonly viewportReference: React.RefObject<HTMLDivElement | null>;
	readonly onScroll: (event: React.UIEvent<HTMLDivElement>) => void;
	/** Takes a drawn row's measured height, which every row shares. */
	readonly measureRow: (row: HTMLElement | null) => void;
	readonly first: number;
	readonly last: number;
	readonly rowHeight: number;
	readonly totalHeight: number;
	/** Scrolls the least distance that brings a row fully into view. */
	readonly revealRow: (index: number) => void;
}

/**
 * Draws only the rows of a long, fixed-height list that the viewport shows, so a folder of
 * thousands of names or a long file costs the window a screenful of elements.
 */
export function useWindowedRows(count: number): WindowedRows {
	const viewportReference = React.useRef<HTMLDivElement | null>(null);
	const [scrollTop, setScrollTop] = React.useState(0);
	const [viewportHeight, setViewportHeight] = React.useState(0);
	const [rowHeight, setRowHeight] = React.useState(FALLBACK_ROW_HEIGHT);
	React.useLayoutEffect(() => {
		const viewport = viewportReference.current;
		if (viewport === null) return undefined;
		setViewportHeight(viewport.clientHeight);
		if (typeof ResizeObserver === 'undefined') return undefined;
		const observer = new ResizeObserver(() => setViewportHeight(viewport.clientHeight));
		observer.observe(viewport);
		return () => observer.disconnect();
	}, []);
	const onScroll = useMemoizedFn((event: React.UIEvent<HTMLDivElement>) => {
		setScrollTop(event.currentTarget.scrollTop);
	});
	const measureRow = useMemoizedFn((row: HTMLElement | null) => {
		const height = row?.offsetHeight ?? 0;
		if (height > 0 && height !== rowHeight) setRowHeight(height);
	});
	const revealRow = useMemoizedFn((index: number) => {
		const viewport = viewportReference.current;
		if (viewport === null) return;
		const top = index * rowHeight;
		const bottom = top + rowHeight;
		if (top < viewport.scrollTop) {
			viewport.scrollTop = top;
		} else if (bottom > viewport.scrollTop + viewport.clientHeight) {
			viewport.scrollTop = bottom - viewport.clientHeight;
		}
		setScrollTop(viewport.scrollTop);
	});
	const first = Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN_ROWS);
	const last = Math.min(
		count,
		Math.ceil((scrollTop + Math.max(viewportHeight, rowHeight)) / rowHeight) + OVERSCAN_ROWS,
	);
	return {
		first,
		last,
		measureRow,
		onScroll,
		revealRow,
		rowHeight,
		totalHeight: count * rowHeight,
		viewportReference,
	};
}
