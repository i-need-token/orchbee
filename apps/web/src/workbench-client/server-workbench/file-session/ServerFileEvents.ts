import type { FileError } from '../../../file-session';
import type { ServerSubscription } from '../../server-connection';
import type { ServerFileChannel } from './ServerFileChannel';

/** What one Events stream makes of the frames its subscription receives. */
export interface ServerFileEventsReader {
	/** The failure a Result reported; null when the record reports none. */
	readonly failure: (record: Uint8Array) => FileError | null;
	/**
	 * Records may have been lost: within the stream, when a replacement snapshot follows, or because
	 * the subscription ended, when nothing it delivered holds any longer.
	 */
	readonly lose: (ended: boolean) => void;
	/** Takes a Snapshot or Update frame; false when its notice cannot be read, which is then absent. */
	readonly notice: (kind: 'snapshot' | 'update', record: Uint8Array) => boolean;
	/** The subscribe request; null when it breaks the contract. */
	readonly request: () => Uint8Array | null;
}

/** One wire subscription, from waiting for room to its end. */
interface EventsAttempt {
	readonly abort: AbortController;
	finished: boolean;
	release: (() => void) | null;
	subscription: ServerSubscription | null;
}

/** Why the stream stopped opening again: for good, or until the service answers again. */
type EventsStop = 'final' | 'until-served';

/** Pauses before the stream opens again after it was lost; the last one repeats. */
const REOPEN_PAUSES_MILLISECONDS = [1000, 2000, 5000, 10_000, 30_000] as const;
const LONGEST_REOPEN_PAUSE_MILLISECONDS = 30_000;

/** What follows a failure: opening again after a pause, or a stop. */
function afterFailure(error: FileError): EventsStop | 'reopen' {
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

/**
 * The life of one Events subscription of a session, whatever its notices say: it opens once asked,
 * holds a stream of the connection's budget while open, and when it is lost opens again after a
 * pause while the service may recover by itself; otherwise it stops until it is asked again or,
 * after the service crashed repeatedly, until the service answers again. The connection's end
 * stops it for good. Its reader keeps what the notices say and tells its followers.
 */
export class ServerFileEvents {
	#attempt: EventsAttempt | null = null;
	readonly #channel: ServerFileChannel;
	#closed = false;
	#ended = false;
	#pause: ReturnType<typeof setTimeout> | null = null;
	#pauses = 0;
	readonly #reader: ServerFileEventsReader;
	#stop: EventsStop | null = null;
	#wanted = false;

	constructor(channel: ServerFileChannel, reader: ServerFileEventsReader) {
		this.#channel = channel;
		this.#reader = reader;
	}

	/** Whether the stream may still deliver, so a new follower is worth taking. */
	get live(): boolean {
		return !this.#closed && !this.#ended;
	}

	/** Opens the stream, and tries a stopped one again. */
	open(): void {
		if (!this.live) return;
		this.#wanted = true;
		this.#stop = null;
		this.#start();
	}

	/** Closes the stream until it is opened again. */
	shut(): void {
		this.#wanted = false;
		this.#halt();
	}

	/** The service answered, so a stream stopped until then opens again. */
	served(): void {
		if (this.#stop !== 'until-served') return;
		this.#stop = null;
		this.#pauses = 0;
		this.#start();
	}

	/** The connection ended: the reader learns that records were lost, and nothing opens again. */
	end(): void {
		if (this.#ended) return;
		this.#ended = true;
		this.#halt();
	}

	close(): void {
		this.#closed = true;
		this.#wanted = false;
		this.#halt();
	}

	#start(): void {
		if (!this.live || !this.#wanted || this.#stop !== null) return;
		if (this.#attempt !== null || this.#pause !== null) return;
		const attempt: EventsAttempt = {
			abort: new AbortController(),
			finished: false,
			release: null,
			subscription: null,
		};
		this.#attempt = attempt;
		void this.#run(attempt);
	}

	async #run(attempt: EventsAttempt): Promise<void> {
		const release = await this.#channel.budget.enter('stream', attempt.abort.signal);
		if (release === null) return;
		if (attempt.finished) {
			release();
			return;
		}
		attempt.release = release;
		const request = this.#reader.request();
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
					if (attempt === this.#attempt && !attempt.finished) this.#reader.lose(false);
				},
			});
		} catch {
			// Only an ended connection refuses the root lane, and its end arrives on its own.
			this.#finish(attempt, 'final');
		}
	}

	#frame(attempt: EventsAttempt, kind: 'result' | 'snapshot' | 'update', record: Uint8Array): void {
		if (attempt !== this.#attempt || attempt.finished) return;
		if (kind === 'result') {
			// The stream failed; the Server closes it next, which no longer concerns this attempt.
			attempt.subscription?.close();
			const error = this.#reader.failure(record) ?? { code: 'failed' };
			this.#finish(attempt, afterFailure(error));
			this.#channel.answered(error);
			return;
		}
		if (!this.#reader.notice(kind, record) || kind !== 'snapshot') return;
		this.#pauses = 0;
		this.#channel.answered(null);
	}

	#closedBy(attempt: EventsAttempt, reason: string): void {
		if (attempt !== this.#attempt || attempt.finished) return;
		// A Server that lacks the stream, refuses its request or is stopping will not serve it later.
		const final = reason === 'invalid-request' || reason === 'stopping' || reason === 'unsupported';
		this.#finish(attempt, final ? 'final' : 'reopen');
	}

	#finish(attempt: EventsAttempt, next: EventsStop | 'reopen'): void {
		attempt.finished = true;
		attempt.release?.();
		attempt.release = null;
		if (attempt !== this.#attempt) return;
		this.#attempt = null;
		this.#reader.lose(true);
		if (!this.live || !this.#wanted) return;
		if (next !== 'reopen') {
			this.#stop = next;
			return;
		}
		const pause = REOPEN_PAUSES_MILLISECONDS[this.#pauses] ?? LONGEST_REOPEN_PAUSE_MILLISECONDS;
		this.#pauses += 1;
		this.#pause = setTimeout(() => {
			this.#pause = null;
			this.#start();
		}, pause);
	}

	/** Ends the attempt and any pause; whatever the attempt delivered no longer holds. */
	#halt(): void {
		if (this.#pause !== null) {
			clearTimeout(this.#pause);
			this.#pause = null;
		}
		const attempt = this.#attempt;
		this.#attempt = null;
		this.#reader.lose(true);
		if (attempt === null) return;
		attempt.finished = true;
		attempt.abort.abort();
		attempt.subscription?.close();
		attempt.release?.();
		attempt.release = null;
	}
}
