import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { PanelResizeAdjustment, PanelResizeMultiplier } from '../model/resolvePanelResize';
import { resolvePanelResizeKey } from '../model/resolvePanelResizeKey';
import type {
	PanelWorkspaceMessages,
	PendingPanelResize,
	WorkbenchPanelWorkspaceView,
} from '../types/panel-workspace-view';

interface WorkbenchPanelResizeDialogHost {
	readonly messages: Pick<PanelWorkspaceMessages, 'resizeUnavailable'>;
	readonly pendingResize: PendingPanelResize | null;
	readonly resizePanel: WorkbenchPanelWorkspaceView['resizePanel'];
}

interface WorkbenchPanelResizeDialogView {
	readonly finalFocus: () => HTMLElement | null;
	readonly onKeyDownCapture: (event: React.KeyboardEvent<HTMLElement>) => void;
	readonly resize: (adjustment: PanelResizeAdjustment, multiplier: PanelResizeMultiplier) => void;
	readonly status: string | null;
}

export function useWorkbenchPanelResizeDialog(
	view: WorkbenchPanelResizeDialogHost,
): WorkbenchPanelResizeDialogView {
	const [status, setStatus] = React.useState<string | null>(null);
	const pendingResize = view.pendingResize;
	React.useEffect(() => {
		setStatus(null);
	}, [pendingResize?.panelId, pendingResize?.tabId]);
	const stillAsksFor = useMemoizedFn(
		(asked: PendingPanelResize) =>
			pendingResize?.panelId === asked.panelId && pendingResize.tabId === asked.tabId,
	);
	const resize = useMemoizedFn(
		(adjustment: PanelResizeAdjustment, multiplier: PanelResizeMultiplier) => {
			if (pendingResize === null) {
				setStatus(view.messages.resizeUnavailable);
				return;
			}
			void view.resizePanel(pendingResize.panelId, adjustment, multiplier).then((resized) => {
				if (stillAsksFor(pendingResize)) {
					setStatus(resized ? null : view.messages.resizeUnavailable);
				}
			});
		},
	);
	const onKeyDownCapture = useMemoizedFn((event: React.KeyboardEvent<HTMLElement>) => {
		const resolution = resolvePanelResizeKey(event);
		if (resolution === null) return;
		event.preventDefault();
		event.stopPropagation();
		resize(resolution.adjustment, resolution.multiplier);
	});
	const finalFocus = useMemoizedFn(() => {
		if (typeof document === 'undefined' || pendingResize === null) return null;
		const viewport = Array.from(
			document.querySelectorAll<HTMLElement>('[data-slot="workbench-terminal-viewport"]'),
		).find((element) => element.dataset['panelId'] === pendingResize.panelId);
		return viewport?.querySelector<HTMLElement>('textarea, [role="textbox"]') ?? viewport ?? null;
	});

	return { finalFocus, onKeyDownCapture, resize, status };
}
