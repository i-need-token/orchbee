import type { FileRootEvent } from '../../../file-session';
import { notifyEachListener } from '../../contained-client-fault';
import {
	FILES_ROOT_STREAM,
	type FilesRootSnapshot,
	type FilesRootUpdate,
} from '../../server-connection';
import type { ServerFileChannel } from './ServerFileChannel';
import { ServerFileEvents } from './ServerFileEvents';

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
 * follows, with the life `ServerFileEvents` gives it. A follower that joins later gets the current
 * snapshot first, and every follower counts its own sequence. When the stream is lost, followers
 * that had a snapshot get a gap, and a new snapshot follows once it opens again.
 */
export class ServerFileRootStream {
	readonly #events: ServerFileEvents;
	#facts: RootFacts | null = null;
	readonly #followers = new Set<RootFollower>();

	constructor(channel: ServerFileChannel, workspaceId: string) {
		this.#events = new ServerFileEvents(channel, {
			failure: (record) => FILES_ROOT_STREAM.decodeFailure(record),
			lose: () => this.#lose(),
			notice: (kind, record) => {
				const notice = FILES_ROOT_STREAM.decodeNotice(kind, record);
				if (notice === null) return false;
				if (notice.kind === 'snapshot') this.#snapshot(notice.snapshot);
				else this.#update(notice.update);
				return true;
			},
			request: () => FILES_ROOT_STREAM.encode({ op: 'fs.subscribe', workspaceId }),
		});
	}

	subscribe(listener: (event: FileRootEvent) => void): () => void {
		if (!this.#events.live) return () => undefined;
		const follower: RootFollower = { listener, primed: false, sequence: 0 };
		this.#followers.add(follower);
		if (this.#facts !== null) queueMicrotask(() => this.#prime(follower));
		// A new follower tries a stopped stream again.
		this.#events.open();
		return () => {
			if (!this.#followers.delete(follower) || this.#followers.size > 0) return;
			this.#events.shut();
		};
	}

	/** The service answered, so a stream stopped until then opens again. */
	served(): void {
		this.#events.served();
	}

	/** The connection ended: followers learn that records were lost, and nothing opens again. */
	end(): void {
		this.#events.end();
	}

	close(): void {
		this.#followers.clear();
		this.#events.close();
	}

	#snapshot(snapshot: FilesRootSnapshot): void {
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
}
