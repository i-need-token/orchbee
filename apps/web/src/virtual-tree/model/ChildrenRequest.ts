interface ChildrenRequestOptions<T> {
	readonly controller: AbortController;
	/** The source's answer. */
	readonly result: Promise<readonly T[]>;
	readonly startedAt: number;
	readonly recursive: boolean;
	/** Applies the answer unless the request no longer stands, and resolves once its loads settle. */
	readonly answer: (request: ChildrenRequest<T>, items: readonly T[]) => Promise<void> | null;
	/** Handles a failure unless the request no longer stands; throwing rejects the callers. */
	readonly fail: (request: ChildrenRequest<T>, error: unknown) => void;
}

/**
 * One fetch of a node's children that has not ended. Callers whose refresh it is new enough for
 * share it, and callers that need a newer answer share one follow-up fetch that starts once it ends.
 * An abort settles its callers at once, however late the source answers, or whether it does.
 */
export class ChildrenRequest<T> {
	/**
	 * The refresh count when it started: its answer covers every refresh asked until then, and the
	 * loads the answer starts below must be at least as new.
	 */
	readonly startedAt: number;
	/** Some caller asked for a recursive refresh, so the answer applies as one. */
	recursive: boolean;
	/** The newest refresh among the callers this answer is too old for. */
	followUpSince = 0;
	followUpRecursive = false;
	/** The fetch those callers share; null while none waits. */
	followUp: Promise<void> | null = null;
	/** Settles once the answer has applied and the loads it started have settled, or on abort. */
	readonly settled: Promise<void>;
	/** Settles once the request has ended: answered, failed or aborted. */
	readonly ended: Promise<void>;
	private readonly controller: AbortController;
	private end: (() => void) | undefined;
	private timer: ReturnType<typeof setTimeout> | undefined;

	constructor(options: ChildrenRequestOptions<T>) {
		this.controller = options.controller;
		this.startedAt = options.startedAt;
		this.recursive = options.recursive;
		this.ended = new Promise((resolve) => {
			this.end = resolve;
		});
		const aborted = new Promise<void>((resolve) => {
			options.controller.signal.addEventListener('abort', () => {
				resolve();
			});
		});
		const answered = options.result.then(
			(items) => options.answer(this, items) ?? undefined,
			(error: unknown) => {
				options.fail(this, error);
			},
		);
		this.settled = Promise.race([answered, aborted]);
	}

	/** Runs `callback` after `delay` milliseconds unless the request has ended by then. */
	notifyAfter(delay: number, callback: () => void): void {
		this.timer = setTimeout(callback, delay);
	}

	/** Ends the request once its answer has arrived. */
	close(): void {
		clearTimeout(this.timer);
		this.end?.();
	}

	/** Tells the source that nobody wants the answer any more, and ends the request. */
	abort(): void {
		this.controller.abort();
		this.close();
	}
}
