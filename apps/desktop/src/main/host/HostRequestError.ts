import type { CorrelationId, TypedFailure } from '@orchbee/terminal-protocol';

export class HostRequestError extends Error {
	readonly code: string;
	readonly correlationId: CorrelationId | undefined;
	readonly retryable: boolean;

	constructor(failure: TypedFailure, correlationId?: CorrelationId) {
		super(failure.message);
		this.name = 'HostRequestError';
		this.code = failure.code;
		this.correlationId = correlationId;
		this.retryable = failure.retryable;
	}
}
