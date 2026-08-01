import * as React from 'react';

import type { WorkbenchConfirmationAnchor } from '../../../types/workbench-confirmation';

interface WorkbenchConfirmationPopoverView {
	readonly confirmButtonRef: React.RefObject<HTMLButtonElement | null>;
	readonly virtualAnchor: { readonly getBoundingClientRect: () => DOMRect };
}

export function useWorkbenchConfirmationPopover(
	anchor: WorkbenchConfirmationAnchor,
): WorkbenchConfirmationPopoverView {
	const confirmButtonRef = React.useRef<HTMLButtonElement>(null);
	const virtualAnchor = React.useMemo(
		() => ({
			getBoundingClientRect: (): DOMRect => {
				const { x, y } = anchor;
				return {
					bottom: y,
					height: 0,
					left: x,
					right: x,
					top: y,
					width: 0,
					x,
					y,
					toJSON: () => ({ x, y }),
				};
			},
		}),
		[anchor],
	);
	return { confirmButtonRef, virtualAnchor };
}
