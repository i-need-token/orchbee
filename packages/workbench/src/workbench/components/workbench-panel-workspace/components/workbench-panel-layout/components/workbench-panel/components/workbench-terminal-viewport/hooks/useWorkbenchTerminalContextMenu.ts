import * as React from 'react';

import type { PanelSplitDirection } from '../../../../../../../model/panel-workspace-state';

interface PendingPanelFocus {
	readonly ownerDocument: Document;
	readonly panelId: string;
}

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
	readonly split: (direction: PanelSplitDirection, ownerDocument: Document) => void;
}

export function useWorkbenchTerminalContextMenu({
	panelId,
	splitPanel,
}: UseWorkbenchTerminalContextMenuOptions): WorkbenchTerminalContextMenu {
	const pendingPanelFocus = React.useRef<PendingPanelFocus | null>(null);
	const capturedFinalFocus = React.useRef<HTMLElement | null>(null);
	const captureFinalFocus = (ownerDocument: Document): void => {
		const activeElement = ownerDocument.activeElement;
		const HTMLElementConstructor = ownerDocument.defaultView?.HTMLElement;
		capturedFinalFocus.current =
			HTMLElementConstructor !== undefined && activeElement instanceof HTMLElementConstructor
				? activeElement
				: null;
	};
	const split = (direction: PanelSplitDirection, ownerDocument: Document): void => {
		const newPanelId = splitPanel(panelId, direction, 'context-menu');
		if (newPanelId === null) {
			return;
		}
		pendingPanelFocus.current = { ownerDocument, panelId: newPanelId };
		ownerDocument.defaultView?.requestAnimationFrame(() => {
			ownerDocument
				.querySelector<HTMLElement>(
					`[data-slot="workbench-terminal-viewport"][data-panel-id="${newPanelId}"]`,
				)
				?.focus();
		});
	};
	const finalFocus = (): boolean | HTMLElement => {
		const pendingFocus = pendingPanelFocus.current;
		if (pendingFocus !== null) {
			pendingPanelFocus.current = null;
			capturedFinalFocus.current = null;
			return (
				pendingFocus.ownerDocument.querySelector<HTMLElement>(
					`[data-slot="workbench-terminal-viewport"][data-panel-id="${pendingFocus.panelId}"]`,
				) ?? false
			);
		}
		const finalFocusTarget = capturedFinalFocus.current;
		capturedFinalFocus.current = null;
		return finalFocusTarget ?? false;
	};

	return { captureFinalFocus, finalFocus, split };
}
