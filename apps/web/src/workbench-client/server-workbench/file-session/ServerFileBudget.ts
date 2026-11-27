/** File Bar streams one connection holds at once, by the backend's bound. */
const MAXIMUM_STREAMS = 24;
/** Reads one connection runs at once; each holds a lane, which terminals need too. */
const MAXIMUM_TRANSFERS = 8;

/** A stream that carries a chunk, any other File Bar stream, or a read's transfer and its lane. */
type ServerFileNeed = 'chunk' | 'stream' | 'transfer';

interface Waiter {
	readonly grant: (release: () => void) => void;
	readonly need: ServerFileNeed;
}

/**
 * What one connection's File Bar may hold at once: 24 streams, of which only so many carry chunks,
 * and 8 reads. Waiters are served in arrival order; one whose need does not fit yet lets the next
 * that fits go ahead, so a read waiting for chunk room never holds back a listing.
 */
export class ServerFileBudget {
	#chunks = 0;
	readonly #maximumChunks: number;
	#streams = 0;
	#transfers = 0;
	readonly #waiting: Waiter[] = [];

	constructor(maximumChunks: number) {
		this.#maximumChunks = maximumChunks;
	}

	/**
	 * Waits until `need` fits and takes it; null when `signal` aborts first. Calling the release
	 * returns it, and calling it again does nothing.
	 */
	enter(need: ServerFileNeed, signal?: AbortSignal): Promise<(() => void) | null> {
		if (signal?.aborted === true) return Promise.resolve(null);
		return new Promise((resolve) => {
			const abort = (): void => {
				const index = this.#waiting.indexOf(waiter);
				if (index >= 0) this.#waiting.splice(index, 1);
				resolve(null);
			};
			const waiter: Waiter = {
				grant: (release) => {
					signal?.removeEventListener('abort', abort);
					resolve(release);
				},
				need,
			};
			signal?.addEventListener('abort', abort, { once: true });
			this.#waiting.push(waiter);
			this.#admit();
		});
	}

	#admit(): void {
		let index = 0;
		while (index < this.#waiting.length) {
			const waiter = this.#waiting[index];
			if (waiter === undefined) return;
			if (!this.#fits(waiter.need)) {
				index += 1;
				continue;
			}
			this.#waiting.splice(index, 1);
			waiter.grant(this.#take(waiter.need));
		}
	}

	#fits(need: ServerFileNeed): boolean {
		switch (need) {
			case 'chunk':
				return this.#streams < MAXIMUM_STREAMS && this.#chunks < this.#maximumChunks;
			case 'stream':
				return this.#streams < MAXIMUM_STREAMS;
			case 'transfer':
				return this.#transfers < MAXIMUM_TRANSFERS;
		}
	}

	#take(need: ServerFileNeed): () => void {
		this.#count(need, 1);
		let released = false;
		return () => {
			if (released) return;
			released = true;
			this.#count(need, -1);
			this.#admit();
		};
	}

	#count(need: ServerFileNeed, change: 1 | -1): void {
		if (need === 'transfer') {
			this.#transfers += change;
			return;
		}
		this.#streams += change;
		if (need === 'chunk') this.#chunks += change;
	}
}
