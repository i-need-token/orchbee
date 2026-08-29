import { describe, expect, it } from 'vitest';

import {
	SemanticCommandExecutionCancellationResultSchema,
	SemanticCommandExecutionLookupResultSchema,
	SemanticCommandExecutionSnapshotSchema,
	SemanticCommandInvocationRequestSchema,
	SemanticCommandInvocationResultSchema,
	SemanticCommandTerminalResultSchema,
} from '../execution-contract';
import { SEMANTIC_COMMAND_PROTOCOL_VERSION } from '../identity-contract';
import { SemanticCommandJsonValueSchema } from '../json-value';
import {
	SemanticCommandOperationDescriptionResultSchema,
	SemanticCommandOperationDescriptorSchema,
	SemanticCommandOperationListSchema,
} from '../operation-contract';

const descriptor = {
	id: 'workbench.terminal.create',
	schemaVersion: 1,
	kind: 'command',
	confirmation: 'noninteractive-allowed',
	retry: 'same-request-while-retained',
	input: { name: 'workbench.terminal-create-input', schemaVersion: 1 },
	output: { name: 'workbench.terminal-create-output', schemaVersion: 1 },
} as const;

const activeExecution = {
	protocolVersion: SEMANTIC_COMMAND_PROTOCOL_VERSION,
	coordinatorGeneration: 'desktop:generation:1',
	executionIdentity: 'desktop:execution:1',
	requestIdentity: 'client:request:1',
	operationId: descriptor.id,
	operationSchemaVersion: descriptor.schemaVersion,
	phase: 'active',
	cancellationRequested: false,
} as const;

describe('semantic command protocol', () => {
	it('accepts one strict descriptor and keeps executable policy private', () => {
		expect(SemanticCommandOperationDescriptorSchema.parse(descriptor)).toEqual(descriptor);
		expect(
			SemanticCommandOperationDescriptorSchema.safeParse({ ...descriptor, handler: 'hidden' })
				.success,
		).toBe(false);
		expect(
			SemanticCommandOperationDescriptorSchema.safeParse({
				...descriptor,
				id: 'terminal.create',
			}).success,
		).toBe(false);
	});

	it('separates protocol and operation schema versions in a strict JSON request', () => {
		const request = {
			protocolVersion: 1,
			requestIdentity: 'client:request:1',
			operationId: descriptor.id,
			operationSchemaVersion: 1,
			input: { directory: null },
		};

		expect(SemanticCommandInvocationRequestSchema.parse(request)).toEqual(request);
		expect(
			SemanticCommandInvocationRequestSchema.safeParse({ ...request, protocolVersion: 2 }).success,
		).toBe(false);
		expect(
			SemanticCommandInvocationRequestSchema.safeParse({ ...request, unknown: true }).success,
		).toBe(false);
	});

	it('rejects non-JSON numbers and values', () => {
		for (const invalid of [Number.NaN, Number.POSITIVE_INFINITY, undefined, 1n]) {
			expect(SemanticCommandJsonValueSchema.safeParse(invalid).success).toBe(false);
		}
	});

	it('makes active and terminal execution fields mutually exclusive', () => {
		expect(SemanticCommandExecutionSnapshotSchema.parse(activeExecution)).toEqual(activeExecution);
		expect(
			SemanticCommandExecutionSnapshotSchema.safeParse({
				...activeExecution,
				result: { kind: 'failed', code: 'operation-failed' },
			}).success,
		).toBe(false);

		const terminal = {
			...activeExecution,
			phase: 'terminal',
			result: { kind: 'completed', code: 'terminal-created', output: { revision: '2' } },
		};
		Reflect.deleteProperty(terminal, 'cancellationRequested');
		expect(SemanticCommandExecutionSnapshotSchema.safeParse(terminal).success).toBe(true);
	});

	it('requires safe output only for successful terminal results', () => {
		expect(
			SemanticCommandTerminalResultSchema.safeParse({
				kind: 'completed',
				code: 'operation-completed',
			}).success,
		).toBe(false);
		expect(
			SemanticCommandTerminalResultSchema.safeParse({
				kind: 'failed',
				code: 'operation-failed',
				output: { private: true },
			}).success,
		).toBe(false);
	});

	it('validates discovery, admission, lookup, and cancellation documents', () => {
		expect(
			SemanticCommandOperationListSchema.safeParse({
				protocolVersion: 1,
				coordinatorGeneration: 'desktop:generation:1',
				kind: 'operation-list',
				operations: [descriptor],
			}).success,
		).toBe(true);
		expect(
			SemanticCommandOperationDescriptionResultSchema.safeParse({
				protocolVersion: 1,
				coordinatorGeneration: 'desktop:generation:1',
				kind: 'operation-unavailable',
				operationId: descriptor.id,
			}).success,
		).toBe(true);
		expect(
			SemanticCommandInvocationResultSchema.safeParse({
				kind: 'accepted',
				replayed: false,
				execution: activeExecution,
			}).success,
		).toBe(true);
		expect(
			SemanticCommandExecutionLookupResultSchema.safeParse({
				kind: 'found',
				execution: activeExecution,
			}).success,
		).toBe(true);
		expect(
			SemanticCommandExecutionCancellationResultSchema.safeParse({
				kind: 'cancellation-observed',
				forwarded: true,
				execution: { ...activeExecution, cancellationRequested: true },
			}).success,
		).toBe(true);
	});
});
