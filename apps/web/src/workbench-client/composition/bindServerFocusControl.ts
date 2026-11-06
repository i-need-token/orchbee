import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';
import { hasWorkbenchExclusiveFocusSurface } from '../../workbench';
import type { ServerTerminalProjectionClient } from '../server-workbench';
import type { ServerTitleWindow } from './bindServerWindowTitle';

/** One foreground activation per target, cancelled by blur, a modal or target replacement. */
export function bindServerFocusControl({
	readTarget,
	subscribeNavigation,
	projectionClient,
	titleWindow,
	ownerDocument,
}: {
	readonly readTarget: () => TerminalPresentationId | null;
	readonly subscribeNavigation: (listener: () => void) => () => void;
	readonly projectionClient: Pick<ServerTerminalProjectionClient, 'activateFocus'>;
	readonly titleWindow: ServerTitleWindow;
	readonly ownerDocument: Document | undefined;
}): { dispose(): void } {
	let current: string | null = null;
	let activated = false;
	let abort: AbortController | null = null;
	const refresh = (): void => {
		const id =
			titleWindow.document.hasFocus() &&
			(ownerDocument === undefined || !hasWorkbenchExclusiveFocusSurface(ownerDocument))
				? readTarget()
				: null;
		if (id === current) return;
		abort?.abort();
		abort = null;
		current = id;
		if (id === null) return;
		abort = new AbortController();
		projectionClient.activateFocus(id, !activated, abort.signal);
		activated = true;
	};
	const stop = subscribeNavigation(refresh);
	const observer = ownerDocument === undefined ? null : new MutationObserver(refresh);
	if (ownerDocument !== undefined)
		observer?.observe(ownerDocument.body, { childList: true, subtree: true });
	titleWindow.addEventListener('focus', refresh);
	titleWindow.addEventListener('blur', refresh);
	refresh();
	return {
		dispose: () => {
			abort?.abort();
			observer?.disconnect();
			stop();
			titleWindow.removeEventListener('focus', refresh);
			titleWindow.removeEventListener('blur', refresh);
		},
	};
}
