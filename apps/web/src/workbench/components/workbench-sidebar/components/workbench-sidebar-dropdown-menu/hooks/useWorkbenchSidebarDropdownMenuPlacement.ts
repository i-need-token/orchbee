import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import { takeNavigationRowMenuKeyboardOrigin } from '../../project-navigation-item/utils/navigation-row-menu';

interface WorkbenchSidebarDropdownMenuPlacement {
	readonly alignOffset: number;
	readonly side: 'bottom' | 'right';
	readonly sideOffset: number;
}

/** The shared menu primitive's offset from its trigger. */
const TRIGGER_OFFSET = 4;
/** How far past its row's end a menu opened from the keyboard starts. */
const ROW_OFFSET = 12;

const BELOW_TRIGGER: WorkbenchSidebarDropdownMenuPlacement = {
	alignOffset: 0,
	side: 'bottom',
	sideOffset: TRIGGER_OFFSET,
};

/**
 * Where a row's menu opens. From the pointer it opens below the ⋯ or + that opened it, start edges
 * aligned; from Shift+F10 or the Menu key it opens beside its row, top edges level, as the TUI's
 * does. The primitive's collision handling moves it where the window is too small.
 */
export function useWorkbenchSidebarDropdownMenuPlacement(): WorkbenchSidebarDropdownMenuPlacement & {
	readonly onOpenChange: (open: boolean) => void;
	readonly triggerRef: React.RefObject<HTMLButtonElement | null>;
} {
	const triggerRef = React.useRef<HTMLButtonElement>(null);
	const [placement, setPlacement] = React.useState(BELOW_TRIGGER);
	const onOpenChange = useMemoizedFn((open: boolean) => {
		const trigger = triggerRef.current;
		if (!open || trigger === null) return;
		const row = takeNavigationRowMenuKeyboardOrigin(trigger);
		if (row === null) {
			setPlacement(BELOW_TRIGGER);
			return;
		}
		const rowBounds = row.getBoundingClientRect();
		const triggerBounds = trigger.getBoundingClientRect();
		setPlacement({
			alignOffset: rowBounds.top - triggerBounds.top,
			side: 'right',
			sideOffset: rowBounds.right - triggerBounds.right + ROW_OFFSET,
		});
	});
	return { ...placement, onOpenChange, triggerRef };
}
