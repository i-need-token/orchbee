import { describe, expect, it } from 'vitest';

import { SemanticCommandCoordinatorGenerationSchema } from '@orchbee/semantic-command-protocol';
import {
	decodeSemanticCommandLocalHttpResponse,
	encodeSemanticCommandLocalHttpRequest,
	SemanticCommandLocalHttpErrorSchema,
} from '@orchbee/semantic-command-protocol/local-http';

import { DesktopSemanticCommandCatalog } from '../DesktopSemanticCommandCatalog';
import { DesktopSemanticCommandService } from '../DesktopSemanticCommandService';
import { handleDesktopSemanticCommandLocalHttpRequest } from '../handleDesktopSemanticCommandLocalHttpRequest';

function createService(): DesktopSemanticCommandService {
	return new DesktopSemanticCommandService(new DesktopSemanticCommandCatalog([]), {
		coordinatorGeneration: SemanticCommandCoordinatorGenerationSchema.parse(
			'desktop:generation:http-errors',
		),
		createExecutionIdentity: () => {
			throw new Error('No operation is admitted by this fixture.');
		},
	});
}

function exchange(bytes: Uint8Array) {
	return decodeSemanticCommandLocalHttpResponse(
		handleDesktopSemanticCommandLocalHttpRequest(createService(), bytes),
	);
}

describe('handleDesktopSemanticCommandLocalHttpRequest', () => {
	it('fails closed on unknown routes, unknown methods, malformed framing, and pipelining', () => {
		const cases = [
			[encodeSemanticCommandLocalHttpRequest('GET', '/v1/unknown'), 404, 'route-not-found'],
			[
				new TextEncoder().encode(
					'PUT /v1/status HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 0\r\n\r\n',
				),
				405,
				'method-not-allowed',
			],
			[new Uint8Array([1, 2, 3]), 400, 'malformed-request'],
			[
				new TextEncoder().encode(
					'GET /v1/status HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 0\r\n\r\nextra',
				),
				400,
				'malformed-request',
			],
		] as const;

		for (const [request, status, code] of cases) {
			const response = exchange(request);
			expect(response.status).toBe(status);
			expect(SemanticCommandLocalHttpErrorSchema.parse(response.body).code).toBe(code);
		}
	});

	it('requires application/json for every POST body', () => {
		const response = exchange(
			new TextEncoder().encode(
				'POST /v1/executions HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\n{}',
			),
		);
		expect(response.status).toBe(400);
		expect(SemanticCommandLocalHttpErrorSchema.parse(response.body).code).toBe(
			'content-type-required',
		);
	});
});
