import type { ActionScope } from '@orchbee/actions';
import * as React from 'react';

import type { WorkbenchTerminalPathDrops } from '../../../../../../../../../model/WorkbenchTerminalPathDrops';

/**
 * Lets the File Bar's files and folders drop on a terminal's viewport while `drops` is given, and says
 * whether a drag over the viewport would drop there now.
 */
export function useWorkbenchTerminalPathDrop(
	drops: WorkbenchTerminalPathDrops | undefined,
	presentationId: string,
	scope: ActionScope,
	viewportRef: React.RefObject<HTMLElement | null>,
): boolean {
	const [dropTarget, setDropTarget] = React.useState(false);
	React.useEffect(() => {
		const viewport = viewportRef.current;
		if (drops === undefined || viewport === null) return undefined;
		return drops.registerTerminal(presentationId, scope, viewport, setDropTarget);
	}, [drops, presentationId, scope, viewportRef]);
	return dropTarget;
}
