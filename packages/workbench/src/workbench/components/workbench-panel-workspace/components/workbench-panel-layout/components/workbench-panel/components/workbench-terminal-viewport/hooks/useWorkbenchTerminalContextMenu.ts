import * as React from 'react';

import type { PanelSplitDirection } from '../../../../../../../model/panel-workspace-state';

interface UseWorkbenchTerminalContextMenuOptions {
	readonly panelId: string;
	readonly splitPanel: (
		panelId: string,
		direction: PanelSplitDirection,
		source: 'context-menu',
	) => string | null;
}

interface WorkbenchTerminalContextMenu {
	readonly captureFinalFocus: (ownerDocument: Document) => void;
	readonly finalFocus: () => boolean | HTMLElement;
	readonly split: (direction: PanelSplitDirection) => void;
}

export function useWorkbenchTerminalContextMenu({
	panelId,
	splitPanel,
}: UseWorkbenchTerminalContextMenuOptions): WorkbenchTerminalContextMenu {
	const transferFocusToSplit = React.useRef(false);
	const capturedFinalFocus = React.useRef<HTMLElement | null>(null);
	const captureFinalFocus = (ownerDocument: Document): void => {
		const activeElement = ownerDocument.activeElement;
		const HTMLElementConstructor = ownerDocument.defaultView?.HTMLElement;
		capturedFinalFocus.current =
			HTMLElementConstructor !== undefined && activeElement instanceof HTMLElementConstructor
				? activeElement
				: null;
	};
	const split = (direction: PanelSplitDirection): void => {
		const newPanelId = splitPanel(panelId, direction, 'context-menu');
		if (newPanelId !== null) transferFocusToSplit.current = true;
	};
	const finalFocus = (): boolean | HTMLElement => {
		if (transferFocusToSplit.current) {
			transferFocusToSplit.current = false;
			capturedFinalFocus.current = null;
			return false;
		}
		const finalFocusTarget = capturedFinalFocus.current;
		capturedFinalFocus.current = null;
		return finalFocusTarget ?? false;
	};

	return { captureFinalFocus, finalFocus, split };
}
