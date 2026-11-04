import type { WorkbenchWorkspaceLocation, WorkbenchWorkspaceLocationSource } from '../../workbench';
import {
	WORKBENCH_PROTOCOL_VERSION,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import type { ServerWorkbenchSession } from './ServerWorkbenchSession';
import {
	acceptWorkspaceGit,
	followWorkspaceSnapshot,
	nextHoldDeadline,
	NO_WORKSPACE_LOCATIONS,
	presentWorkspaceLocations,
	sameWorkspaceLocations,
	withdrawWorkspaceGit,
	workspaceGitSources,
	type WorkspaceGitReading,
	type WorkspaceGitSource,
	type WorkspaceLocationState,
} from './workspace-locations';

type LocationSession = Pick<ServerWorkbenchSession, 'ask' | 'snapshot' | 'subscribe'>;
type WorkspaceId = WorkbenchSnapshot['navigation']['workspaces'][number]['workspaceId'];
type Timer = ReturnType<typeof setTimeout>;

// The TUI's cadence (`machine_observation.rs`), so Git costs every Client the same share of the carrier.

/** Facts stay shown this long after the last read that reached the Server, through failed refreshes. */
const FRESH_MILLISECONDS = 10_000;
/**
 * A round pauses three times as long as it took, from two to four seconds: Git reads take at most a
 * quarter of the carrier, and the next five-second read completes before the facts age out.
 */
const SAMPLE_DUTY = 3;
const MINIMUM_SAMPLE_PAUSE_MILLISECONDS = 2_000;
const MAXIMUM_SAMPLE_PAUSE_MILLISECONDS = 4_000;
/** One read's budget, across every Workspace it asks. */
const READ_MILLISECONDS = 5_000;

/** A failed read retries after half a second, doubling up to thirty. */
function retryAfter(failures: number): number {
	return Math.min(500 * 2 ** Math.min(failures, 6), 30_000);
}

export interface ServerWorkspaceLocations extends WorkbenchWorkspaceLocationSource {
	dispose(): void;
}

export interface ServerWorkspaceLocationsOptions {
	/** The person's home folder where the Server runs; a folder under it shows as `~`. */
	readonly homeDirectory: string | null;
	readonly session: LocationSession;
}

/** What one stretch of observation owns; a read that outlives it changes nothing. */
interface Watch {
	demand: Set<WorkspaceId>;
	/** Each Workspace and directory gets one read ahead of sampling; a repeated failure waits for it. */
	demanded: Set<string>;
	failures: number;
	freshTimer: Timer | null;
	holdTimer: Timer | null;
	reading: boolean;
	sampleDue: boolean;
	sampleTimer: Timer | null;
	stopSnapshots: () => void;
}

function demandKey({ source, workspaceId }: WorkspaceGitSource): string {
	return `${workspaceId}\u0000${source}`;
}

/** One Workspace's Git facts; null when the read itself failed and the round must stop. */
async function askGit(
	session: LocationSession,
	workspaceId: WorkspaceId,
	milliseconds: number,
): Promise<WorkspaceGitReading | null> {
	const requestId = createWorkbenchRequestId();
	const answer = await session.ask(
		{
			kind: 'workbench.workspace-git',
			protocolVersion: WORKBENCH_PROTOCOL_VERSION,
			requestId,
			sessionId: session.snapshot.sessionId,
			workspaceId,
		},
		milliseconds,
	);
	if (
		answer.kind !== 'answered' ||
		answer.response.kind !== 'workbench.workspace-git.result' ||
		answer.response.requestId !== requestId
	) {
		return null;
	}
	const { outcome } = answer.response;
	if (outcome.kind === 'observed') {
		return {
			git: { branch: outcome.branch, divergence: outcome.divergence },
			kind: 'observed',
			revision: outcome.workbenchRevision,
		};
	}
	switch (outcome.reason) {
		case 'malformed':
		case 'timeout':
		case 'unavailable':
			return { kind: 'unanswered' };
		case 'denied':
		case 'not-found':
		case 'source-changed':
			return null;
	}
}

/**
 * The branch, folder and divergence of a Server Session's Workspaces, read only while someone
 * watches. Rounds ask Git about every Workspace at the TUI's cadence, and a Workspace whose directory
 * changed gets one read of its own as soon as no read runs.
 */
export function createServerWorkspaceLocations({
	homeDirectory,
	session,
}: ServerWorkspaceLocationsOptions): ServerWorkspaceLocations {
	const listeners = new Set<() => void>();
	let disposed = false;
	let presented: ReadonlyMap<string, WorkbenchWorkspaceLocation> = new Map();
	let state: WorkspaceLocationState = NO_WORKSPACE_LOCATIONS;
	let watch: Watch | null = null;

	function present(current: Watch): void {
		const now = performance.now();
		const next = presentWorkspaceLocations(state, now, homeDirectory);
		if (!sameWorkspaceLocations(next, presented)) {
			presented = next;
			for (const listener of Array.from(listeners)) listener();
		}
		if (current.holdTimer !== null) clearTimeout(current.holdTimer);
		const deadline = nextHoldDeadline(state, now);
		current.holdTimer =
			deadline === null
				? null
				: setTimeout(() => {
						if (watch === current) present(current);
					}, deadline - now);
	}

	function change(current: Watch, next: WorkspaceLocationState): void {
		state = next;
		const unanswered = new Set<string>();
		for (const source of workspaceGitSources(state)) {
			if (source.answered) continue;
			const key = demandKey(source);
			unanswered.add(key);
			if (!current.demanded.has(key)) current.demand.add(source.workspaceId);
		}
		current.demanded = unanswered;
		present(current);
		pump(current);
	}

	/** Marks every Workspace as asked, so only sampling reads it next. */
	function leaveToSampling(current: Watch): void {
		current.demand.clear();
		for (const source of workspaceGitSources(state)) current.demanded.add(demandKey(source));
	}

	function reached(current: Watch): void {
		if (current.freshTimer !== null) clearTimeout(current.freshTimer);
		current.freshTimer = setTimeout(() => {
			if (watch !== current) return;
			current.freshTimer = null;
			state = withdrawWorkspaceGit(state);
			// Reads are failing; the next round, not a read per Workspace, finds out when they recover.
			leaveToSampling(current);
			change(current, state);
		}, FRESH_MILLISECONDS);
	}

	function schedule(current: Watch, milliseconds: number): void {
		if (current.sampleTimer !== null) clearTimeout(current.sampleTimer);
		current.sampleTimer = setTimeout(() => {
			if (watch !== current) return;
			current.sampleTimer = null;
			current.sampleDue = true;
			pump(current);
		}, milliseconds);
	}

	async function read(current: Watch, only: ReadonlySet<WorkspaceId> | null): Promise<void> {
		current.reading = true;
		const round = only === null ? 'complete' : 'partial';
		const started = performance.now();
		const deadline = started + READ_MILLISECONDS;
		const sources = workspaceGitSources(state).filter(
			(source) => only === null || (only.has(source.workspaceId) && !source.answered),
		);
		if (only === null) leaveToSampling(current);
		const unanswered: WorkspaceId[] = [];
		let complete = true;
		for (const { workspaceId } of sources) {
			const remaining = deadline - performance.now();
			const reading = remaining <= 0 ? null : await askGit(session, workspaceId, remaining);
			if (watch !== current) return;
			if (reading === null) {
				complete = false;
				break;
			}
			if (reading.kind === 'unanswered') unanswered.push(workspaceId);
			else change(current, acceptWorkspaceGit(state, workspaceId, reading, round));
		}
		current.reading = false;
		let next = state;
		if (complete) {
			current.failures = 0;
			reached(current);
			for (const workspaceId of unanswered) {
				next = acceptWorkspaceGit(next, workspaceId, { kind: 'unanswered' }, round);
			}
		}
		// Only a round sets the pause; any failed read sets the retry.
		if (!complete) schedule(current, retryAfter(current.failures++));
		else if (only === null) {
			schedule(
				current,
				Math.min(
					Math.max((performance.now() - started) * SAMPLE_DUTY, MINIMUM_SAMPLE_PAUSE_MILLISECONDS),
					MAXIMUM_SAMPLE_PAUSE_MILLISECONDS,
				),
			);
		}
		change(current, next);
	}

	/** Starts the next read once none runs: a due round first, since it covers every Workspace. */
	function pump(current: Watch): void {
		if (current.reading) return;
		if (current.sampleDue) {
			current.sampleDue = false;
			void read(current, null);
		} else if (current.demand.size > 0) {
			const only = current.demand;
			current.demand = new Set();
			void read(current, only);
		}
	}

	function start(): void {
		const current: Watch = {
			demand: new Set(),
			demanded: new Set(),
			failures: 0,
			freshTimer: null,
			holdTimer: null,
			reading: false,
			sampleDue: true,
			sampleTimer: null,
			stopSnapshots: () => undefined,
		};
		watch = current;
		current.stopSnapshots = session.subscribe((snapshot) => {
			if (watch === current) {
				change(current, followWorkspaceSnapshot(state, snapshot, performance.now()));
			}
		});
		// Facts kept from an earlier watch stay until this one's first round, or until they age out.
		reached(current);
		change(current, followWorkspaceSnapshot(state, session.snapshot, performance.now()));
	}

	function stop(): void {
		const current = watch;
		if (current === null) return;
		watch = null;
		current.stopSnapshots();
		for (const timer of [current.freshTimer, current.holdTimer, current.sampleTimer]) {
			if (timer !== null) clearTimeout(timer);
		}
	}

	return {
		dispose() {
			disposed = true;
			listeners.clear();
			stop();
		},
		getSnapshot: () => presented,
		subscribe(listener) {
			if (disposed) return () => undefined;
			listeners.add(listener);
			if (listeners.size === 1) start();
			return () => {
				if (listeners.delete(listener) && listeners.size === 0) stop();
			};
		},
	};
}
