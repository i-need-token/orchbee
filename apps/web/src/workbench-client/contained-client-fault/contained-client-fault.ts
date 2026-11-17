/**
 * A fault the window contained to one handler, listener, or stream instead of letting it unwind
 * through the code that serves everything else. Its message names only the site, a closed token,
 * and the original error travels as its cause.
 */
export type ContainedClientFaultSite =
	| 'connection-end-listener'
	| 'message-dispatch'
	| 'subscription-closed'
	| 'subscription-frame'
	| 'subscription-gap'
	| 'viewer-ready-listener';

export class ContainedClientFault extends Error {
	readonly site: ContainedClientFaultSite;

	constructor(site: ContainedClientFaultSite, cause: unknown) {
		super(`Contained client fault: ${site}.`, { cause });
		this.name = 'ContainedClientFault';
		this.site = site;
	}
}

export type ContainedClientFaultReporter = (fault: ContainedClientFault) => void;

/**
 * Reports through the page's own error reporting, where an uncaught error would have gone, without
 * unwinding the caller. A runtime without `reportError`, such as Node under test, drops it.
 */
export function reportContainedClientFault(fault: ContainedClientFault): void {
	const report: unknown = Reflect.get(globalThis, 'reportError');
	if (typeof report !== 'function') return;
	try {
		Reflect.apply(report, globalThis, [fault]);
	} catch {
		// Reporting never owns or alters what it reports on.
	}
}
