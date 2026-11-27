import type { FileError, FileRootEvent } from '../../../file-session';
import { notifyEachListener } from '../../contained-client-fault';
import {
	FILES_ROOT_STREAM,
	type FilesRootSnapshot,
	type FilesRootUpdate,
	type ServerSubscription,
} from '../../server-connection';
import type { ServerFileChannel } from './ServerFileChannel';

type FileRootSnapshotEvent = Extract<FileRootEvent, { readonly kind: 'snapshot' }>;
type FileRootUpdateEvent = Extract<FileRootEvent, { readonly kind: 'update' }>;

/** What the latest snapshot and the updates since describe. */
interface RootFacts {
	root: FileRootSnapshotEvent['root'];
	readonly rules: FileRootSnapshotEvent['rules'];
	watch: FilesRootSnapshot['watch'];
}

interface RootFollower {
	readonly listener: (event: FileRootEvent) => void;
	/** The follower has had the current snapshot. */
	primed: boolean;
	/** The sequence of the follower's last record; each follower counts its own. */
	sequence: number;
}

/** One wire subscription, from waiting for room to its end. */
interface RootAttempt {
	readonly abort: AbortController;
	finished: boolean;
	release: (() => void) | null;
	subscription: ServerSubscription | null;
}

/** Why the stream stopped opening again: for good, or until the service answers again. */
type RootStop = 'final' | 'until-served';

/** Pauses before the stream opens again after it was lost; the last one repeats. */
const REOPEN_PAUSES_MILLISECONDS = [1000, 2000, 5000, 10_000, 30_000] as const;
const LONGEST_REOPEN_PAUSE_MILLISECONDS = 30_000;

/** What follows a failure: opening again after a pause, or a stop. */
function afterFailure(error: FileError): RootStop | 'reopen' {
	switch (error.code) {
		case 'cancelled':
		case 'timeout':
			return 'reopen';
		case 'unavailable':
			switch (error.reason) {
				case undefined:
				case 'restarting':
				case 'stuck-filesystem':
					return 'reopen';
				// The Server stops restarting the service until the person restarts it.
				case 'crashed-repeatedly':
					return 'until-served';
				default:
					return 'final';
			}
		default:
			return 'final';
	}
}

function updateEvent(update: FilesRootUpdate): Omit<FileRootUpdateEvent, 'kind' | 'sequence'> {
	switch (update.kind) {
		case 'changes':
			return { changes: update.changes, rescan: [], root: null, watch: null };
		case 'rescan':
			return { changes: [], rescan: update.paths, root: null, watch: null };
		case 'root':
			return { changes: [], rescan: [], root: update.root, watch: null };
		case 'watch':
			return { changes: [], rescan: [], root: null, watch: update.watch };
	}
}

/**
 * One session's root stream: a single `fs.subscribe` shared by every follower, open while any
 * follows. A follower that joins later gets the current snapshot first, and every follower counts
 * its own sequence. When the stream is lost, followers that had a snapshot get a gap; it opens
 * again after a pause while the service may recover by itself, and otherwise stops until another
 * follower joins or, after the service crashed repeatedly, until it answers again. The
 * connection's end leaves only the gap.
 */
export class ServerFileRootStream {
	#attempt: RootAttempt | null = null;
	readonly #channel: ServerFileChannel;
	#closed = false;
	#ended = false;
	#facts: RootFacts | null = null;
	readonly #followers = new Set<RootFollower>();
	#pause: ReturnType<typeof setTimeout> | null = null;
	#pauses = 0;
	#stop: RootStop | null = null;
	readonly #workspaceId: string;

	constructor(channel: ServerFileChannel, workspaceId: string) {
		this.#channel = channel;
		this.#workspaceId = workspaceId;
	}

	subscribe(listener: (event: FileRootEvent) => void): () => void {
		if (this.#closed || this.#ended) return () => undefined;
		const follower: RootFollower = { listener, primed: false, sequence: 0 };
		this.#followers.add(follower);
		if (this.#facts !== null) queueMicrotask(() => this.#prime(follower));
		// A new follower tries a stopped stream again.
		this.#stop = null;
		this.#open();
		return () => {
			if (!this.#followers.delete(follower) || this.#followers.size > 0) return;
			this.#shut();
		};
	}

	/** The service answered, so a stream stopped until then opens again. */
	served(): void {
		if (this.#stop !== 'until-served') return;
		this.#stop = null;
		this.#pauses = 0;
		this.#open();
	}

	/** The connection ended: followers learn that records were lost, and nothing opens again. */
	end(): void {
		if (this.#ended) return;
		this.#ended = true;
		this.#lose();
		this.#shut();
	}

	close(): void {
		this.#closed = true;
		this.#followers.clear();
		this.#shut();
	}

	#open(): void {
		if (this.#closed || this.#ended || this.#stop !== null || this.#followers.size === 0) return;
		if (this.#attempt !== null || this.#pause !== null) return;
		const attempt: RootAttempt = {
			abort: new AbortController(),
			finished: false,
			release: null,
			subscription: null,
		};
		this.#attempt = attempt;
		void this.#run(attempt);
	}

	async #run(attempt: RootAttempt): Promise<void> {
		const release = await this.#channel.budget.enter('stream', attempt.abort.signal);
		if (release === null) return;
		if (attempt.finished) {
			release();
			return;
		}
		attempt.release = release;
		const request = FILES_ROOT_STREAM.encode({
			op: 'fs.subscribe',
			workspaceId: this.#workspaceId,
		});
		if (request === null) {
			this.#finish(attempt, 'final');
			return;
		}
		try {
			attempt.subscription = this.#channel.connection.subscribe('events', request, {
				onClosed: (reason) => this.#closedBy(attempt, reason),
				onFrame: (kind, record) => this.#frame(attempt, kind, record),
				onGap: () => {
					// A replacement snapshot follows the gap.
					if (attempt === this.#attempt && !attempt.finished) this.#lose();
				},
			});
		} catch {
			// Only an ended connection refuses the root lane, and its end arrives on its own.
			this.#finish(attempt, 'final');
		}
	}

	#frame(attempt: RootAttempt, kind: 'result' | 'snapshot' | 'update', record: Uint8Array): void {
		if (attempt !== this.#attempt || attempt.finished) return;
		if (kind === 'result') {
			// The stream failed; the Server closes it next, which no longer concerns this attempt.
			attempt.subscription?.close();
			const error = FILES_ROOT_STREAM.decodeFailure(record) ?? { code: 'failed' };
			this.#finish(attempt, afterFailure(error));
			this.#channel.answered(error);
			return;
		}
		const notice = FILES_ROOT_STREAM.decodeNotice(kind, record);
		// A notice this Client cannot read is absent.
		if (notice === null) return;
		if (notice.kind === 'snapshot') this.#snapshot(notice.snapshot);
		else this.#update(notice.update);
	}

	#closedBy(attempt: RootAttempt, reason: string): void {
		if (attempt !== this.#attempt || attempt.finished) return;
		// A Server that lacks the stream, refuses its request or is stopping will not serve it later.
		const final = reason === 'invalid-request' || reason === 'stopping' || reason === 'unsupported';
		this.#finish(attempt, final ? 'final' : 'reopen');
	}

	#finish(attempt: RootAttempt, next: RootStop | 'reopen'): void {
		attempt.finished = true;
		attempt.release?.();
		attempt.release = null;
		if (attempt !== this.#attempt) return;
		this.#attempt = null;
		this.#lose();
		if (this.#closed || this.#ended || this.#followers.size === 0) return;
		if (next !== 'reopen') {
			this.#stop = next;
			return;
		}
		const pause = REOPEN_PAUSES_MILLISECONDS[this.#pauses] ?? LONGEST_REOPEN_PAUSE_MILLISECONDS;
		this.#pauses += 1;
		this.#pause = setTimeout(() => {
			this.#pause = null;
			this.#open();
		}, pause);
	}

	#snapshot(snapshot: FilesRootSnapshot): void {
		this.#pauses = 0;
		this.#facts = {
			root: snapshot.root,
			rules: {
				caseSensitive: snapshot.caseSensitive,
				os: snapshot.os,
				separator: snapshot.separator,
			},
			watch: snapshot.watch,
		};
		// A snapshot starts every follower over.
		for (const follower of this.#followers) follower.primed = false;
		for (const follower of this.#followers) this.#prime(follower);
		this.#channel.answered(null);
	}

	#update(update: FilesRootUpdate): void {
		const facts = this.#facts;
		if (facts === null) return;
		// A follower that has not had the snapshot gets it first, as it stood before this update.
		for (const follower of this.#followers) this.#prime(follower);
		if (update.kind === 'root') facts.root = update.root;
		if (update.kind === 'watch') facts.watch = update.watch;
		const event = updateEvent(update);
		for (const follower of this.#followers) {
			if (follower.primed) {
				this.#send(follower, (sequence) => ({ ...event, kind: 'update', sequence }));
			}
		}
	}

	/** Gives a follower that lacks it the current snapshot, then the watch state unless full. */
	#prime(follower: RootFollower): void {
		const facts = this.#facts;
		if (facts === null || follower.primed || !this.#followers.has(follower)) return;
		follower.primed = true;
		const { root, rules, watch } = facts;
		this.#send(follower, (sequence) => ({ kind: 'snapshot', root, rules, sequence }));
		if (watch === 'full') return;
		this.#send(follower, (sequence) => ({
			changes: [],
			kind: 'update',
			rescan: [],
			root: null,
			sequence,
			watch,
		}));
	}

	/** Followers that had a snapshot learn that records were lost; a new snapshot follows. */
	#lose(): void {
		if (this.#facts === null) return;
		this.#facts = null;
		const primed = [...this.#followers].filter((follower) => follower.primed);
		for (const follower of this.#followers) follower.primed = false;
		notifyEachListener(primed, 'source-listener', (follower) => follower.listener({ kind: 'gap' }));
	}

	#send(follower: RootFollower, event: (sequence: number) => FileRootEvent): void {
		follower.sequence += 1;
		const record = event(follower.sequence);
		notifyEachListener([follower], 'source-listener', (target) => target.listener(record));
	}

	#shut(): void {
		if (this.#pause !== null) {
			clearTimeout(this.#pause);
			this.#pause = null;
		}
		this.#facts = null;
		const attempt = this.#attempt;
		this.#attempt = null;
		if (attempt === null) return;
		attempt.finished = true;
		attempt.abort.abort();
		attempt.subscription?.close();
		attempt.release?.();
		attempt.release = null;
	}
}
