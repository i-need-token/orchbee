/** An Agent session's status as the Server reports it; the board words it and never reinterprets it. */
export type WorkbenchAgentBoardStatus =
	| { readonly count: number; readonly kind: 'needs-attention' }
	| {
			readonly kind:
				| 'canceled'
				| 'ended'
				| 'failed'
				| 'idle'
				| 'interrupted'
				| 'succeeded'
				| 'unknown'
				| 'working';
	  };

/** One Agent session the Server owns. Its session id keys selection and jumps and never shows. */
export interface WorkbenchAgentBoardRow {
	readonly ended: boolean;
	readonly freshness: 'disconnected' | 'fresh' | 'stale';
	/** The terminal that names the row: the session's first navigable one, else null. */
	readonly presentationId: string | null;
	readonly providerId: string;
	readonly sessionId: string;
	readonly status: WorkbenchAgentBoardStatus;
}

export type WorkbenchAgentBoardReadFailure = 'denied' | 'transport' | 'unavailable';

/** The last list attempt. A failed or cut-short one keeps the rows of the last whole one. */
type WorkbenchAgentBoardRead =
	| { readonly kind: 'unread' }
	| { readonly kind: 'reading' }
	| { readonly complete: boolean; readonly kind: 'listed' }
	| { readonly kind: 'failed'; readonly reason: WorkbenchAgentBoardReadFailure };

export type WorkbenchAgentBoardJumpFailure =
	| 'denied'
	| 'ended'
	| 'expired'
	| 'gone'
	| 'no-placement'
	| 'unavailable';

export interface WorkbenchAgentBoardSnapshot {
	readonly jumping: boolean;
	/** Why the last jump stopped, until a later jump, refresh or list. */
	readonly notice: WorkbenchAgentBoardJumpFailure | null;
	readonly read: WorkbenchAgentBoardRead;
	/** Sessions needing attention first, otherwise in the Server's order. */
	readonly rows: readonly WorkbenchAgentBoardRow[];
}

/** `shown` once the session's terminal is in front; `superseded` by a later jump or a close. */
export type WorkbenchAgentBoardJump = 'failed' | 'shown' | 'superseded';

/** The Server's Agent sessions, which the Agent board lists and opens. */
export interface WorkbenchAgentBoard {
	getSnapshot(): WorkbenchAgentBoardSnapshot;
	subscribe(listener: () => void): () => void;
	/** Starts an episode, which reads once two seconds have passed since the last read ended. */
	open(): void;
	/** Ends the episode and drops its waiting read and jump; the rows stay for the next one. */
	close(): void;
	/** Reads again; ignored within two seconds of the last read's end or while a jump waits. */
	refresh(): void;
	/** Checks the session again, then shows its terminal; a later jump or a close supersedes it. */
	jump(sessionId: string): Promise<WorkbenchAgentBoardJump>;
}
