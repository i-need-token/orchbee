import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import { readWorkbenchMenuReferenceInlineSize } from '../utils/readWorkbenchMenuReferenceInlineSize';
import {
	resolveWorkbenchSidebarDropdownMenuPlacement,
	type WorkbenchSidebarDropdownMenuPlacement,
} from '../utils/resolveWorkbenchSidebarDropdownMenuPlacement';

export function useWorkbenchSidebarDropdownMenuPlacement(): {
	readonly align: 'end' | 'start';
	readonly onOpenChange: (open: boolean) => void;
	readonly side: 'bottom' | 'right';
	readonly triggerRef: React.RefObject<HTMLButtonElement | null>;
} {
	const triggerRef = React.useRef<HTMLButtonElement>(null);
	const [open, setOpen] = React.useState(false);
	const [placement, setPlacement] =
		React.useState<WorkbenchSidebarDropdownMenuPlacement>('right-start');

	const updatePlacement = useMemoizedFn(() => {
		const trigger = triggerRef.current;
		if (trigger === null) {
			return;
		}

		const ownerDocument = trigger.ownerDocument;
		const triggerBounds = trigger.getBoundingClientRect();
		setPlacement(
			resolveWorkbenchSidebarDropdownMenuPlacement({
				menuInlineSize: readWorkbenchMenuReferenceInlineSize(ownerDocument),
				triggerInlineEnd: triggerBounds.right,
				viewportInlineSize: ownerDocument.documentElement.clientWidth,
			}),
		);
	});

	const onOpenChange = useMemoizedFn((nextOpen: boolean) => {
		setOpen(nextOpen);
		if (nextOpen) {
			updatePlacement();
		}
	});

	React.useEffect(() => {
		const trigger = triggerRef.current;
		const ownerWindow = trigger?.ownerDocument.defaultView;
		const ResizeObserverConstructor = ownerWindow?.ResizeObserver;
		if (
			!open ||
			trigger === null ||
			ownerWindow === null ||
			ownerWindow === undefined ||
			ResizeObserverConstructor === undefined
		) {
			return;
		}

		const resizeObserver = new ResizeObserverConstructor(updatePlacement);
		resizeObserver.observe(trigger.ownerDocument.documentElement);
		const sidebar = trigger.closest('[data-slot="workbench-sidebar"]');
		if (sidebar !== null) {
			resizeObserver.observe(sidebar);
		}
		return () => resizeObserver.disconnect();
	}, [open, updatePlacement]);

	return {
		align: placement === 'right-start' ? 'start' : 'end',
		onOpenChange,
		side: placement === 'right-start' ? 'right' : 'bottom',
		triggerRef,
	};
}
