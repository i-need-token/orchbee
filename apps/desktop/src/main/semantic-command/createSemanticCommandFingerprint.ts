import { createHash } from 'node:crypto';

import type {
	SemanticCommandInvocationRequest,
	SemanticCommandJsonValue,
} from '@orchbee/semantic-command-protocol';

function serializeCanonicalJson(value: SemanticCommandJsonValue): string {
	if (value === null || typeof value === 'boolean' || typeof value === 'number') {
		const encoded = JSON.stringify(value);
		if (encoded === undefined) throw new Error('Semantic command input is not JSON encodable.');
		return encoded;
	}
	if (typeof value === 'string') return JSON.stringify(value);
	if (Array.isArray(value)) {
		return `[${value.map((item) => serializeCanonicalJson(item)).join(',')}]`;
	}
	return `{${Object.entries(value)
		.sort(([left], [right]) => {
			if (left < right) return -1;
			if (left > right) return 1;
			return 0;
		})
		.map(([key, item]) => `${JSON.stringify(key)}:${serializeCanonicalJson(item)}`)
		.join(',')}}`;
}

export function createSemanticCommandFingerprint(
	request: Pick<
		SemanticCommandInvocationRequest,
		'input' | 'operationId' | 'operationSchemaVersion'
	>,
): string {
	const canonical = serializeCanonicalJson({
		input: request.input,
		operationId: request.operationId,
		operationSchemaVersion: request.operationSchemaVersion,
	});
	return createHash('sha256').update(canonical).digest('hex');
}
