import {
	SemanticCommandCoordinatorStatusSchema,
	SemanticCommandExecutionCancellationResultSchema,
	SemanticCommandExecutionLookupResultSchema,
	SemanticCommandExecutionReferenceSchema,
	SemanticCommandInvocationRequestSchema,
	SemanticCommandInvocationResultSchema,
	SemanticCommandOperationDescriptionResultSchema,
	SemanticCommandOperationIdSchema,
	SemanticCommandOperationListSchema,
	type SemanticCommandCoordinatorGeneration,
	type SemanticCommandCoordinatorStatus,
	type SemanticCommandExecutionCancellationResult,
	type SemanticCommandExecutionLookupResult,
	type SemanticCommandExecutionReference,
	type SemanticCommandInvocationRequest,
	type SemanticCommandInvocationResult,
	type SemanticCommandOperationDescriptionResult,
	type SemanticCommandOperationId,
	type SemanticCommandOperationList,
} from '@orchbee/semantic-command-protocol';

interface SemanticCommandClientResponseSchema<Value> {
	safeParse(
		candidate: unknown,
	): { readonly data: Value; readonly success: true } | { readonly success: false };
}

export interface SemanticCommandClientPort {
	cancelExecution(reference: SemanticCommandExecutionReference): Promise<unknown>;
	describeOperation(operationId: SemanticCommandOperationId): Promise<unknown>;
	getExecution(reference: SemanticCommandExecutionReference): Promise<unknown>;
	invoke(request: SemanticCommandInvocationRequest): Promise<unknown>;
	listOperations(): Promise<unknown>;
	readStatus(): Promise<unknown>;
}

type SemanticCommandClientResponse =
	| SemanticCommandCoordinatorStatus
	| SemanticCommandExecutionCancellationResult
	| SemanticCommandExecutionLookupResult
	| SemanticCommandInvocationResult
	| SemanticCommandOperationDescriptionResult
	| SemanticCommandOperationList;

export type SemanticCommandClientErrorCode =
	| 'coordinator-generation-mismatch'
	| 'invalid-request'
	| 'invalid-response'
	| 'observation-cancelled'
	| 'transport-unavailable';

export type SemanticCommandClientResult<Value> =
	| { readonly code: SemanticCommandClientErrorCode; readonly kind: 'error' }
	| { readonly kind: 'ok'; readonly value: Value };

export interface SemanticCommandClientWaitOptions {
	readonly initialDelayMilliseconds?: number;
	readonly maximumDelayMilliseconds?: number;
	readonly signal?: AbortSignal;
}

export interface SemanticCommandClientOptions {
	readonly coordinatorGeneration: SemanticCommandCoordinatorGeneration;
	readonly delay?: (milliseconds: number, signal: AbortSignal | undefined) => Promise<boolean>;
	readonly port: SemanticCommandClientPort;
}

function requirePositiveInteger(value: number, field: string): number {
	if (!Number.isSafeInteger(value) || value <= 0) {
		throw new Error(`${field} must be a positive safe integer.`);
	}
	return value;
}

function defaultDelay(milliseconds: number, signal: AbortSignal | undefined): Promise<boolean> {
	if (signal?.aborted === true) return Promise.resolve(false);
	return new Promise((resolve) => {
		let settled = false;
		const finish = (completed: boolean): void => {
			if (settled) return;
			settled = true;
			clearTimeout(timer);
			signal?.removeEventListener('abort', handleAbort);
			resolve(completed);
		};
		const handleAbort = (): void => finish(false);
		const timer = setTimeout(() => finish(true), milliseconds);
		signal?.addEventListener('abort', handleAbort, { once: true });
	});
}

function responseGeneration(
	value: SemanticCommandClientResponse,
): SemanticCommandCoordinatorGeneration {
	if ('coordinatorGeneration' in value) return value.coordinatorGeneration;
	return value.execution.coordinatorGeneration;
}

export class SemanticCommandClient {
	readonly #coordinatorGeneration: SemanticCommandCoordinatorGeneration;
	readonly #delay: (milliseconds: number, signal: AbortSignal | undefined) => Promise<boolean>;
	readonly #port: SemanticCommandClientPort;

	constructor(options: SemanticCommandClientOptions) {
		this.#coordinatorGeneration = options.coordinatorGeneration;
		this.#delay = options.delay ?? defaultDelay;
		this.#port = options.port;
	}

	cancelExecution(
		reference: SemanticCommandExecutionReference,
	): Promise<SemanticCommandClientResult<SemanticCommandExecutionCancellationResult>> {
		const parsedReference = SemanticCommandExecutionReferenceSchema.safeParse(reference);
		if (!parsedReference.success)
			return Promise.resolve({ kind: 'error', code: 'invalid-request' });
		return this.#read(
			() => this.#port.cancelExecution(parsedReference.data),
			SemanticCommandExecutionCancellationResultSchema,
		);
	}

	describeOperation(
		operationId: SemanticCommandOperationId,
	): Promise<SemanticCommandClientResult<SemanticCommandOperationDescriptionResult>> {
		const parsedOperationId = SemanticCommandOperationIdSchema.safeParse(operationId);
		if (!parsedOperationId.success)
			return Promise.resolve({ kind: 'error', code: 'invalid-request' });
		return this.#read(
			() => this.#port.describeOperation(parsedOperationId.data),
			SemanticCommandOperationDescriptionResultSchema,
		);
	}

	getExecution(
		reference: SemanticCommandExecutionReference,
	): Promise<SemanticCommandClientResult<SemanticCommandExecutionLookupResult>> {
		const parsedReference = SemanticCommandExecutionReferenceSchema.safeParse(reference);
		if (!parsedReference.success)
			return Promise.resolve({ kind: 'error', code: 'invalid-request' });
		return this.#read(
			() => this.#port.getExecution(parsedReference.data),
			SemanticCommandExecutionLookupResultSchema,
		);
	}

	invoke(
		request: SemanticCommandInvocationRequest,
	): Promise<SemanticCommandClientResult<SemanticCommandInvocationResult>> {
		const parsedRequest = SemanticCommandInvocationRequestSchema.safeParse(request);
		if (!parsedRequest.success) return Promise.resolve({ kind: 'error', code: 'invalid-request' });
		return this.#read(
			() => this.#port.invoke(parsedRequest.data),
			SemanticCommandInvocationResultSchema,
		);
	}

	listOperations(): Promise<SemanticCommandClientResult<SemanticCommandOperationList>> {
		return this.#read(() => this.#port.listOperations(), SemanticCommandOperationListSchema);
	}

	readStatus(): Promise<SemanticCommandClientResult<SemanticCommandCoordinatorStatus>> {
		return this.#read(() => this.#port.readStatus(), SemanticCommandCoordinatorStatusSchema);
	}

	async waitForExecution(
		reference: SemanticCommandExecutionReference,
		options: SemanticCommandClientWaitOptions = {},
	): Promise<SemanticCommandClientResult<SemanticCommandExecutionLookupResult>> {
		let delayMilliseconds = requirePositiveInteger(
			options.initialDelayMilliseconds ?? 25,
			'initialDelayMilliseconds',
		);
		const maximumDelayMilliseconds = requirePositiveInteger(
			options.maximumDelayMilliseconds ?? 1_000,
			'maximumDelayMilliseconds',
		);
		if (delayMilliseconds > maximumDelayMilliseconds) {
			throw new Error('initialDelayMilliseconds cannot exceed maximumDelayMilliseconds.');
		}

		while (true) {
			if (options.signal?.aborted === true) {
				return { kind: 'error', code: 'observation-cancelled' };
			}
			const observed = await this.getExecution(reference);
			if (observed.kind === 'error') return observed;
			if (
				observed.value.kind === 'execution-receipt-unavailable' ||
				observed.value.execution.phase === 'terminal'
			) {
				return observed;
			}
			if (!(await this.#delay(delayMilliseconds, options.signal))) {
				return { kind: 'error', code: 'observation-cancelled' };
			}
			delayMilliseconds = Math.min(maximumDelayMilliseconds, delayMilliseconds * 2);
		}
	}

	async #read<Value extends SemanticCommandClientResponse>(
		read: () => Promise<unknown>,
		schema: SemanticCommandClientResponseSchema<Value>,
	): Promise<SemanticCommandClientResult<Value>> {
		let candidate: unknown;
		try {
			candidate = await read();
		} catch {
			return { kind: 'error', code: 'transport-unavailable' };
		}
		const parsed = schema.safeParse(candidate);
		if (!parsed.success) return { kind: 'error', code: 'invalid-response' };
		if (responseGeneration(parsed.data) !== this.#coordinatorGeneration) {
			return { kind: 'error', code: 'coordinator-generation-mismatch' };
		}
		return { kind: 'ok', value: parsed.data };
	}
}
