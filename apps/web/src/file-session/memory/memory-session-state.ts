import type { FileRoot, FileRootEvent } from '../file-session-port';
import type { GitRelayEvent, GitSummaryEvent } from '../git-port';

/** One subscription to a stream and the last sequence it was sent. */
export interface MemoryStream<Event> {
	readonly listener: (event: Event) => void;
	/** Whether its first records went out on the current link; nothing else is sent before them. */
	ready: boolean;
	sequence: number;
}

/** A root stream subscription, which also remembers the root it last reported. */
export interface MemoryRootStream extends MemoryStream<FileRootEvent> {
	root: FileRoot;
}

/**
 * What one open session holds on the connection: its streams, loaded folders and opened files,
 * which a dropped link releases.
 */
export interface MemorySessionState {
	closed: boolean;
	/** Opened files, by comparison key. */
	readonly granted: Set<string>;
	/** Loaded folders, by comparison key. */
	readonly loaded: Set<string>;
	readonly relayStreams: Set<MemoryStream<GitRelayEvent>>;
	readonly rootStreams: Set<MemoryRootStream>;
	readonly summaryStreams: Set<MemoryStream<GitSummaryEvent>>;
	readonly workspaceId: string;
}
