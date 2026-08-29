import {
	SemanticCommandCoordinatorGenerationSchema,
	SemanticCommandExecutionIdentitySchema,
	SemanticCommandExecutionLookupResultSchema,
	SemanticCommandInvocationRequestSchema,
	SemanticCommandInvocationResultSchema,
	SemanticCommandOperationIdSchema,
	SemanticCommandTerminalResultSchema,
} from '@orchbee/semantic-command-protocol';
import {
	decodeSemanticCommandLocalHttpResponse,
	semanticCommandLocalHttpRequests,
} from '@orchbee/semantic-command-protocol/local-http';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import type { DesktopTerminalOutputReader } from '../../terminal-output-read';
import { DesktopSemanticCommandCatalog } from '../DesktopSemanticCommandCatalog';
import { DesktopSemanticCommandService } from '../DesktopSemanticCommandService';
import { createDesktopTerminalOutputReadOperation } from '../createDesktopTerminalOutputReadOperation';
import { handleDesktopSemanticCommandLocalHttpRequest } from '../handleDesktopSemanticCommandLocalHttpRequest';

const generation = SemanticCommandCoordinatorGenerationSchema.parse('desktop:generation:output');
const presentationId = terminalPresentationId('presentation-semantic-output');
const output = {
	activeScreen: 'primary',
	format: 'plain-text',
	generation: '2',
	presentationId,
	requestedRows: 80,
	returnedRows: 2,
	revision: '7',
	source: 'tail-unwrapped',
	startRow: '8',
	text: 'one\ntwo',
	totalRows: '10',
	truncated: true,
} as const;

describe('Desktop Terminal output read operation', () => {
	it('admits the owner descriptor and returns the reader projection without an effect', async () => {
		const read = vi
			.fn<DesktopTerminalOutputReader['read']>()
			.mockResolvedValue({ kind: 'completed', output });
		const service = createService(read);

		expect(service.list({ kind: 'local-user' }).operations).toEqual([
			expect.objectContaining({
				confirmation: 'noninteractive-allowed',
				id: 'workbench.terminal.output.read',
				kind: 'query',
			}),
		]);
		await expect(invokeAndRead(service, 1, { presentationId })).resolves.toEqual({
			code: 'terminal-output-read',
			kind: 'completed',
			output,
		});
		expect(read).toHaveBeenCalledWith(presentationId, 80, expect.any(AbortSignal));
	});

	it('fails closed for invalid input and maps stale, retirement, availability, and cleanup distinctly', async () => {
		const invalid = createService(vi.fn<DesktopTerminalOutputReader['read']>());
		expect(
			invalid.invoke({ kind: 'local-user' }, request(1, { presentationId, rows: 1_001 })),
		).toMatchObject({ code: 'input-invalid', kind: 'rejected' });

		for (const [index, readerKind, expected] of [
			[2, 'stale', { code: 'terminal-output-read-stale', kind: 'blocked' }],
			[3, 'target-retired', { code: 'terminal-output-target-retired', kind: 'blocked' }],
			[4, 'unavailable', { code: 'terminal-output-read-unavailable', kind: 'failed' }],
			[5, 'cleanup-failed', { code: 'terminal-output-read-cleanup-failed', kind: 'failed' }],
		] as const) {
			const read = vi
				.fn<DesktopTerminalOutputReader['read']>()
				.mockResolvedValue({ kind: readerKind });
			await expect(invokeAndRead(createService(read), index, { presentationId })).resolves.toEqual(
				expected,
			);
		}
	});

	it('serves the exact bounded projection through the strict local HTTP boundary', async () => {
		const read = vi
			.fn<DesktopTerminalOutputReader['read']>()
			.mockResolvedValue({ kind: 'completed', output: { ...output, requestedRows: 2 } });
		const service = createService(read);
		const invocation = request(6, { presentationId, rows: 2 });
		const admitted = SemanticCommandInvocationResultSchema.parse(
			localExchange(service, semanticCommandLocalHttpRequests.invoke(invocation)),
		);
		expect(admitted.kind).toBe('accepted');
		await vi.waitFor(() => {
			const observed = SemanticCommandExecutionLookupResultSchema.parse(
				localExchange(
					service,
					semanticCommandLocalHttpRequests.getExecution({
						kind: 'request',
						requestIdentity: invocation.requestIdentity,
					}),
				),
			);
			if (observed.kind !== 'found' || observed.execution.phase !== 'terminal') {
				throw new Error('Terminal output read has not settled yet.');
			}
			expect(observed.execution.result).toEqual({
				code: 'terminal-output-read',
				kind: 'completed',
				output: { ...output, requestedRows: 2 },
			});
		});
		expect(read).toHaveBeenCalledWith(presentationId, 2, expect.any(AbortSignal));
	});
});

function createService(read: DesktopTerminalOutputReader['read']): DesktopSemanticCommandService {
	return new DesktopSemanticCommandService(
		new DesktopSemanticCommandCatalog([
			createDesktopTerminalOutputReadOperation({ reader: { read } }),
		]),
		{
			coordinatorGeneration: generation,
			createExecutionIdentity: () =>
				SemanticCommandExecutionIdentitySchema.parse('desktop:execution:output'),
		},
	);
}

function request(index: number, input: unknown) {
	return SemanticCommandInvocationRequestSchema.parse({
		input,
		operationId: SemanticCommandOperationIdSchema.parse('workbench.terminal.output.read'),
		operationSchemaVersion: 1,
		protocolVersion: 1,
		requestIdentity: `cli:output:${index}`,
	});
}

async function invokeAndRead(
	service: DesktopSemanticCommandService,
	index: number,
	input: unknown,
) {
	const admission = service.invoke({ kind: 'local-user' }, request(index, input));
	if (admission.kind !== 'accepted') return admission;
	for (let attempt = 0; attempt < 10; attempt += 1) {
		await Promise.resolve();
		const observed = service.get({
			executionIdentity: admission.execution.executionIdentity,
			kind: 'execution',
		});
		if (observed.kind === 'found' && observed.execution.phase === 'terminal') {
			return SemanticCommandTerminalResultSchema.parse(observed.execution.result);
		}
	}
	throw new Error('Expected one terminal output execution.');
}

function localExchange(service: DesktopSemanticCommandService, requestBytes: Uint8Array): unknown {
	const response = decodeSemanticCommandLocalHttpResponse(
		handleDesktopSemanticCommandLocalHttpRequest(service, requestBytes),
	);
	expect(response.status).toBe(200);
	return response.body;
}
