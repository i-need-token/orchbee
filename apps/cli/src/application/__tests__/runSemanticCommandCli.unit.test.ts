import { describe, expect, it, vi } from 'vitest';

import {
	SemanticCommandCoordinatorGenerationSchema,
	SemanticCommandExecutionSnapshotSchema,
	SemanticCommandInvocationResultSchema,
	SemanticCommandOperationDescriptionResultSchema,
	SemanticCommandOperationDescriptorSchema,
	SemanticCommandOperationListSchema,
	SemanticCommandTerminalResultSchema,
	type SemanticCommandCoordinatorGeneration,
	type SemanticCommandExecutionSnapshot,
	type SemanticCommandTerminalResult,
} from '@orchbee/semantic-command-protocol';

import { SemanticCommandClient, type SemanticCommandClientPort } from '../../client';
import {
	runSemanticCommandCli,
	type SemanticCommandCliConnectionResult,
	type SemanticCommandCliDependencies,
} from '../runSemanticCommandCli';

const generationOne = SemanticCommandCoordinatorGenerationSchema.parse('desktop:generation:one');
const generationTwo = SemanticCommandCoordinatorGenerationSchema.parse('desktop:generation:two');
const descriptor = SemanticCommandOperationDescriptorSchema.parse({
	confirmation: 'noninteractive-allowed',
	id: 'workbench.terminal.create',
	input: { name: 'workbench.terminal-create-input', schemaVersion: 1 },
	kind: 'command',
	output: { name: 'workbench.terminal-create-output', schemaVersion: 1 },
	retry: 'same-request-while-retained',
	schemaVersion: 1,
});

function activeExecution(
	requestIdentity = 'cli:request:1',
	executionIdentity = 'desktop:execution:1',
	generation = generationOne,
): SemanticCommandExecutionSnapshot {
	return SemanticCommandExecutionSnapshotSchema.parse({
		cancellationRequested: false,
		coordinatorGeneration: generation,
		executionIdentity,
		operationId: descriptor.id,
		operationSchemaVersion: 1,
		phase: 'active',
		protocolVersion: 1,
		requestIdentity,
	});
}

function terminalExecution(
	result: SemanticCommandTerminalResult = SemanticCommandTerminalResultSchema.parse({
		code: 'terminal-created',
		kind: 'completed',
		output: { revision: '2' },
	}),
	requestIdentity = 'cli:request:1',
	executionIdentity = 'desktop:execution:1',
	generation = generationOne,
): SemanticCommandExecutionSnapshot {
	return SemanticCommandExecutionSnapshotSchema.parse({
		coordinatorGeneration: generation,
		executionIdentity,
		operationId: descriptor.id,
		operationSchemaVersion: 1,
		phase: 'terminal',
		protocolVersion: 1,
		requestIdentity,
		result,
	});
}

function clientWith(
	overrides: Partial<SemanticCommandClientPort> = {},
	generation: SemanticCommandCoordinatorGeneration = generationOne,
): { readonly client: SemanticCommandClient; readonly port: SemanticCommandClientPort } {
	const port: SemanticCommandClientPort = {
		cancelExecution: async (reference) => ({
			execution: activeExecution(),
			forwarded: true,
			kind: 'cancellation-observed',
			reference,
		}),
		describeOperation: async () =>
			SemanticCommandOperationDescriptionResultSchema.parse({
				coordinatorGeneration: generation,
				kind: 'operation-description',
				operation: descriptor,
				protocolVersion: 1,
			}),
		getExecution: async (reference) => ({
			coordinatorGeneration: generation,
			kind: 'execution-receipt-unavailable',
			protocolVersion: 1,
			reference,
		}),
		invoke: async (request) =>
			SemanticCommandInvocationResultSchema.parse({
				execution: terminalExecution(undefined, request.requestIdentity),
				kind: 'accepted',
				replayed: false,
			}),
		listOperations: async () =>
			SemanticCommandOperationListSchema.parse({
				coordinatorGeneration: generation,
				kind: 'operation-list',
				operations: [descriptor],
				protocolVersion: 1,
			}),
		readStatus: async () => ({
			coordinatorGeneration: generation,
			kind: 'ready',
			protocolVersion: 1,
		}),
		...overrides,
	};
	return {
		client: new SemanticCommandClient({
			coordinatorGeneration: generation,
			delay: async () => true,
			port,
		}),
		port,
	};
}

function dependenciesWith(
	connection: SemanticCommandCliConnectionResult,
	overrides: Partial<SemanticCommandCliDependencies> = {},
): SemanticCommandCliDependencies {
	return {
		connect: async () => connection,
		createRequestIdentity: () => 'cli:request:1',
		readInput: async () => new TextEncoder().encode('{"location":{"kind":"standalone"}}'),
		...overrides,
	};
}

function connected(
	client: SemanticCommandClient,
	coordinatorGeneration = generationOne,
): SemanticCommandCliConnectionResult {
	return { client, coordinatorGeneration, kind: 'connected' };
}

function output(result: Awaited<ReturnType<typeof runSemanticCommandCli>>) {
	return JSON.parse(result.stdout);
}

describe('runSemanticCommandCli', () => {
	it('emits one versioned JSON document and forwards one exact optional target', async () => {
		const fixture = clientWith();
		const endpointId = 'ab'.repeat(16);
		const connect = vi.fn<SemanticCommandCliDependencies['connect']>(async () =>
			connected(fixture.client),
		);
		const result = await runSemanticCommandCli(
			['operation', 'list', '--target', endpointId],
			dependenciesWith(connected(fixture.client), { connect }),
		);

		expect(result.exitCode).toBe(0);
		expect(result.stderr).toBe('');
		expect(result.stdout.endsWith('\n')).toBe(true);
		expect(output(result)).toMatchObject({
			cliVersion: 1,
			command: 'operation.list',
			kind: 'result',
			result: { kind: 'operation-list' },
		});
		expect(connect).toHaveBeenCalledWith(endpointId);
	});

	it('creates request identity before connecting, invokes once, and waits by read-only lookup', async () => {
		const trace: string[] = [];
		let lookupCount = 0;
		const invoke = vi.fn<SemanticCommandClientPort['invoke']>(async (request) => {
			trace.push('invoke');
			return {
				execution: activeExecution(request.requestIdentity),
				kind: 'accepted',
				replayed: false,
			};
		});
		const getExecution = vi.fn<SemanticCommandClientPort['getExecution']>(async () => {
			lookupCount += 1;
			return {
				execution: lookupCount === 1 ? activeExecution() : terminalExecution(),
				kind: 'found',
			};
		});
		const fixture = clientWith({ getExecution, invoke });
		const result = await runSemanticCommandCli(
			['operation', 'invoke', descriptor.id, '--input-file', '-'],
			dependenciesWith(connected(fixture.client), {
				connect: async () => {
					trace.push('connect');
					return connected(fixture.client);
				},
				createRequestIdentity: () => {
					trace.push('request');
					return 'cli:request:1';
				},
			}),
		);

		expect(trace).toEqual(['request', 'connect', 'invoke']);
		expect(invoke).toHaveBeenCalledOnce();
		expect(getExecution).toHaveBeenCalledTimes(2);
		expect(result.exitCode).toBe(0);
		expect(output(result)).toMatchObject({
			command: 'operation.invoke',
			requestIdentity: 'cli:request:1',
			result: { execution: { phase: 'terminal' }, kind: 'found' },
		});
	});

	it('detach returns acceptance without observing or cancelling business work', async () => {
		const getExecution = vi.fn<SemanticCommandClientPort['getExecution']>();
		const cancelExecution = vi.fn<SemanticCommandClientPort['cancelExecution']>();
		const fixture = clientWith({
			cancelExecution,
			getExecution,
			invoke: async (request) => ({
				execution: activeExecution(request.requestIdentity),
				kind: 'accepted',
				replayed: false,
			}),
		});
		const result = await runSemanticCommandCli(
			['operation', 'invoke', descriptor.id, '--input-file', '-', '--detach'],
			dependenciesWith(connected(fixture.client)),
		);

		expect(result.exitCode).toBe(0);
		expect(output(result)).toMatchObject({ result: { execution: { phase: 'active' } } });
		expect(getExecution).not.toHaveBeenCalled();
		expect(cancelExecution).not.toHaveBeenCalled();
	});

	it('rejects malformed and oversized input before request generation or connection', async () => {
		for (const bytes of [new TextEncoder().encode('{private-payload'), new Uint8Array(65_537)]) {
			const connect = vi.fn<SemanticCommandCliDependencies['connect']>();
			const createRequestIdentity =
				vi.fn<SemanticCommandCliDependencies['createRequestIdentity']>();
			const result = await runSemanticCommandCli(
				['operation', 'invoke', descriptor.id, '--input-file', '-'],
				dependenciesWith(
					{ code: 'coordinator-unavailable', kind: 'error' },
					{
						connect,
						createRequestIdentity,
						readInput: async () => bytes,
					},
				),
			);
			expect(result.exitCode).toBe(2);
			expect(connect).not.toHaveBeenCalled();
			expect(createRequestIdentity).not.toHaveBeenCalled();
			expect(result.stdout).not.toContain('private-payload');
			expect(result.stderr).not.toContain('private-payload');
		}
	});

	it('maps every terminal result kind to the accepted coarse exit code', async () => {
		for (const [kind, expectedExitCode] of [
			['completed', 0],
			['unchanged', 0],
			['blocked', 3],
			['unsupported', 3],
			['cancelled', 4],
			['failed', 5],
		] as const) {
			const terminalResult = SemanticCommandTerminalResultSchema.parse(
				kind === 'completed' || kind === 'unchanged'
					? { code: `operation-${kind}`, kind, output: {} }
					: { code: `operation-${kind}`, kind },
			);
			const fixture = clientWith({
				invoke: async (request) => ({
					execution: terminalExecution(terminalResult, request.requestIdentity),
					kind: 'accepted',
					replayed: false,
				}),
			});
			const result = await runSemanticCommandCli(
				['operation', 'invoke', descriptor.id, '--input-file', '-'],
				dependenciesWith(connected(fixture.client)),
			);
			expect(result.exitCode).toBe(expectedExitCode);
		}
	});

	it('reports an accepted mutation as unknown after a generation change without replay', async () => {
		const invoke = vi.fn<SemanticCommandClientPort['invoke']>(async (request) => ({
			execution: activeExecution(request.requestIdentity),
			kind: 'accepted',
			replayed: false,
		}));
		const first = clientWith({
			getExecution: async () => {
				throw new Error('transport lost');
			},
			invoke,
		});
		const second = clientWith({}, generationTwo);
		const connections = [connected(first.client), connected(second.client, generationTwo)];
		const connect = vi.fn<SemanticCommandCliDependencies['connect']>(async () => {
			const next = connections.shift();
			return next ?? { code: 'coordinator-unavailable', kind: 'error' };
		});
		const result = await runSemanticCommandCli(
			['operation', 'invoke', descriptor.id, '--input-file', '-'],
			dependenciesWith(connected(first.client), { connect }),
		);

		expect(result.exitCode).toBe(6);
		expect(output(result)).toMatchObject({
			code: 'outcome-unknown-after-restart',
			requestIdentity: 'cli:request:1',
		});
		expect(invoke).toHaveBeenCalledOnce();
	});

	it('recovers a lost invocation response by receipt lookup without replaying the mutation', async () => {
		const invoke = vi.fn<SemanticCommandClientPort['invoke']>(async () => {
			throw new Error('response lost');
		});
		const first = clientWith({ invoke });
		const second = clientWith({
			getExecution: async () => ({ execution: terminalExecution(), kind: 'found' }),
		});
		const connections = [connected(first.client), connected(second.client)];
		const result = await runSemanticCommandCli(
			['operation', 'invoke', descriptor.id, '--input-file', '-'],
			dependenciesWith(connected(first.client), {
				connect: async () =>
					connections.shift() ?? { code: 'coordinator-unavailable', kind: 'error' },
			}),
		);

		expect(result.exitCode).toBe(0);
		expect(output(result)).toMatchObject({
			requestIdentity: 'cli:request:1',
			result: { execution: { phase: 'terminal' }, kind: 'found' },
		});
		expect(invoke).toHaveBeenCalledOnce();
	});

	it('resolves a positional reference read-only and cancels only one unambiguous execution', async () => {
		const cancelExecution = vi.fn<SemanticCommandClientPort['cancelExecution']>(async () => ({
			execution: activeExecution('shared', 'execution-a'),
			forwarded: true,
			kind: 'cancellation-observed',
		}));
		const fixture = clientWith({
			cancelExecution,
			getExecution: async (reference) =>
				reference.kind === 'request'
					? { execution: activeExecution('shared', 'execution-a'), kind: 'found' }
					: {
							coordinatorGeneration: generationOne,
							kind: 'execution-receipt-unavailable',
							protocolVersion: 1,
							reference,
						},
		});
		const result = await runSemanticCommandCli(
			['execution', 'cancel', 'shared'],
			dependenciesWith(connected(fixture.client)),
		);
		expect(result.exitCode).toBe(0);
		expect(cancelExecution).toHaveBeenCalledOnce();
		expect(cancelExecution).toHaveBeenCalledWith({
			executionIdentity: 'execution-a',
			kind: 'execution',
		});
	});

	it('fails closed when one string names two different retained executions', async () => {
		const cancelExecution = vi.fn<SemanticCommandClientPort['cancelExecution']>();
		const fixture = clientWith({
			cancelExecution,
			getExecution: async (reference) => ({
				execution:
					reference.kind === 'request'
						? activeExecution('shared', 'execution-a')
						: activeExecution('other-request', 'execution-b'),
				kind: 'found',
			}),
		});
		const result = await runSemanticCommandCli(
			['execution', 'cancel', 'shared'],
			dependenciesWith(connected(fixture.client)),
		);

		expect(result.exitCode).toBe(6);
		expect(output(result)).toMatchObject({ code: 'execution-reference-ambiguous' });
		expect(cancelExecution).not.toHaveBeenCalled();
	});

	it('returns receipt absence as an explicit unknown result', async () => {
		const fixture = clientWith();
		const result = await runSemanticCommandCli(
			['execution', 'get', 'missing'],
			dependenciesWith(connected(fixture.client)),
		);
		expect(result.exitCode).toBe(6);
		expect(output(result)).toMatchObject({
			result: { kind: 'execution-receipt-unavailable' },
		});
	});
});
