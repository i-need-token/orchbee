import * as React from 'react';

import { registerPointerDropTarget, type PointerDropTarget } from '../../../../../pointer-drag';

/**
 * Lets drags drop on the scrim through `target` while both exist, and says whether a drag over the
 * scrim would drop there now, which lights it up.
 */
export function useFileOverlayScrimDrop(
	target: PointerDropTarget | null,
	scrim: HTMLElement | null,
): boolean {
	const [dropTarget, setDropTarget] = React.useState(false);
	React.useEffect(() => {
		if (target === null || scrim === null) return undefined;
		return registerPointerDropTarget(scrim, {
			drop: (drag, point) => {
				setDropTarget(false);
				target.drop(drag, point);
			},
			leave: (drag) => {
				setDropTarget(false);
				target.leave(drag);
			},
			over: (drag, point) => {
				const accepted = target.over(drag, point);
				setDropTarget(accepted);
				return accepted;
			},
		});
	}, [scrim, target]);
	return dropTarget;
}
