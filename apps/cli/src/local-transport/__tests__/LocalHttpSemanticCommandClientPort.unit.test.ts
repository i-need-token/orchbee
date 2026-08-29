import { describe, expect, it, vi } from 'vitest';

import {
	SemanticCommandExecutionIdentitySchema,
	SemanticCommandInvocationRequestSchema,
	SemanticCommandOperationIdSchema,
	SemanticCommandRequestIdentitySchema,
	type SemanticCommandExecutionReference,
} from '@orchbee/semantic-command-protocol';
import {
	encodeSemanticCommandLocalHttpResponse,
	inspectSemanticCommandLocalHttpRequest,
	localHttpError,
	SemanticCommandLocalEndpointIdSchema,
} from '@orchbee/semantic-command-protocol/local-http';

import { LocalHttpSemanticCommandClientPort } from '../LocalHttpSemanticCommandClientPort';
import type { SemanticCommandEndpointNativeClient } from '../semantic-command-endpoint-native-client';

const endpointId = SemanticCommandLocalEndpointIdSchema.parse('12'.repeat(16));
const generation = 'desktop:generation:transport';

function response(body: unknown, status = 200): Uint8Array {
	return encodeSemanticCommandLocalHttpResponse(status, body);
}

function createNativeClient(responses: readonly Uint8Array[]) {
	let responseIndex = 0;
	const exchangeAuthenticated = vi.fn<SemanticCommandEndpointNativeClient['exchangeAuthenticated']>(
		async () => {
			const selected = responses[responseIndex++];
			if (selected === undefined) throw new Error('Missing native response fixture.');
			return selected;
		},
	);
	return {
		exchangeAuthenticated,
		nativeClient: {
			exchangeAuthenticated,
			readLocator: vi.fn<SemanticCommandEndpointNativeClient['readLocator']>(),
		} satisfies SemanticCommandEndpointNativeClient,
	};
}

function requestPath(request: Uint8Array): string {
	const inspected = inspectSemanticCommandLocalHttpRequest(request);
	if (inspected.kind !== 'complete') throw new Error('Expected a complete local HTTP request.');
	return `${inspected.frame.method} ${inspected.frame.path}`;
}

describe('LocalHttpSemanticCommandClientPort', () => {
	it('maps status, discovery, description, invocation, and lookup to the exact local routes', async () => {
		const bodies = [
			{ protocolVersion: 1, coordinatorGeneration: generation, kind: 'ready' },
			{
				protocolVersion: 1,
				coordinatorGeneration: generation,
				kind: 'operation-list',
				operations: [],
			},
			{
				protocolVersion: 1,
				coordinatorGeneration: generation,
				kind: 'operation-unavailable',
				operationId: 'workbench.project.list',
			},
			{
				protocolVersion: 1,
				coordinatorGeneration: generation,
				kind: 'rejected',
				requestIdentity: 'cli:request:1',
				code: 'operation-unavailable',
			},
			{
				protocolVersion: 1,
				coordinatorGeneration: generation,
				kind: 'execution-receipt-unavailable',
				reference: { kind: 'request', requestIdentity: 'cli:request:1' },
			},
		];
		const native = createNativeClient(bodies.map((body) => response(body)));
		const port = new LocalHttpSemanticCommandClientPort({
			endpointId,
			nativeClient: native.nativeClient,
			runtimeDirectory: '/run/user/501/orchbee',
		});
		const operationId = SemanticCommandOperationIdSchema.parse('workbench.project.list');
		const requestIdentity = SemanticCommandRequestIdentitySchema.parse('cli:request:1');

		await port.readStatus();
		await port.listOperations();
		await port.describeOperation(operationId);
		await port.invoke(
			SemanticCommandInvocationRequestSchema.parse({
				protocolVersion: 1,
				requestIdentity,
				operationId,
				operationSchemaVersion: 1,
				input: {},
			}),
		);
		await port.getExecution({ kind: 'request', requestIdentity });

		expect(native.exchangeAuthenticated.mock.calls.map((call) => requestPath(call[2]))).toEqual([
			'GET /v1/status',
			'GET /v1/operations',
			'GET /v1/operations/workbench.project.list',
			'POST /v1/executions',
			'GET /v1/requests/cli%3Arequest%3A1',
		]);
		expect(native.exchangeAuthenticated.mock.calls[0]).toEqual([
			'/run/user/501/orchbee',
			endpointId,
			expect.any(Uint8Array),
			1_064_960,
			5_000,
		]);
	});

	it('resolves a request receipt before using the execution-only cancellation route', async () => {
		const executionIdentity = SemanticCommandExecutionIdentitySchema.parse('desktop:execution:1');
		const requestIdentity = SemanticCommandRequestIdentitySchema.parse('cli:request:1');
		const native = createNativeClient([
			response({
				kind: 'found',
				execution: {
					protocolVersion: 1,
					coordinatorGeneration: generation,
					executionIdentity,
					requestIdentity,
					operationId: 'workbench.project.list',
					operationSchemaVersion: 1,
					phase: 'active',
					cancellationRequested: false,
				},
			}),
			response({
				kind: 'cancellation-observed',
				forwarded: true,
				execution: {
					protocolVersion: 1,
					coordinatorGeneration: generation,
					executionIdentity,
					requestIdentity,
					operationId: 'workbench.project.list',
					operationSchemaVersion: 1,
					phase: 'active',
					cancellationRequested: true,
				},
			}),
		]);
		const port = new LocalHttpSemanticCommandClientPort({
			endpointId,
			nativeClient: native.nativeClient,
			runtimeDirectory: '/run/user/501/orchbee',
		});
		const reference: SemanticCommandExecutionReference = { kind: 'request', requestIdentity };

		await expect(port.cancelExecution(reference)).resolves.toMatchObject({
			kind: 'cancellation-observed',
		});
		expect(native.exchangeAuthenticated.mock.calls.map((call) => requestPath(call[2]))).toEqual([
			'GET /v1/requests/cli%3Arequest%3A1',
			'POST /v1/executions/desktop%3Aexecution%3A1/cancel',
		]);
	});

	it('rejects non-success and malformed responses without exposing their payload', async () => {
		for (const encoded of [
			response(localHttpError('route-not-found'), 404),
			new Uint8Array([1, 2, 3]),
		]) {
			const native = createNativeClient([encoded]);
			const port = new LocalHttpSemanticCommandClientPort({
				endpointId,
				nativeClient: native.nativeClient,
				runtimeDirectory: '/run/user/501/orchbee',
			});
			await expect(port.readStatus()).rejects.toThrow(/(?:Local HTTP|Semantic command)/u);
		}
	});
});
