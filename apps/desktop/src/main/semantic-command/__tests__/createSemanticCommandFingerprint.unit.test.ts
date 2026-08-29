import { describe, expect, it } from 'vitest';

import {
	SemanticCommandInvocationRequestSchema,
	type SemanticCommandInvocationRequest,
} from '@orchbee/semantic-command-protocol';

import { createSemanticCommandFingerprint } from '../createSemanticCommandFingerprint';

function invocationRequest(
	index: number,
	input: SemanticCommandInvocationRequest['input'],
): SemanticCommandInvocationRequest {
	return SemanticCommandInvocationRequestSchema.parse({
		protocolVersion: 1,
		requestIdentity: `client:request:${index}`,
		operationId: 'test.resource.execute',
		operationSchemaVersion: 1,
		input,
	});
}

describe('createSemanticCommandFingerprint', () => {
	it('canonicalizes object keys without changing array order or operation identity', () => {
		const first = invocationRequest(1, { alpha: 1, nested: { first: true, second: false } });
		const reordered = invocationRequest(2, {
			nested: { second: false, first: true },
			alpha: 1,
		});
		const changed = invocationRequest(3, { alpha: 1, nested: [false, true] });

		expect(createSemanticCommandFingerprint(first)).toMatch(/^[a-f\d]{64}$/u);
		expect(createSemanticCommandFingerprint(first)).toBe(
			createSemanticCommandFingerprint(reordered),
		);
		expect(createSemanticCommandFingerprint(changed)).not.toBe(
			createSemanticCommandFingerprint(first),
		);
	});
});
