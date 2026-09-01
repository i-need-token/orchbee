import { describe, expect, it } from 'vitest';

import { createMessageDescriptorFactory } from '../createMessageDescriptorFactory';

describe('createMessageDescriptorFactory', () => {
	it('binds message identifiers to their canonical English defaults', () => {
		const message = createMessageDescriptorFactory({ greeting: 'Hello' });

		expect(message('greeting')).toEqual({ defaultMessage: 'Hello', id: 'greeting' });
	});

	it('retains optional translator context', () => {
		const message = createMessageDescriptorFactory({ greeting: 'Hello' });

		expect(message('greeting', 'Greeting shown on startup.')).toEqual({
			defaultMessage: 'Hello',
			description: 'Greeting shown on startup.',
			id: 'greeting',
		});
	});
});
