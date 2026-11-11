/**
 * A terminal Agent's report state; only explicit reports say done or error. `opening`: the Agent
 * process is seen and its screen is not yet recognised.
 */
export type WorkbenchAgentState =
	| 'blocked'
	| 'done'
	| 'error'
	| 'idle'
	| 'opening'
	| 'unknown'
	| 'working';

/** The Agent the Server observes in one terminal Presentation. */
export interface WorkbenchAgentObservation {
	/** The reported Agent label: a detected provider such as `claude`, or a custom report's name. */
	readonly agent: string;
	/** The exact terminal incarnation. A replacement terminal never inherits an acknowledgement. */
	readonly binding: string;
	/** Advances only when the Agent label or state changes. */
	readonly sequence: bigint;
	readonly state: WorkbenchAgentState;
	/** Server turn acknowledgement, absent when this source cannot report it; never inferred locally. */
	readonly completion?: 'none' | 'awaiting-input' | 'acknowledged';
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

/** What the Server reports as naming one terminal's Agent, beyond the names this window holds. */
export interface WorkbenchAgentNames {
	/** The terminal incarnation these were read for; they name no other incarnation's Agent. */
	readonly binding: string;
	/** The name a person gave the Agent. */
	readonly name: string | null;
	/** The title an Agent report or Panel metadata gives the terminal. */
	readonly reportedTitle: string | null;
	/**
	 * What the terminal's own title names, read while this window may not be showing it; undefined
	 * until the Server has answered for this incarnation.
	 */
	readonly terminalTitle: string | null | undefined;
}

/**
 * The Server's Agent names by Presentation ID, read again every few seconds. Both members are
 * called detached from the source, as `useSyncExternalStore` calls them.
 */
export interface WorkbenchAgentNameSource {
	readonly getSnapshot: () => ReadonlyMap<string, WorkbenchAgentNames>;
	readonly subscribe: (listener: () => void) => () => void;
}
