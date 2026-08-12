interface InputDispatchGeneration {
	readonly controller: AbortController;
	pending: number;
	tail: Promise<void>;
}

function createInputDispatchGeneration(): InputDispatchGeneration {
	return {
		controller: new AbortController(),
		pending: 0,
		tail: Promise.resolve(),
	};
}

/** Serializes ordinary input while allowing lifecycle repair to invalidate a stalled generation. */
export class OrderedInputDispatcher {
	private generation = createInputDispatchGeneration();
	private readonly onDeferredError: (error: unknown) => void;

	constructor(onDeferredError: (error: unknown) => void = () => {}) {
		this.onDeferredError = onDeferredError;
	}

	get isPending(): boolean {
		return this.generation.pending > 0;
	}

	dispatch<Result>(
		operation: (signal: AbortSignal) => Result | Promise<Result>,
		cancelled: () => Result | Promise<Result>,
	): Promise<Result> {
		const generation = this.generation;
		generation.pending++;
		const result = generation.tail.then(async () => {
			const signal = generation.controller.signal;
			if (signal.aborted) return await cancelled();
			return await this.runCancellable(signal, operation, cancelled);
		});
		generation.tail = result.then(
			() => {
				generation.pending--;
			},
			() => {
				generation.pending--;
			},
		);
		return result;
	}

	dispatchSync(operation: () => void): void {
		if (!this.isPending) {
			operation();
			return;
		}
		void this.dispatch(
			(signal) => {
				if (!signal.aborted) operation();
			},
			() => undefined,
		).catch(this.onDeferredError);
	}

	/** Cancels and detaches all work admitted to the current generation. */
	invalidate(): void {
		this.generation.controller.abort();
		this.generation = createInputDispatchGeneration();
	}

	private async runCancellable<Result>(
		signal: AbortSignal,
		operation: (signal: AbortSignal) => Result | Promise<Result>,
		cancelled: () => Result | Promise<Result>,
	): Promise<Result> {
		let handleAbort = (): void => undefined;
		const cancellation = new Promise<Result>((resolve, reject) => {
			handleAbort = () => {
				Promise.resolve().then(cancelled).then(resolve, reject);
			};
			signal.addEventListener('abort', handleAbort, { once: true });
		});
		let result: Promise<Result>;
		try {
			result = Promise.resolve(operation(signal));
		} catch (error) {
			result = Promise.reject(error);
		}
		try {
			return await Promise.race([result, cancellation]);
		} finally {
			signal.removeEventListener('abort', handleAbort);
		}
	}
}
