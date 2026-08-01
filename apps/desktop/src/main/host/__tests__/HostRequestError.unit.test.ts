import { describe, expect, it } from 'vitest';

import { HostRequestError } from '../HostRequestError';

describe('HostRequestError', () => {
	it('preserves the bounded typed Host failure facts', () => {
		const error = new HostRequestError({
			code: 'input-backpressure',
			message: 'The bounded input queue is full.',
			retryable: true,
		});

		expect(error).toBeInstanceOf(Error);
		expect(error.name).toBe('HostRequestError');
		expect(error.code).toBe('input-backpressure');
		expect(error.retryable).toBe(true);
	});
});
