/**
 * Why the Session's move onto this Client's version waits, or `updating` while it runs: a configured
 * command or plugin job runs in the Session, an older Client that cannot reconnect by itself is
 * connected, every terminal would restart, a failed move stopped moving terminals off a Terminal
 * Host of the older `sessionVersion`, or another reason.
 */
type WorkbenchSessionUpdateState =
	| 'background-programs'
	| 'clients'
	| 'host-required'
	| 'moves-stopped'
	| 'other'
	| 'updating';

/**
 * What Update offers: a newer `version` of this Client to switch to, or, while the Session's Server
 * runs the older `sessionVersion`, or a Terminal Host of that version still runs terminals, a move
 * of the Session onto this Client's own `version`.
 */
export type WorkbenchUpdateOffer =
	| { readonly kind: 'client'; readonly version: string }
	| {
			readonly kind: 'session';
			readonly sessionVersion: string;
			readonly state: WorkbenchSessionUpdateState;
			readonly version: string;
	  };

/**
 * A newer version of this Client that its host can switch to while the Session's terminals keep
 * running, or a move of an older Session onto this Client's version that the host could not make by
 * itself, and the host's answer when either has to wait. A host without one offers no Update.
 */
export interface WorkbenchClientUpdate {
	/**
	 * Check for updates, where the Session can run its automatic-update job on request; its result
	 * reaches this offer as any installed version does.
	 */
	readonly check?: WorkbenchUpdateCheck | undefined;
	/** What is offered now, or null while nothing is. */
	readonly getSnapshot: () => WorkbenchUpdateOffer | null;
	/** Asks the host to carry out the offer whose `version` this names. */
	readonly request: (version: string) => void;
	readonly subscribe: (listener: () => void) => () => void;
	/** Calls back each time a requested switch fails to launch the newer window. */
	readonly subscribeFailed?: ((listener: () => void) => () => void) | undefined;
	/**
	 * Calls back each time a request waits: a switch until no terminal runs, a Session move for the
	 * reason its offer names.
	 */
	readonly subscribeWaiting: (listener: () => void) => () => void;
	/**
	 * Calls back when a requested switch needs a new Terminal Host, which restarts every terminal,
	 * with the terminals that could not move off it once the Session runs the newer version, or null
	 * before the Session restarts; the window asks the person and answers with
	 * {@link answerHostReplacement}.
	 */
	readonly subscribeHostReplacement?:
		| ((listener: (terminals: number | null) => void) => () => void)
		| undefined;
	readonly answerHostReplacement?: ((confirmed: boolean) => void) | undefined;
}

/**
 * Why Check for updates cannot run, in the order it is decided: a development build of the window,
 * an installation with no release store or window update (or a Session Server that runs no update
 * job), or a Session whose handshake lacks the request.
 */
export type WorkbenchUpdateCheckUnavailableReason = 'development' | 'installation' | 'session';

/**
 * The phase of a run of the Session Server's update job that a failure ended in: reading the release
 * feed and the store, downloading, or installing.
 */
export type WorkbenchUpdatePhase = 'check' | 'download' | 'install';

/**
 * What Check for updates shows: a run queued or reading the feed, downloading `version` with the
 * whole `percent` of everything it downloads that has arrived, or installing, and how a run this
 * window asked for ended.
 */
export type WorkbenchUpdateCheckStatus =
	| { readonly kind: 'checking' | 'idle' | 'installing' | 'up-to-date' }
	| { readonly kind: 'downloading'; readonly percent: number; readonly version: string }
	| { readonly kind: 'failed'; readonly phase: WorkbenchUpdatePhase }
	| { readonly kind: 'unavailable'; readonly reason: WorkbenchUpdateCheckUnavailableReason };

/**
 * What one `workbench.update-check` exchange came to: where the Session Server's job stands, how its
 * last run ended, or `not-answered` when the answer was refused, lost, late, or unreadable. Only a
 * Session that lists `update-phases` answers a download or an install, and names the phase of a
 * failure; another one's failure reads as the check's.
 */
export type WorkbenchUpdateCheckAnswer =
	| {
			readonly kind:
				| 'checking'
				| 'idle'
				| 'installed'
				| 'installing'
				| 'not-answered'
				| 'unavailable'
				| 'up-to-date';
	  }
	| {
			readonly kind: 'downloading';
			readonly receivedBytes: number;
			readonly totalBytes: number;
			readonly version: string;
	  }
	| { readonly kind: 'run-failed'; readonly phase: WorkbenchUpdatePhase };

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
	 * queued or under way, the status is read about once a second.
	 */
	readonly show: () => () => void;
	readonly subscribe: (listener: () => void) => () => void;
}
