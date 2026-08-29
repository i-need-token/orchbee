import { describe, expect, it, vi } from 'vitest';

import {
	SemanticCommandCoordinatorGenerationSchema,
	SemanticCommandCoordinatorStatusSchema,
	SemanticCommandExecutionCancellationResultSchema,
	SemanticCommandExecutionLookupResultSchema,
	SemanticCommandExecutionReferenceSchema,
	SemanticCommandExecutionSnapshotSchema,
	SemanticCommandInvocationRequestSchema,
	SemanticCommandInvocationResultSchema,
	SemanticCommandOperationDescriptionResultSchema,
	SemanticCommandOperationDescriptorSchema,
	SemanticCommandOperationIdSchema,
	SemanticCommandOperationListSchema,
	type SemanticCommandExecutionLookupResult,
} from '@orchbee/semantic-command-protocol';

import {
	SemanticCommandClient,
	type SemanticCommandClientOptions,
	type SemanticCommandClientPort,
} from '../SemanticCommandClient';

const generation = SemanticCommandCoordinatorGenerationSchema.parse('desktop:generation:1');
const operationId = SemanticCommandOperationIdSchema.parse('workbench.project.list');
const descriptor = SemanticCommandOperationDescriptorSchema.parse({
	id: operationId,
	schemaVersion: 1,
	kind: 'query',
	confirmation: 'noninteractive-allowed',
	retry: 'same-request-while-retained',
	input: { name: 'workbench.page-input', schemaVersion: 1 },
	output: { name: 'workbench.project-page', schemaVersion: 1 },
});
const request = SemanticCommandInvocationRequestSchema.parse({
	protocolVersion: 1,
	requestIdentity: 'client:request:1',
	operationId,
	operationSchemaVersion: 1,
	input: { limit: 100 },
});
const reference = SemanticCommandExecutionReferenceSchema.parse({
	kind: 'request',
	requestIdentity: request.requestIdentity,
});
const activeExecution = SemanticCommandExecutionSnapshotSchema.parse({
	protocolVersion: 1,
	coordinatorGeneration: generation,
	executionIdentity: 'desktop:execution:1',
	requestIdentity: request.requestIdentity,
	operationId,
	operationSchemaVersion: 1,
	phase: 'active',
	cancellationRequested: false,
});
const terminalExecution = SemanticCommandExecutionSnapshotSchema.parse({
	protocolVersion: 1,
	coordinatorGeneration: generation,
	executionIdentity: 'desktop:execution:1',
	requestIdentity: request.requestIdentity,
	operationId,
	operationSchemaVersion: 1,
	phase: 'terminal',
	result: { kind: 'completed', code: 'projects-listed', output: { items: [] } },
});
const status = SemanticCommandCoordinatorStatusSchema.parse({
	protocolVersion: 1,
	coordinatorGeneration: generation,
	kind: 'ready',
});
const operationList = SemanticCommandOperationListSchema.parse({
	protocolVersion: 1,
	coordinatorGeneration: generation,
	kind: 'operation-list',
	operations: [descriptor],
});
const operationDescription = SemanticCommandOperationDescriptionResultSchema.parse({
	protocolVersion: 1,
	coordinatorGeneration: generation,
	kind: 'operation-description',
	operation: descriptor,
});
const invocation = SemanticCommandInvocationResultSchema.parse({
	kind: 'accepted',
	replayed: false,
	execution: activeExecution,
});
const lookup = SemanticCommandExecutionLookupResultSchema.parse({
	kind: 'found',
	execution: activeExecution,
});
const cancellation = SemanticCommandExecutionCancellationResultSchema.parse({
	kind: 'cancellation-observed',
	forwarded: true,
	execution: { ...activeExecution, cancellationRequested: true },
});

function clientPort(overrides: Partial<SemanticCommandClientPort> = {}): SemanticCommandClientPort {
	return {
		cancelExecution: async () => cancellation,
		describeOperation: async () => operationDescription,
		getExecution: async () => lookup,
		invoke: async () => invocation,
		listOperations: async () => operationList,
		readStatus: async () => status,
		...overrides,
	};
}

describe('SemanticCommandClient', () => {
	it('validates every transport-independent coordinator response', async () => {
		const client = new SemanticCommandClient({
			coordinatorGeneration: generation,
			port: clientPort(),
		});

		expect(await client.readStatus()).toEqual({ kind: 'ok', value: status });
		expect(await client.listOperations()).toEqual({ kind: 'ok', value: operationList });
		expect(await client.describeOperation(operationId)).toEqual({
			kind: 'ok',
			value: operationDescription,
		});
		expect(await client.invoke(request)).toEqual({ kind: 'ok', value: invocation });
		expect(await client.getExecution(reference)).toEqual({ kind: 'ok', value: lookup });
		expect(await client.cancelExecution(reference)).toEqual({ kind: 'ok', value: cancellation });
	});

	it('maps malformed responses and thrown transport details to payload-free errors', async () => {
		const malformed = new SemanticCommandClient({
			coordinatorGeneration: generation,
			port: clientPort({ readStatus: async () => ({ ...status, private: 'payload' }) }),
		});
		const unavailable = new SemanticCommandClient({
			coordinatorGeneration: generation,
			port: clientPort({
				readStatus: async () => {
					throw new Error('private transport detail');
				},
			}),
		});

		expect(await malformed.readStatus()).toEqual({ kind: 'error', code: 'invalid-response' });
		expect(await unavailable.readStatus()).toEqual({
			kind: 'error',
			code: 'transport-unavailable',
		});
	});

	it('rejects a response from a different coordinator generation', async () => {
		const differentGeneration = SemanticCommandCoordinatorStatusSchema.parse({
			...status,
			coordinatorGeneration: 'desktop:generation:2',
		});
		const client = new SemanticCommandClient({
			coordinatorGeneration: generation,
			port: clientPort({ readStatus: async () => differentGeneration }),
		});

		expect(await client.readStatus()).toEqual({
			kind: 'error',
			code: 'coordinator-generation-mismatch',
		});
	});

	it('waits by read-only polling with bounded increasing backoff', async () => {
		const observations: SemanticCommandExecutionLookupResult[] = [
			lookup,
			lookup,
			SemanticCommandExecutionLookupResultSchema.parse({
				kind: 'found',
				execution: terminalExecution,
			}),
		];
		let observationIndex = 0;
		const invoke = vi.fn<SemanticCommandClientPort['invoke']>(async () => invocation);
		const cancelExecution = vi.fn<SemanticCommandClientPort['cancelExecution']>(
			async () => cancellation,
		);
		const delays: number[] = [];
		const client = new SemanticCommandClient({
			coordinatorGeneration: generation,
			delay: async (milliseconds) => {
				delays.push(milliseconds);
				return true;
			},
			port: clientPort({
				cancelExecution,
				getExecution: async () => observations[observationIndex++] ?? observations.at(-1),
				invoke,
			}),
		});

		const result = await client.waitForExecution(reference, {
			initialDelayMilliseconds: 10,
			maximumDelayMilliseconds: 15,
		});
		expect(result).toMatchObject({ kind: 'ok', value: { execution: { phase: 'terminal' } } });
		expect(delays).toEqual([10, 15]);
		expect(invoke).not.toHaveBeenCalled();
		expect(cancelExecution).not.toHaveBeenCalled();
	});

	it('stops only local observation when its caller signal is cancelled', async () => {
		const controller = new AbortController();
		const cancelExecution = vi.fn<SemanticCommandClientPort['cancelExecution']>(
			async () => cancellation,
		);
		let reads = 0;
		const client = new SemanticCommandClient({
			coordinatorGeneration: generation,
			delay: async (_milliseconds, signal) => {
				controller.abort();
				return signal?.aborted !== true;
			},
			port: clientPort({
				cancelExecution,
				getExecution: async () => {
					reads += 1;
					return lookup;
				},
			}),
		});

		expect(await client.waitForExecution(reference, { signal: controller.signal })).toEqual({
			kind: 'error',
			code: 'observation-cancelled',
		});
		expect(reads).toBe(1);
		expect(cancelExecution).not.toHaveBeenCalled();
	});

	it('returns receipt unavailability without resubmitting or waiting', async () => {
		const unavailable = SemanticCommandExecutionLookupResultSchema.parse({
			protocolVersion: 1,
			coordinatorGeneration: generation,
			kind: 'execution-receipt-unavailable',
			reference,
		});
		const delay = vi.fn<NonNullable<SemanticCommandClientOptions['delay']>>(async () => true);
		const invoke = vi.fn<SemanticCommandClientPort['invoke']>(async () => invocation);
		const client = new SemanticCommandClient({
			coordinatorGeneration: generation,
			delay,
			port: clientPort({ getExecution: async () => unavailable, invoke }),
		});

		expect(await client.waitForExecution(reference)).toEqual({
			kind: 'ok',
			value: unavailable,
		});
		expect(delay).not.toHaveBeenCalled();
		expect(invoke).not.toHaveBeenCalled();
	});

	it('validates wait bounds before reading the port', async () => {
		const getExecution = vi.fn<SemanticCommandClientPort['getExecution']>(async () => lookup);
		const client = new SemanticCommandClient({
			coordinatorGeneration: generation,
			port: clientPort({ getExecution }),
		});

		await expect(
			client.waitForExecution(reference, {
				initialDelayMilliseconds: 20,
				maximumDelayMilliseconds: 10,
			}),
		).rejects.toThrow(/cannot exceed/u);
		expect(getExecution).not.toHaveBeenCalled();
	});
});
