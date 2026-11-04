/** A terminal Agent's report state; only explicit reports say done or error. */
export type WorkbenchAgentState = 'blocked' | 'done' | 'error' | 'idle' | 'unknown' | 'working';

/** The Agent the Server observes in one terminal Presentation. */
export interface WorkbenchAgentObservation {
	/** The reported Agent label: a detected provider such as `claude`, or a custom report's name. */
	readonly agent: string;
	/** The exact terminal incarnation. A replacement terminal never inherits an acknowledgement. */
	readonly binding: string;
	/** Advances only when the Agent label or state changes. */
	readonly sequence: bigint;
	readonly state: WorkbenchAgentState;
}

export interface WorkbenchAgentSnapshot {
	/** Set once every Presentation has been observed; entries before that are partial. */
	readonly complete: boolean;
	/** The Presentations holding an Agent, by Presentation ID. */
	readonly presentations: ReadonlyMap<string, WorkbenchAgentObservation>;
}

/** The Server's terminal Agent observations as this Client reads them. */
export interface WorkbenchAgentSource {
	getSnapshot(): WorkbenchAgentSnapshot;
	subscribe(listener: () => void): () => void;
}
