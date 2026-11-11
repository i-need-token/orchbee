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

/**
 * One Agent the board lists: a canonical session the Server owns (`session`), or an Agent the
 * sidebar shows without one (`sidebar`), such as one started without a hook integration.
 */
export interface WorkbenchAgentBoardRow {
	readonly ended: boolean;
	readonly freshness: 'disconnected' | 'fresh' | 'stale';
	readonly origin: 'session' | 'sidebar';
	/** The terminal that names the row: the session's first navigable one, else null. */
	readonly presentationId: string | null;
	readonly providerId: string;
	/**
	 * Keys selection and jumps and never shows: the session id, or for a sidebar row a key derived
	 * from its Presentation.
	 */
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
	/**
	 * Rows needing attention first; otherwise sessions in the Server's order, then the sidebar's
	 * Agents no session attaches, in the sidebar's order.
	 */
	readonly rows: readonly WorkbenchAgentBoardRow[];
}

/** `shown` once the session's terminal is in front; `superseded` by a later jump or a close. */
export type WorkbenchAgentBoardJump = 'failed' | 'shown' | 'superseded';

/** The Server's Agent sessions and the sidebar's other Agents, which the Agent board lists and opens. */
export interface WorkbenchAgentBoard {
	getSnapshot(): WorkbenchAgentBoardSnapshot;
	subscribe(listener: () => void): () => void;
	/** Starts an episode, which reads once two seconds have passed since the last read ended. */
	open(): void;
	/** Ends the episode and drops its waiting read and jump; the rows stay for the next one. */
	close(): void;
	/** Reads again; ignored within two seconds of the last read's end or while a jump waits. */
	refresh(): void;
	/**
	 * Checks the session again, then shows its terminal; a later jump or a close supersedes it. A
	 * sidebar row has no session to check and shows its terminal at once.
	 */
	jump(sessionId: string): Promise<WorkbenchAgentBoardJump>;
}
