import { CorrelationIdSchema } from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import { HostRequestError } from '../HostRequestError';

describe('HostRequestError', () => {
	it('preserves the bounded typed Host failure facts', () => {
		const correlationId = CorrelationIdSchema.parse('11'.repeat(16));
		const error = new HostRequestError(
			{
				code: 'input-backpressure',
				message: 'The bounded input queue is full.',
				retryable: true,
			},
			correlationId,
		);

		expect(error).toBeInstanceOf(Error);
		expect(error.name).toBe('HostRequestError');
		expect(error.code).toBe('input-backpressure');
		expect(error.correlationId).toBe(correlationId);
		expect(error.retryable).toBe(true);
	});
});
