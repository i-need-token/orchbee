import type { GitSummary, GitSummaryEvent } from '../../../file-session';
import { notifyEachListener } from '../../contained-client-fault';
import { FILES_SCM_STREAM } from '../../server-connection';
import type { ServerFileChannel } from './ServerFileChannel';
import { ServerFileEvents } from './ServerFileEvents';

interface SummaryFollower {
	readonly listener: (event: GitSummaryEvent) => void;
	/** The follower has had the current summary. */
	primed: boolean;
	/** The sequence of the follower's last record; each follower counts its own. */
	sequence: number;
}

/**
 * A session's `scm.subscribe`, shared by every follower and open while any follows. Every notice
 * carries the whole summary, so a follower that joins later gets the current one first, and a lost
 * stream leaves followers that had one a gap, which the next summary fills.
 */
export class ServerGitSummaryStream {
	readonly #events: ServerFileEvents;
	readonly #followers = new Set<SummaryFollower>();
	#summary: GitSummary | null = null;

	constructor(channel: ServerFileChannel, workspaceId: string) {
		this.#events = new ServerFileEvents(channel, {
			failure: (record) => FILES_SCM_STREAM.decodeFailure(record),
			lose: () => this.#lose(),
			notice: (kind, record) => {
				const notice = FILES_SCM_STREAM.decodeNotice(kind, record);
				if (notice === null) return false;
				this.#take(notice.kind === 'snapshot' ? notice.snapshot : notice.update);
				return true;
			},
			request: () => FILES_SCM_STREAM.encode({ op: 'scm.subscribe', workspaceId }),
		});
	}

	subscribe(listener: (event: GitSummaryEvent) => void): () => void {
		if (!this.#events.live) return () => undefined;
		const follower: SummaryFollower = { listener, primed: false, sequence: 0 };
		this.#followers.add(follower);
		if (this.#summary !== null) queueMicrotask(() => this.#prime(follower));
		this.#events.open();
		return () => {
			if (!this.#followers.delete(follower) || this.#followers.size > 0) return;
			this.#events.shut();
		};
	}

	served(): void {
		this.#events.served();
	}

	end(): void {
		this.#events.end();
	}

	close(): void {
		this.#followers.clear();
		this.#events.close();
	}

	#take(summary: GitSummary): void {
		this.#summary = summary;
		for (const follower of this.#followers) {
			if (follower.primed) this.#send(follower, summary);
			else this.#prime(follower);
		}
	}

	#prime(follower: SummaryFollower): void {
		const summary = this.#summary;
		if (summary === null || follower.primed || !this.#followers.has(follower)) return;
		follower.primed = true;
		this.#send(follower, summary);
	}

	#send(follower: SummaryFollower, summary: GitSummary): void {
		follower.sequence += 1;
		const event: GitSummaryEvent = { kind: 'summary', sequence: follower.sequence, summary };
		notifyEachListener([follower], 'source-listener', (target) => target.listener(event));
	}

	/** Followers that had a summary learn that records were lost; the next summary is whole. */
	#lose(): void {
		if (this.#summary === null) return;
		this.#summary = null;
		const primed = [...this.#followers].filter((follower) => follower.primed);
		for (const follower of this.#followers) follower.primed = false;
		notifyEachListener(primed, 'source-listener', (follower) => follower.listener({ kind: 'gap' }));
	}
}
