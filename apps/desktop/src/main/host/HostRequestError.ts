import type { TypedFailure } from '@orchbee/terminal-protocol';

export class HostRequestError extends Error {
	readonly code: string;
	readonly retryable: boolean;

	constructor(failure: TypedFailure) {
		super(failure.message);
		this.name = 'HostRequestError';
		this.code = failure.code;
		this.retryable = failure.retryable;
	}
}
