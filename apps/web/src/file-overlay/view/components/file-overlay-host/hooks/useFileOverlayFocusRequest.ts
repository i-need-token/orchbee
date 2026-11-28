import { useImperativeHandle, useMemo, useRef, useState, type Ref } from 'react';

import type { FileOverlayFocus } from '../../../file-overlay-contract';

/** The Workbench's asks to give the active tab the keys, as the overlay's components see them. */
export interface FileOverlayFocusRequest {
	/** How many asks there have been; each renders the overlay again. */
	readonly count: number;
	/** Takes the ask that waits: true once for each ask, then false until the next. */
	readonly take: () => boolean;
}

/**
 * Receives the Workbench's asks to give the active tab the keys. An ask waits until the row shows
 * the change dispatched with it, which can take more than one render while the overlay opens.
 */
export function useFileOverlayFocusRequest(
	reference: Ref<FileOverlayFocus> | undefined,
): FileOverlayFocusRequest {
	const [count, setCount] = useState(0);
	const taken = useRef(0);
	useImperativeHandle(reference, () => ({ focus: () => setCount((asked) => asked + 1) }), []);
	// Each render's own take: the row's layout effects run before this component's, so a callback
	// that this component's layout effect brings up to date would still hold the previous count.
	return useMemo(
		() => ({
			count,
			take: (): boolean => {
				if (taken.current === count) return false;
				taken.current = count;
				return true;
			},
		}),
		[count],
	);
}
