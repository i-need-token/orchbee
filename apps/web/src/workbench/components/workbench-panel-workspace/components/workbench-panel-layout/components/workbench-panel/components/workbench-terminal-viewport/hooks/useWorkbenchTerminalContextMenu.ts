import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { PanelSplitDirection } from '../../../../../../../model/panel-workspace-state';
import { findTerminalFocusTarget } from '../../../../../../../../../utils/workbench-terminal-focus-target';

interface UseWorkbenchTerminalContextMenuOptions {
	readonly panelId: string;
	readonly splitPanel: (
		panelId: string,
		direction: PanelSplitDirection,
		source: 'context-menu',
	) => string | null;
}

interface WorkbenchTerminalContextMenu {
	readonly captureFinalFocus: (viewport: HTMLElement) => void;
	readonly finalFocus: () => boolean | HTMLElement;
	readonly split: (direction: PanelSplitDirection) => void;
	readonly transferFocus: () => void;
}

export function useWorkbenchTerminalContextMenu({
	panelId,
	splitPanel,
}: UseWorkbenchTerminalContextMenuOptions): WorkbenchTerminalContextMenu {
	const focusTransferred = React.useRef(false);
	const transferFocus = useMemoizedFn(() => {
		focusTransferred.current = true;
	});
	const capturedFinalFocus = React.useRef<{
		readonly active: HTMLElement;
		readonly viewport: HTMLElement;
	} | null>(null);
	const captureFinalFocus = (viewport: HTMLElement): void => {
		const { ownerDocument } = viewport;
		const activeElement = ownerDocument.activeElement;
		const HTMLElementConstructor = ownerDocument.defaultView?.HTMLElement;
		capturedFinalFocus.current =
			HTMLElementConstructor !== undefined && activeElement instanceof HTMLElementConstructor
				? { active: activeElement, viewport }
				: null;
	};
	const split = (direction: PanelSplitDirection): void => {
		const newPanelId = splitPanel(panelId, direction, 'context-menu');
		if (newPanelId !== null) transferFocus();
	};
	const finalFocus = (): boolean | HTMLElement => {
		if (focusTransferred.current) {
			focusTransferred.current = false;
			capturedFinalFocus.current = null;
			return false;
		}
		const captured = capturedFinalFocus.current;
		capturedFinalFocus.current = null;
		if (captured === null || !captured.active.isConnected || focusMovedOn(captured.active)) {
			return false;
		}
		return captured.active === captured.viewport
			? findTerminalFocusTarget(captured.viewport)
			: captured.active;
	};

	return { captureFinalFocus, finalFocus, split, transferFocus };
}

/**
 * The menu closes after a surface its command opened has taken focus, and Base UI takes focus back
 * unconditionally for an explicit target, unlike the default one.
 */
function focusMovedOn(finalFocusTarget: HTMLElement): boolean {
	const { activeElement, body } = finalFocusTarget.ownerDocument;
	return (
		activeElement !== null &&
		activeElement !== finalFocusTarget &&
		activeElement !== body &&
		activeElement.closest('[role="menu"]') === null
	);
}
