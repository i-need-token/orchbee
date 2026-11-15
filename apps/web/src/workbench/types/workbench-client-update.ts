/**
 * A newer version of this Client that its host can switch to while the Session's terminals keep
 * running, and the host's answer when a switch has to wait. A host without one offers no Update.
 */
export interface WorkbenchClientUpdate {
	/** The version offered now, or null while none is. */
	readonly getSnapshot: () => string | null;
	/** Asks the host to switch to `version`, the version it offered. */
	readonly request: (version: string) => void;
	readonly subscribe: (listener: () => void) => () => void;
	/** Calls back each time a requested switch waits until no terminal runs. */
	readonly subscribeWaiting: (listener: () => void) => () => void;
}
