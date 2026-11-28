import { useMemoizedFn } from '@orchbee/foundation/react';
import { useImperativeHandle, useMemo, useRef, useState, type Ref } from 'react';

import type { FileOverlayFocus } from '../../../file-overlay-contract';

/**
 * The asks to give the overlay the keys, as its components see them: the active tab takes an ask
 * while the overlay is open, picture-in-picture while it is shrunk.
 */
export interface FileOverlayFocusRequest {
	/** Asks from inside the overlay, as restoring picture-in-picture gives the active tab the keys. */
	readonly ask: () => void;
	/** How many asks there have been; each renders the overlay again. */
	readonly count: number;
	/** Takes the ask that waits: true once for each ask, then false until the next. */
	readonly take: () => boolean;
}

/**
 * Receives the Workbench's asks to give the overlay the keys. An ask waits until the overlay shows
 * the change dispatched with it, which can take more than one render while the overlay opens.
 */
export function useFileOverlayFocusRequest(
	reference: Ref<FileOverlayFocus> | undefined,
): FileOverlayFocusRequest {
	const [count, setCount] = useState(0);
	const taken = useRef(0);
	const ask = useMemoizedFn(() => setCount((asked) => asked + 1));
	useImperativeHandle(reference, () => ({ focus: ask }), [ask]);
	// Each render's own take: the row's layout effects run before this component's, so a callback
	// that this component's layout effect brings up to date would still hold the previous count.
	return useMemo(
		() => ({
			ask,
			count,
			take: (): boolean => {
				if (taken.current === count) return false;
				taken.current = count;
				return true;
			},
		}),
		[ask, count],
	);
}
