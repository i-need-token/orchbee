import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	SemanticCommandCoordinatorGenerationSchema,
	SemanticCommandExecutionIdentitySchema,
	SemanticCommandInvocationRequestSchema,
	SemanticCommandTerminalResultSchema,
	type SemanticCommandExecutionIdentity,
	type SemanticCommandInvocationRequest,
} from '@orchbee/semantic-command-protocol';

import { DesktopSemanticCommandExecutionLedger } from '../DesktopSemanticCommandExecutionLedger';

const ACTIVE_EXECUTION_LIMIT = 32;
const TERMINAL_EXECUTION_LIMIT = 256;
const testCoordinatorGeneration =
	SemanticCommandCoordinatorGenerationSchema.parse('desktop:generation:test');
const completedResult = SemanticCommandTerminalResultSchema.parse({
	kind: 'completed',
	code: 'operation-completed',
	output: { value: 'done' },
});

function executionIdentity(index: number): SemanticCommandExecutionIdentity {
	return SemanticCommandExecutionIdentitySchema.parse(`desktop:execution:${index}`);
}

function invocationRequest(index: number): SemanticCommandInvocationRequest {
	return SemanticCommandInvocationRequestSchema.parse({
		protocolVersion: 1,
		requestIdentity: `client:request:${index}`,
		operationId: 'test.resource.execute',
		operationSchemaVersion: 1,
		input: { value: index },
	});
}

const requestReference = (index: number) => ({
	kind: 'request' as const,
	requestIdentity: invocationRequest(index).requestIdentity,
});

function requireEqual<Value>(actual: Value, expected: Value, context: string): void {
	if (!Object.is(actual, expected)) {
		throw new Error(`${context}: expected ${String(expected)}, received ${String(actual)}.`);
	}
}

describe('DesktopSemanticCommandExecutionLedger', () => {
	it('deduplicates one retained request and rejects a conflicting fingerprint without allocating', () => {
		const ledger = new DesktopSemanticCommandExecutionLedger({
			coordinatorGeneration: testCoordinatorGeneration,
		});
		let allocations = 0;
		const request = invocationRequest(1);
		const first = ledger.admit(request, 'fingerprint-a', () => {
			allocations += 1;
			return executionIdentity(1);
		});
		const replay = ledger.admit(request, 'fingerprint-a', () => {
			allocations += 1;
			return executionIdentity(2);
		});
		const conflict = ledger.admit(request, 'fingerprint-b', () => {
			allocations += 1;
			return executionIdentity(3);
		});

		expect(first.kind).toBe('admitted');
		expect(replay).toEqual(
			first.kind === 'admitted'
				? { kind: 'existing', snapshot: first.snapshot }
				: { kind: 'unreachable' },
		);
		expect(conflict).toEqual({ kind: 'conflict' });
		expect(allocations).toBe(1);
		expect(ledger.activeCount).toBe(1);
	});

	it('admits exactly the active limit and never allocates the rejected execution', () => {
		const ledger = new DesktopSemanticCommandExecutionLedger({
			coordinatorGeneration: testCoordinatorGeneration,
		});
		let allocations = 0;
		for (let index = 0; index < ACTIVE_EXECUTION_LIMIT; index += 1) {
			expect(
				ledger.admit(invocationRequest(index), `fingerprint-${index}`, () => {
					allocations += 1;
					return executionIdentity(index);
				}).kind,
			).toBe('admitted');
		}

		expect(
			ledger.admit(invocationRequest(100), 'over-capacity', () => {
				allocations += 1;
				return executionIdentity(100);
			}),
		).toEqual({ kind: 'capacity-exhausted' });
		expect(allocations).toBe(ACTIVE_EXECUTION_LIMIT);
		expect(ledger.activeCount).toBe(ACTIVE_EXECUTION_LIMIT);
	});

	it('forwards cancellation once and preserves a later terminal result', () => {
		const ledger = new DesktopSemanticCommandExecutionLedger({
			coordinatorGeneration: testCoordinatorGeneration,
		});
		const admitted = ledger.admit(invocationRequest(1), 'fingerprint', () => executionIdentity(1));
		if (admitted.kind !== 'admitted') throw new Error('Expected an admitted execution.');

		const first = ledger.requestCancellation(requestReference(1));
		const repeated = ledger.requestCancellation(requestReference(1));
		ledger.complete(admitted.snapshot.executionIdentity, completedResult, 4);
		const terminal = ledger.requestCancellation(requestReference(1));

		expect(first.kind === 'cancellation-observed' && first.forwarded).toBe(true);
		expect(repeated.kind === 'cancellation-observed' && repeated.forwarded).toBe(false);
		expect(terminal.kind === 'cancellation-observed' && terminal.forwarded).toBe(false);
		expect(terminal.kind === 'cancellation-observed' && terminal.execution.phase).toBe('terminal');
	});

	it('retains the latest terminal record count and evicts only terminal executions', () => {
		const ledger = new DesktopSemanticCommandExecutionLedger({
			coordinatorGeneration: testCoordinatorGeneration,
			maxActiveExecutions: 2,
			maxTerminalExecutions: 1,
		});
		const active = ledger.admit(invocationRequest(0), 'active', () => executionIdentity(0));
		if (active.kind !== 'admitted') throw new Error('Expected an active execution.');
		for (const index of [1, 2]) {
			const admitted = ledger.admit(invocationRequest(index), `terminal-${index}`, () =>
				executionIdentity(index),
			);
			if (admitted.kind !== 'admitted') throw new Error('Expected a terminal candidate.');
			ledger.complete(admitted.snapshot.executionIdentity, completedResult, 1);
		}

		expect(ledger.lookup(requestReference(0)).kind).toBe('found');
		expect(ledger.lookup(requestReference(1)).kind).toBe('execution-receipt-unavailable');
		expect(ledger.lookup(requestReference(2)).kind).toBe('found');
		expect(ledger.activeCount).toBe(1);
		expect(ledger.terminalCount).toBe(1);
	});

	it('retains exactly 256 latest terminal records by default', () => {
		const ledger = new DesktopSemanticCommandExecutionLedger({
			coordinatorGeneration: testCoordinatorGeneration,
		});
		for (let index = 0; index <= TERMINAL_EXECUTION_LIMIT; index += 1) {
			const admitted = ledger.admit(invocationRequest(index), `fingerprint-${index}`, () =>
				executionIdentity(index),
			);
			if (admitted.kind !== 'admitted') throw new Error('Expected an admitted execution.');
			ledger.complete(admitted.snapshot.executionIdentity, completedResult, 1);
		}

		expect(ledger.terminalCount).toBe(TERMINAL_EXECUTION_LIMIT);
		expect(ledger.lookup(requestReference(0)).kind).toBe('execution-receipt-unavailable');
		expect(ledger.lookup(requestReference(1)).kind).toBe('found');
	});

	it('stops forwarding cancellation at the owner commit boundary', () => {
		const ledger = new DesktopSemanticCommandExecutionLedger({
			coordinatorGeneration: testCoordinatorGeneration,
		});
		const admitted = ledger.admit(invocationRequest(1), 'fingerprint-1', () =>
			executionIdentity(1),
		);
		if (admitted.kind !== 'admitted') throw new Error('Expected an admitted execution.');

		expect(ledger.markCommitted(admitted.snapshot.executionIdentity)).toBe(true);
		expect(ledger.markCommitted(admitted.snapshot.executionIdentity)).toBe(false);
		expect(ledger.isCommitted(admitted.snapshot.executionIdentity)).toBe(true);
		expect(ledger.requestCancellation(requestReference(1))).toMatchObject({
			kind: 'cancellation-observed',
			forwarded: false,
			execution: { cancellationRequested: false, phase: 'active' },
		});
	});

	it('evicts oldest terminal records until the exact byte budget holds', () => {
		const ledger = new DesktopSemanticCommandExecutionLedger({
			coordinatorGeneration: testCoordinatorGeneration,
			maxTerminalExecutions: 10,
			maxTerminalResultBytes: 5,
		});
		for (const [index, bytes] of [
			[0, 2],
			[1, 2],
			[2, 3],
		] as const) {
			const admitted = ledger.admit(invocationRequest(index), `fingerprint-${index}`, () =>
				executionIdentity(index),
			);
			if (admitted.kind !== 'admitted') throw new Error('Expected an admitted execution.');
			ledger.complete(admitted.snapshot.executionIdentity, completedResult, bytes);
		}

		expect(ledger.terminalResultBytes).toBe(5);
		expect(ledger.terminalCount).toBe(2);
		expect(ledger.lookup(requestReference(0)).kind).toBe('execution-receipt-unavailable');
	});

	it('preserves a naive receipt model under generated admission, completion, and cancellation traces', () => {
		type Event =
			| { readonly fingerprint: number; readonly kind: 'admit'; readonly request: number }
			| { readonly bytes: number; readonly kind: 'complete'; readonly request: number }
			| { readonly kind: 'cancel'; readonly request: number }
			| { readonly kind: 'mark-committed'; readonly request: number };
		interface ModelRecord {
			cancelled: boolean;
			committed: boolean;
			readonly executionIdentity: SemanticCommandExecutionIdentity;
			readonly fingerprint: string;
			phase: 'active' | 'terminal';
			resultBytes: number;
		}

		const eventArbitrary: fc.Arbitrary<Event> = fc.oneof(
			fc.record({
				kind: fc.constant('admit'),
				request: fc.integer({ min: 0, max: 4 }),
				fingerprint: fc.integer({ min: 0, max: 2 }),
			}),
			fc.record({
				kind: fc.constant('complete'),
				request: fc.integer({ min: 0, max: 4 }),
				bytes: fc.integer({ min: 1, max: 5 }),
			}),
			fc.record({
				kind: fc.constant('cancel'),
				request: fc.integer({ min: 0, max: 4 }),
			}),
			fc.record({
				kind: fc.constant('mark-committed'),
				request: fc.integer({ min: 0, max: 4 }),
			}),
		);
		const reached = new Set<string>();

		fc.assert(
			fc.property(fc.array(eventArbitrary, { maxLength: 80, minLength: 20 }), (events) => {
				const ledger = new DesktopSemanticCommandExecutionLedger({
					coordinatorGeneration: testCoordinatorGeneration,
					maxActiveExecutions: 3,
					maxTerminalExecutions: 3,
					maxTerminalResultBytes: 5,
				});
				const model = new Map<number, ModelRecord>();
				const terminalOrder: number[] = [];
				let nextExecution = 0;

				for (const event of events) {
					const record = model.get(event.request);
					if (event.kind === 'admit') {
						const fingerprint = `fingerprint-${event.fingerprint}`;
						const activeCount = [...model.values()].filter(
							({ phase }) => phase === 'active',
						).length;
						let expected: 'admitted' | 'capacity-exhausted' | 'conflict' | 'existing';
						if (record === undefined) {
							expected = activeCount >= 3 ? 'capacity-exhausted' : 'admitted';
						} else if (record.fingerprint === fingerprint) {
							expected = 'existing';
						} else {
							expected = 'conflict';
						}
						const result = ledger.admit(invocationRequest(event.request), fingerprint, () =>
							executionIdentity(nextExecution),
						);
						requireEqual(result.kind, expected, 'admission result');
						reached.add(expected);
						if (expected === 'admitted') {
							model.set(event.request, {
								cancelled: false,
								committed: false,
								executionIdentity: executionIdentity(nextExecution),
								fingerprint,
								phase: 'active',
								resultBytes: 0,
							});
							nextExecution += 1;
						}
					} else if (event.kind === 'complete' && record?.phase === 'active') {
						ledger.complete(record.executionIdentity, completedResult, event.bytes);
						record.phase = 'terminal';
						record.resultBytes = event.bytes;
						terminalOrder.push(event.request);
						reached.add('complete');
						while (
							terminalOrder.length > 3 ||
							terminalOrder.reduce(
								(total, request) => total + (model.get(request)?.resultBytes ?? 0),
								0,
							) > 5
						) {
							const evicted = terminalOrder.shift();
							if (evicted === undefined) throw new Error('Expected an evicted model record.');
							model.delete(evicted);
							reached.add('evict');
						}
					} else if (event.kind === 'mark-committed' && record?.phase === 'active') {
						const changed = ledger.markCommitted(record.executionIdentity);
						requireEqual(changed, !record.committed, 'commit marker result');
						record.committed = true;
						reached.add('mark-committed');
					} else if (event.kind === 'cancel') {
						const result = ledger.requestCancellation(requestReference(event.request));
						if (record === undefined) {
							requireEqual(
								result.kind,
								'execution-receipt-unavailable',
								'missing cancellation result',
							);
						} else {
							const expectedForward =
								record.phase === 'active' && !record.cancelled && !record.committed;
							requireEqual(result.kind, 'cancellation-observed', 'retained cancellation result');
							if (result.kind !== 'cancellation-observed') {
								throw new Error('Expected a retained cancellation result.');
							}
							requireEqual(result.forwarded, expectedForward, 'cancellation forwarding');
							if (expectedForward) record.cancelled = true;
							reached.add(expectedForward ? 'cancel-forward' : 'cancel-noop');
						}
					}

					expect(ledger.activeCount).toBe(
						[...model.values()].filter(({ phase }) => phase === 'active').length,
					);
					expect(ledger.terminalCount).toBe(terminalOrder.length);
					expect(ledger.terminalResultBytes).toBe(
						terminalOrder.reduce(
							(total, request) => total + (model.get(request)?.resultBytes ?? 0),
							0,
						),
					);
					for (let request = 0; request <= 4; request += 1) {
						const expected = model.get(request);
						const observed = ledger.lookup(requestReference(request));
						if (expected === undefined) {
							requireEqual(
								observed.kind,
								'execution-receipt-unavailable',
								'evicted execution lookup',
							);
						} else {
							requireEqual(observed.kind, 'found', 'retained execution lookup');
							if (observed.kind !== 'found') {
								throw new Error('Expected a retained execution lookup.');
							}
							requireEqual(observed.execution.phase, expected.phase, 'execution phase');
							requireEqual(
								observed.execution.executionIdentity,
								expected.executionIdentity,
								'execution identity',
							);
							if (expected.phase === 'active') {
								requireEqual(
									ledger.isCommitted(expected.executionIdentity),
									expected.committed,
									'active commit marker',
								);
							}
						}
					}
				}
			}),
			{ numRuns: 160, seed: 2_082_026 },
		);
		expect(reached).toEqual(
			new Set([
				'admitted',
				'cancel-forward',
				'cancel-noop',
				'capacity-exhausted',
				'complete',
				'conflict',
				'evict',
				'existing',
				'mark-committed',
			]),
		);
	});

	it('rejects duplicate process execution identities even after receipt eviction', () => {
		const ledger = new DesktopSemanticCommandExecutionLedger({
			coordinatorGeneration: testCoordinatorGeneration,
			maxTerminalExecutions: 1,
		});
		const first = ledger.admit(invocationRequest(1), 'one', () => executionIdentity(1));
		if (first.kind !== 'admitted') throw new Error('Expected an admitted execution.');
		ledger.complete(first.snapshot.executionIdentity, completedResult, 1);
		const second = ledger.admit(invocationRequest(2), 'two', () => executionIdentity(2));
		if (second.kind !== 'admitted') throw new Error('Expected an admitted execution.');
		ledger.complete(second.snapshot.executionIdentity, completedResult, 1);

		expect(() => ledger.admit(invocationRequest(3), 'three', () => executionIdentity(1))).toThrow(
			/process-unique/u,
		);
	});

	it('rejects invalid capacity configuration and result weights', () => {
		expect(
			() =>
				new DesktopSemanticCommandExecutionLedger({
					coordinatorGeneration: testCoordinatorGeneration,
					maxActiveExecutions: 0,
				}),
		).toThrow(/positive safe integer/u);
		const ledger = new DesktopSemanticCommandExecutionLedger({
			coordinatorGeneration: testCoordinatorGeneration,
			maxTerminalResultBytes: 1,
		});
		const admitted = ledger.admit(invocationRequest(1), 'one', () => executionIdentity(1));
		if (admitted.kind !== 'admitted') throw new Error('Expected an admitted execution.');
		expect(() => ledger.complete(admitted.snapshot.executionIdentity, completedResult, 2)).toThrow(
			/byte limit/u,
		);
	});
});
