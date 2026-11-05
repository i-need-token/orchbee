/** One command the person's configuration installs, which a Client runs by its identity. */
export interface WorkbenchConfiguredCommand {
	/** The configured description; null when the configuration gives none. */
	readonly description: string | null;
	/** The command line or plugin target the TUI's key help shows; null when nothing visible remains. */
	readonly display: string | null;
	/** Opaque, and different once anything in the configured commands changes. */
	readonly id: string;
}

/** Why a command was refused before it ran, in the reasons the TUI names. */
export type WorkbenchConfiguredCommandRefusal =
	| 'busy'
	| 'conflict'
	| 'denied'
	| 'invalid-intent'
	| 'not-found'
	| 'session-mismatch'
	| 'stale-source'
	| 'unavailable';

/**
 * How a command ended for this Client: `accepted` once it started, `failed` when it or the focus on
 * the Pane it opened did not, and `unknown` when it may or may not have run.
 */
export type WorkbenchConfiguredCommandOutcome =
	| { readonly kind: 'accepted' | 'failed' | 'unknown' }
	| { readonly kind: 'refused'; readonly reason: WorkbenchConfiguredCommandRefusal };

/** The configured commands this Client can run, and their invocation. */
export interface WorkbenchConfiguredCommands {
	/** Holds its identity until the commands change. */
	getSnapshot(): readonly WorkbenchConfiguredCommand[];
	subscribe(listener: () => void): () => void;
	/**
	 * Runs a command from what the window shows now, one at a time: another asked meanwhile is refused
	 * as `busy`. A command is never repeated.
	 */
	run(commandId: string): Promise<WorkbenchConfiguredCommandOutcome>;
}
