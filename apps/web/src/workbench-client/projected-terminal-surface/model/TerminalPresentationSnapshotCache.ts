const DEFAULT_MAXIMUM_BYTES = 32 * 1024 * 1024;

/** Application-scoped, byte-bounded LRU storage for display-only terminal snapshots. */
export class TerminalPresentationSnapshotCache<Snapshot extends { readonly byteLength: number }> {
	readonly #entries = new Map<string, Snapshot>();
	readonly #maximumBytes: number;
	#byteLength = 0;

	constructor(maximumBytes = DEFAULT_MAXIMUM_BYTES) {
		if (!Number.isSafeInteger(maximumBytes) || maximumBytes < 0) {
			throw new RangeError('Terminal presentation snapshot budget must be non-negative.');
		}
		this.#maximumBytes = maximumBytes;
	}

	get byteLength(): number {
		return this.#byteLength;
	}

	get size(): number {
		return this.#entries.size;
	}

	get(presentationId: string): Snapshot | undefined {
		const snapshot = this.#entries.get(presentationId);
		if (snapshot === undefined) return undefined;
		this.#entries.delete(presentationId);
		this.#entries.set(presentationId, snapshot);
		return snapshot;
	}

	set(presentationId: string, snapshot: Snapshot): boolean {
		if (!Number.isSafeInteger(snapshot.byteLength) || snapshot.byteLength < 0) return false;
		const previous = this.#entries.get(presentationId);
		if (previous !== undefined) {
			this.#entries.delete(presentationId);
			this.#byteLength -= previous.byteLength;
		}
		if (snapshot.byteLength > this.#maximumBytes) return false;
		this.#entries.set(presentationId, snapshot);
		this.#byteLength += snapshot.byteLength;
		while (this.#byteLength > this.#maximumBytes) {
			const oldest = this.#entries.entries().next().value;
			if (oldest === undefined) break;
			this.#entries.delete(oldest[0]);
			this.#byteLength -= oldest[1].byteLength;
		}
		return this.#entries.has(presentationId);
	}

	clear(): void {
		this.#entries.clear();
		this.#byteLength = 0;
	}
}
