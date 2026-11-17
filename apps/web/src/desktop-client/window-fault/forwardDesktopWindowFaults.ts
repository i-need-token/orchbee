import {
	DESKTOP_WINDOW_FAULT_WINDOW_MESSAGE,
	type DesktopWindowFaultSite,
} from '@orchbee/desktop-client-protocol';

import { ContainedClientFault } from '../../workbench-client';

function siteOf(fault: unknown, uncontained: DesktopWindowFaultSite): DesktopWindowFaultSite {
	if (!(fault instanceof ContainedClientFault)) return uncontained;
	// Every site the window contains a fault to is one Main records.
	const site: DesktopWindowFaultSite = fault.site;
	return site;
}

/**
 * Tells Main, through the preload, the site of every fault this document contained or let escape,
 * so it lasts in the daily diagnostic log beyond the developer console. Contained faults arrive as
 * the error events `reportError` dispatches; only the site is sent, never a message or stack.
 */
export function forwardDesktopWindowFaults(targetWindow: Window): void {
	const post = (site: DesktopWindowFaultSite): void => {
		try {
			targetWindow.postMessage(
				{ site, type: DESKTOP_WINDOW_FAULT_WINDOW_MESSAGE },
				targetWindow.location.origin,
			);
		} catch {
			// Forwarding never adds a fault of its own.
		}
	};
	targetWindow.addEventListener('error', (event) => {
		if (event instanceof ErrorEvent) post(siteOf(event.error, 'uncaught-error'));
	});
	targetWindow.addEventListener('unhandledrejection', (event) => {
		post(siteOf(event.reason, 'unhandled-rejection'));
	});
}
