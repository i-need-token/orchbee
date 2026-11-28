import * as React from 'react';

import type { WorkbenchQuickInputSessions } from '../../../model/WorkbenchQuickInputSessions';

function focusedElement(): HTMLElement | null {
	const focused = document.activeElement;
	return focused instanceof HTMLElement && focused !== document.body ? focused : null;
}

/**
 * Gives the keys back to the element that held them before the quick input opened, as VS Code's
 * quick input does once it hides with the focus still inside: after Escape, or a pick whose requester
 * moves the focus nowhere else. A pick that puts something in front focuses it after this returns.
 * A session that replaces another keeps the element the first one found. The element is read as a
 * request opens, before the surface renders and takes the focus.
 */
export function useWorkbenchQuickInputFocusReturn(
	sessions: WorkbenchQuickInputSessions,
	surfaceReference: React.RefObject<HTMLElement | null>,
): void {
	React.useEffect(() => {
		let open = sessions.getSnapshot() !== null;
		let invoker: HTMLElement | null = null;
		return sessions.subscribe(() => {
			const opened = sessions.getSnapshot() !== null;
			if (opened === open) return;
			open = opened;
			if (opened) {
				invoker = focusedElement();
				return;
			}
			const surface = surfaceReference.current;
			const target = invoker;
			invoker = null;
			// The surface is still in the document: it leaves once React renders the ended session.
			if (surface === null || target === null || !target.isConnected) return;
			if (surface.contains(surface.ownerDocument.activeElement))
				target.focus({ preventScroll: true });
		});
	}, [sessions, surfaceReference]);
}
