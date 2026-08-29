import { describe, expect, it } from 'vitest';

import {
	decodeSemanticCommandLocalHttpResponse,
	encodeSemanticCommandLocalHttpRequest,
	encodeSemanticCommandLocalHttpResponse,
	inspectSemanticCommandLocalHttpRequest,
} from '../local-http-framing';

describe('semantic command local HTTP framing', () => {
	it('requires one strict close-delimited HTTP/1.1 message', () => {
		const valid = encodeSemanticCommandLocalHttpRequest('POST', '/v1/executions', {
			value: '你好 👋',
		});
		expect(inspectSemanticCommandLocalHttpRequest(valid)).toMatchObject({
			kind: 'complete',
			frame: { body: { value: '你好 👋' } },
		});
		expect(inspectSemanticCommandLocalHttpRequest(valid.subarray(0, valid.byteLength - 1))).toEqual(
			{ kind: 'incomplete' },
		);
		expect(
			inspectSemanticCommandLocalHttpRequest(
				ascii(
					'POST /v1/executions HTTP/1.1\r\nConnection: close\r\nTransfer-Encoding: chunked\r\nContent-Type: application/json\r\nContent-Length: 0\r\n\r\n',
				),
			),
		).toEqual({ kind: 'rejected', code: 'malformed-request' });
		expect(
			inspectSemanticCommandLocalHttpRequest(
				new Uint8Array([...valid, ...ascii('GET / HTTP/1.1\r\n\r\n')]),
			),
		).toEqual({ kind: 'rejected', code: 'malformed-request' });
	});

	it('enforces request and response capacities without partial JSON', () => {
		expect(() =>
			encodeSemanticCommandLocalHttpRequest('POST', '/v1/executions', {
				value: 'x'.repeat(64 * 1024),
			}),
		).toThrow(/capacity/u);
		expect(
			decodeSemanticCommandLocalHttpResponse(
				encodeSemanticCommandLocalHttpResponse(200, { value: 'x'.repeat(1024 * 1024) }),
			),
		).toEqual({
			status: 500,
			body: {
				protocolVersion: 1,
				kind: 'local-http-error',
				code: 'response-capacity-exceeded',
			},
		});
	});

	it('rejects malformed JSON and noncanonical UTF-8 before routing', () => {
		expect(
			inspectSemanticCommandLocalHttpRequest(
				ascii(
					'POST /v1/executions HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 1\r\n\r\n{',
				),
			),
		).toEqual({ kind: 'rejected', code: 'malformed-request' });
		const prefix = ascii(
			'POST /v1/executions HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n',
		);
		expect(inspectSemanticCommandLocalHttpRequest(new Uint8Array([...prefix, 0xc0, 0x80]))).toEqual(
			{ kind: 'rejected', code: 'malformed-request' },
		);
	});
});

function ascii(value: string): Uint8Array {
	return Uint8Array.from(value, (character) => character.charCodeAt(0));
}
