import { describe, expect, it, vi } from 'vitest';

import {
	SemanticCommandCoordinatorGenerationSchema,
	SemanticCommandExecutionIdentitySchema,
	SemanticCommandInvocationRequestSchema,
	SemanticCommandOperationDescriptorSchema,
	SemanticCommandOperationIdSchema,
	SemanticCommandTerminalResultSchema,
	type SemanticCommandExecutionIdentity,
	type SemanticCommandExecutionSnapshot,
	type SemanticCommandInvocationRequest,
} from '@orchbee/semantic-command-protocol';

import {
	DesktopSemanticCommandCatalog,
	type DesktopSemanticCommandHandlerSettlement,
	type DesktopSemanticCommandOperation,
} from '../DesktopSemanticCommandCatalog';
import { DesktopSemanticCommandService } from '../DesktopSemanticCommandService';

const testCoordinatorGeneration =
	SemanticCommandCoordinatorGenerationSchema.parse('desktop:generation:test');
const testOperationId = SemanticCommandOperationIdSchema.parse('test.resource.execute');
const testDescriptor = SemanticCommandOperationDescriptorSchema.parse({
	id: testOperationId,
	schemaVersion: 1,
	kind: 'command',
	confirmation: 'noninteractive-allowed',
	retry: 'same-request-while-retained',
	input: { name: 'test.resource-input', schemaVersion: 1 },
	output: { name: 'test.resource-output', schemaVersion: 1 },
});
const parsedCompletedResult = SemanticCommandTerminalResultSchema.parse({
	kind: 'completed',
	code: 'operation-completed',
	output: { value: 'done' },
});
if (parsedCompletedResult.kind !== 'completed') {
	throw new Error('Expected a completed semantic command test result.');
}
const completedResult = parsedCompletedResult;
const cancelledResult = SemanticCommandTerminalResultSchema.parse({
	kind: 'cancelled',
	code: 'operation-cancelled',
});

function executionIdentity(index: number): SemanticCommandExecutionIdentity {
	return SemanticCommandExecutionIdentitySchema.parse(`desktop:execution:${index}`);
}

function invocationRequest(
	index: number,
	input: SemanticCommandInvocationRequest['input'] = { value: index },
): SemanticCommandInvocationRequest {
	return SemanticCommandInvocationRequestSchema.parse({
		protocolVersion: 1,
		requestIdentity: `client:request:${index}`,
		operationId: testOperationId,
		operationSchemaVersion: 1,
		input,
	});
}

function semanticCommandOperation(
	overrides: Partial<DesktopSemanticCommandOperation> = {},
): DesktopSemanticCommandOperation {
	return {
		authorize: () => true,
		canonicalizeInput: (input) => ({ input, kind: 'accepted' }),
		descriptor: testDescriptor,
		execute: async (): Promise<DesktopSemanticCommandHandlerSettlement> => ({
			effect: 'none',
			result: completedResult,
		}),
		isVisibleTo: () => true,
		validateOutput: () => true,
		...overrides,
	};
}

interface Deferred<Value> {
	readonly promise: Promise<Value>;
	resolve(value: Value): void;
}

function deferred<Value>(): Deferred<Value> {
	let settle: ((value: Value) => void) | undefined;
	const promise = new Promise<Value>((resolve) => {
		settle = resolve;
	});
	return {
		promise,
		resolve(value) {
			if (settle === undefined) throw new Error('Deferred value has no resolver.');
			settle(value);
		},
	};
}

const caller = { kind: 'local-user' } as const;
const requestReference = (index: number) => ({
	kind: 'request' as const,
	requestIdentity: invocationRequest(index).requestIdentity,
});

interface TerminalExecutionLookupResult {
	readonly execution: Extract<SemanticCommandExecutionSnapshot, { readonly phase: 'terminal' }>;
	readonly kind: 'found';
}

function createService(
	operations = [semanticCommandOperation()],
	limits: {
		readonly maxActiveExecutions?: number;
		readonly maxExecutionResultBytes?: number;
		readonly maxTerminalResultBytes?: number;
	} = {},
): { readonly allocationCount: () => number; readonly service: DesktopSemanticCommandService } {
	let allocations = 0;
	return {
		allocationCount: () => allocations,
		service: new DesktopSemanticCommandService(new DesktopSemanticCommandCatalog(operations), {
			coordinatorGeneration: testCoordinatorGeneration,
			createExecutionIdentity: () => executionIdentity(allocations++),
			...limits,
		}),
	};
}

async function readTerminal(
	service: DesktopSemanticCommandService,
	requestIndex: number,
): Promise<TerminalExecutionLookupResult> {
	await vi.waitFor(() => {
		const result = service.get(requestReference(requestIndex));
		if (result.kind !== 'found' || result.execution.phase !== 'terminal') {
			throw new Error('Semantic command execution is not terminal yet.');
		}
	});
	const result = service.get(requestReference(requestIndex));
	if (result.kind !== 'found' || result.execution.phase !== 'terminal') {
		throw new Error('Expected a terminal semantic command execution.');
	}
	return { kind: 'found', execution: result.execution };
}

describe('DesktopSemanticCommandService', () => {
	it('starts ready with an explicit empty catalog that grants no operation authority', () => {
		const { allocationCount, service } = createService([]);

		expect(service.status()).toEqual({
			protocolVersion: 1,
			coordinatorGeneration: testCoordinatorGeneration,
			kind: 'ready',
		});
		expect(service.list(caller).operations).toEqual([]);
		expect(service.describe(caller, testOperationId).kind).toBe('operation-unavailable');
		expect(service.invoke(caller, invocationRequest(1))).toMatchObject({
			kind: 'rejected',
			code: 'operation-unavailable',
		});
		expect(allocationCount()).toBe(0);
	});

	it('rejects wrong versions, invalid input, unauthorized callers, and confirmation policy before allocation', () => {
		const cases = [
			{
				operation: semanticCommandOperation(),
				request: SemanticCommandInvocationRequestSchema.parse({
					...invocationRequest(1),
					operationSchemaVersion: 2,
				}),
				code: 'operation-version-unsupported',
			},
			{
				operation: semanticCommandOperation({ canonicalizeInput: () => ({ kind: 'rejected' }) }),
				request: invocationRequest(1),
				code: 'input-invalid',
			},
			{
				operation: semanticCommandOperation({ authorize: () => false }),
				request: invocationRequest(1),
				code: 'caller-unauthorized',
			},
			{
				operation: semanticCommandOperation({
					descriptor: SemanticCommandOperationDescriptorSchema.parse({
						...testDescriptor,
						confirmation: 'interactive-only',
					}),
				}),
				request: invocationRequest(1),
				code: 'operation-confirmation-required',
			},
		] as const;

		for (const { code, operation, request } of cases) {
			const { allocationCount, service } = createService([operation]);
			expect(service.invoke(caller, request)).toMatchObject({ kind: 'rejected', code });
			expect(allocationCount()).toBe(0);
		}
	});

	it('returns one active or terminal receipt for canonical replay and rejects conflicting data', async () => {
		const settlement = deferred<{
			readonly effect: 'none';
			readonly result: typeof completedResult;
		}>();
		let handlerStarts = 0;
		const { allocationCount, service } = createService([
			semanticCommandOperation({
				execute: async () => {
					handlerStarts += 1;
					return settlement.promise;
				},
			}),
		]);
		const firstRequest = invocationRequest(1, { alpha: 1, beta: 2 });
		const first = service.invoke(caller, firstRequest);
		const replay = service.invoke(caller, invocationRequest(1, { beta: 2, alpha: 1 }));
		const conflict = service.invoke(caller, invocationRequest(1, { alpha: 2, beta: 2 }));

		expect(first).toMatchObject({ kind: 'accepted', replayed: false });
		expect(replay).toEqual(
			first.kind === 'accepted' ? { ...first, replayed: true } : { kind: 'unreachable' },
		);
		expect(conflict).toMatchObject({ kind: 'rejected', code: 'request-identity-conflict' });
		expect(handlerStarts).toBe(1);
		expect(allocationCount()).toBe(1);

		settlement.resolve({ effect: 'none', result: completedResult });
		const terminal = await readTerminal(service, 1);
		const terminalReplay = service.invoke(caller, firstRequest);
		expect(terminal.execution.phase).toBe('terminal');
		expect(terminalReplay).toMatchObject({
			kind: 'accepted',
			replayed: true,
			execution: { phase: 'terminal' },
		});
		expect(handlerStarts).toBe(1);
	});

	it('forwards one cancellation signal and accepts the handler-owned pre-commit result', async () => {
		const settlement = deferred<{
			readonly effect: 'none';
			readonly result: typeof cancelledResult;
		}>();
		let abortCount = 0;
		const { service } = createService([
			semanticCommandOperation({
				execute: async (_input, context) => {
					context.signal.addEventListener('abort', () => {
						abortCount += 1;
					});
					return settlement.promise;
				},
			}),
		]);
		service.invoke(caller, invocationRequest(1));

		expect(service.cancel(requestReference(1))).toMatchObject({
			kind: 'cancellation-observed',
			forwarded: true,
		});
		expect(service.cancel(requestReference(1))).toMatchObject({
			kind: 'cancellation-observed',
			forwarded: false,
		});
		expect(abortCount).toBe(1);

		settlement.resolve({ effect: 'none', result: cancelledResult });
		const terminal = await readTerminal(service, 1);
		expect(terminal.execution).toMatchObject({
			phase: 'terminal',
			result: { kind: 'cancelled' },
		});
	});

	it('preserves a committed completion when cancellation races after the owner commit', async () => {
		const settlement = deferred<{
			readonly effect: 'committed';
			readonly result: typeof completedResult;
		}>();
		const { service } = createService([
			semanticCommandOperation({ execute: async () => settlement.promise }),
		]);
		service.invoke(caller, invocationRequest(1));
		service.cancel(requestReference(1));
		settlement.resolve({ effect: 'committed', result: completedResult });

		const terminal = await readTerminal(service, 1);
		expect(terminal.execution).toMatchObject({
			phase: 'terminal',
			result: { kind: 'completed' },
		});
	});

	it('does not forward cancellation after an active handler reports its durable commit', async () => {
		const settlement = deferred<{
			readonly effect: 'committed';
			readonly result: typeof completedResult;
		}>();
		let markCommitted: (() => void) | undefined;
		let abortCount = 0;
		const { service } = createService([
			semanticCommandOperation({
				execute: async (_input, context) => {
					markCommitted = context.markCommitted;
					context.signal.addEventListener('abort', () => {
						abortCount += 1;
					});
					return settlement.promise;
				},
			}),
		]);
		service.invoke(caller, invocationRequest(1));
		if (markCommitted === undefined) throw new Error('Expected a handler commit marker.');
		markCommitted();

		expect(service.cancel(requestReference(1))).toMatchObject({
			kind: 'cancellation-observed',
			forwarded: false,
			execution: { cancellationRequested: false, phase: 'active' },
		});
		expect(abortCount).toBe(0);
		settlement.resolve({ effect: 'committed', result: completedResult });
		expect((await readTerminal(service, 1)).execution.result.kind).toBe('completed');
	});

	it('maps handler exceptions, unsafe output, and oversized results to payload-free failures', async () => {
		const scenarios = [
			{
				operation: semanticCommandOperation({
					execute: async () => {
						throw new Error('private payload');
					},
				}),
				code: 'operation-handler-failed',
			},
			{
				operation: semanticCommandOperation({ validateOutput: () => false }),
				code: 'operation-output-invalid',
			},
			{
				operation: semanticCommandOperation({
					execute: async () => ({
						effect: 'none',
						result: {
							...completedResult,
							output: { value: 'x'.repeat(1_000) },
						},
					}),
				}),
				code: 'output-capacity-exceeded',
			},
		] as const;

		for (const [index, scenario] of scenarios.entries()) {
			const { service } = createService([scenario.operation], {
				maxExecutionResultBytes: 100,
			});
			service.invoke(caller, invocationRequest(index));
			const terminal = await readTerminal(service, index);
			expect(terminal.execution).toMatchObject({
				phase: 'terminal',
				result: { kind: 'failed', code: scenario.code },
			});
			expect('output' in terminal.execution.result).toBe(false);
		}
	});

	it('aborts active handlers on disposal and closes later admission', () => {
		let observedSignal: AbortSignal | undefined;
		const neverSettles = deferred<never>();
		const { allocationCount, service } = createService([
			semanticCommandOperation({
				execute: async (_input, context) => {
					observedSignal = context.signal;
					return neverSettles.promise;
				},
			}),
		]);
		service.invoke(caller, invocationRequest(1));
		service.dispose();
		service.dispose();

		expect(observedSignal?.aborted).toBe(true);
		expect(service.invoke(caller, invocationRequest(2))).toMatchObject({
			kind: 'rejected',
			code: 'coordinator-unavailable',
		});
		expect(allocationCount()).toBe(1);
	});
});
