/**
 * A newer version of this Client that its host can switch to while the Session's terminals keep
 * running, and the host's answer when a switch has to wait. A host without one offers no Update.
 */
export interface WorkbenchClientUpdate {
	/**
	 * Check for updates, where the Session can run its automatic-update job on request; its result
	 * reaches this offer as any installed version does.
	 */
	readonly check?: WorkbenchUpdateCheck | undefined;
	/** The version offered now, or null while none is. */
	readonly getSnapshot: () => string | null;
	/** Asks the host to switch to `version`, the version it offered. */
	readonly request: (version: string) => void;
	readonly subscribe: (listener: () => void) => () => void;
	/** Calls back each time a requested switch waits until no terminal runs. */
	readonly subscribeWaiting: (listener: () => void) => () => void;
}

/**
 * Why Check for updates cannot run, in the order it is decided: a development build of the window,
 * an installation with no release store or window update (or a Session Server that runs no update
 * job), or a Session whose handshake lacks the request.
 */
export type WorkbenchUpdateCheckUnavailableReason = 'development' | 'installation' | 'session';

/** What Check for updates shows. */
export type WorkbenchUpdateCheckStatus =
	| { readonly kind: 'checking' | 'failed' | 'idle' | 'up-to-date' }
	| { readonly kind: 'unavailable'; readonly reason: WorkbenchUpdateCheckUnavailableReason };

/**
 * What one `workbench.update-check` exchange came to: where the Session Server's job stands, how its
 * last run ended, or `not-answered` when the answer was refused, lost, late, or unreadable.
 */
export type WorkbenchUpdateCheckAnswer =
	| 'checking'
	| 'idle'
	| 'installed'
	| 'not-answered'
	| 'run-failed'
	| 'unavailable'
	| 'up-to-date';

/**
 * Check for updates: asks the Session Server to run its automatic-update job now. A status shows
 * only a check this window started; a background run shows nothing but its offer.
 */
export interface WorkbenchUpdateCheck {
	readonly getSnapshot: () => WorkbenchUpdateCheckStatus;
	/** Check for updates was pressed. */
	readonly start: () => void;
	/**
	 * A surface shows the status from now until the returned call; while one does and a run is
	 * queued or running, the status is read about once a second.
	 */
	readonly show: () => () => void;
	readonly subscribe: (listener: () => void) => () => void;
}
