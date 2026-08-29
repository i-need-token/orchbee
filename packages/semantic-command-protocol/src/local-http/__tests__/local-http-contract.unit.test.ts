import { describe, expect, it } from 'vitest';

import {
	SemanticCommandLocalEndpointIdSchema,
	SemanticCommandLocalLocatorSchema,
} from '../local-http-contract';

describe('semantic command local HTTP contract', () => {
	it('accepts only bounded locator metadata with one exact endpoint identity', () => {
		expect(
			SemanticCommandLocalLocatorSchema.parse({
				locatorVersion: 1,
				protocolVersion: 1,
				coordinatorGeneration: 'desktop:generation:1',
				endpointId: '12'.repeat(16),
			}),
		).toMatchObject({ endpointId: '12'.repeat(16) });
		expect(SemanticCommandLocalEndpointIdSchema.safeParse('12'.repeat(15)).success).toBe(false);
		expect(
			SemanticCommandLocalLocatorSchema.safeParse({
				locatorVersion: 1,
				protocolVersion: 1,
				coordinatorGeneration: 'desktop:generation:1',
				endpointId: '12'.repeat(16),
				credential: 'forbidden',
			}).success,
		).toBe(false);
	});
});
