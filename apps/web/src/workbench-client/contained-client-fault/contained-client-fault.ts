/**
 * A fault the window contained to one handler, listener, or stream instead of letting it unwind
 * through the code that serves everything else. Its message names only the site, a closed token,
 * and the original error travels as its cause.
 */
export type ContainedClientFaultSite =
	| 'command-decision'
	| 'connection-end-listener'
	| 'message-dispatch'
	| 'publication-apply'
	| 'session-end-listener'
	| 'snapshot-listener'
	| 'source-listener'
	| 'subscription-closed'
	| 'subscription-frame'
	| 'subscription-gap'
	| 'terminal-poll-record'
	| 'terminal-step'
	| 'terminal-surface-render'
	| 'viewer-ready-listener'
	| 'workbench-render';

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

/** Calls every listener in turn; one that throws loses only its own call and is reported. */
export function notifyEachListener<Listener>(
	listeners: Iterable<Listener>,
	site: ContainedClientFaultSite,
	call: (listener: Listener) => void,
): void {
	// A copy, so a listener that subscribes another during the call does not extend this round.
	for (const listener of Array.from(listeners)) {
		try {
			call(listener);
		} catch (error) {
			reportContainedClientFault(new ContainedClientFault(site, error));
		}
	}
}
