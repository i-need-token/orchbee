import { PROTOCOL_LIMITS } from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import {
	TERMINAL_DOCUMENT_CONTROL_IPC_CHANNEL,
	TerminalDocumentControlBootstrapSchema,
	TerminalDocumentControlCommandSchema,
	TerminalDocumentControlEventSchema,
	TerminalDocumentControlWindowMessageSchema,
} from '../terminal-document-control-contract';

describe('terminal document control contract', () => {
	it('accepts only presentation-scoped provision and close commands', () => {
		expect(
			TerminalDocumentControlCommandSchema.parse({
				launchCwd: '/workspace/orchbee',
				operationId: 'operation-1',
				presentationId: 'presentation-1',
				type: 'terminal-provision-requested',
			}),
		).toEqual({
			launchCwd: '/workspace/orchbee',
			operationId: 'operation-1',
			presentationId: 'presentation-1',
			type: 'terminal-provision-requested',
		});
		expect(
			TerminalDocumentControlCommandSchema.parse({
				operationId: 'operation-2',
				presentationId: 'presentation-1',
				type: 'terminal-close-requested',
			}),
		).toMatchObject({ type: 'terminal-close-requested' });
	});

	it('rejects privileged identities, generic operations, and oversized UTF-8 launch paths', () => {
		expect(
			TerminalDocumentControlCommandSchema.safeParse({
				bindingId: 'binding-1',
				launchCwd: null,
				operationId: 'operation-1',
				presentationId: 'presentation-1',
				type: 'terminal-provision-requested',
			}).success,
		).toBe(false);
		expect(
			TerminalDocumentControlCommandSchema.safeParse({
				channel: 'arbitrary-ipc',
				operationId: 'operation-1',
				presentationId: 'presentation-1',
				type: 'send',
			}).success,
		).toBe(false);
		expect(
			TerminalDocumentControlCommandSchema.safeParse({
				launchCwd: '界'.repeat(Math.floor(PROTOCOL_LIMITS.maxStringBytes / 2)),
				operationId: 'operation-1',
				presentationId: 'presentation-1',
				type: 'terminal-provision-requested',
			}).success,
		).toBe(false);
	});

	it('keeps operation results correlated without exposing Host identity', () => {
		expect(
			TerminalDocumentControlEventSchema.parse({
				operationId: 'operation-1',
				presentationId: 'presentation-1',
				type: 'terminal-provision-completed',
			}),
		).toMatchObject({ operationId: 'operation-1', presentationId: 'presentation-1' });
		expect(
			TerminalDocumentControlEventSchema.safeParse({
				hostEpoch: 'epoch-1',
				operationId: 'operation-1',
				presentationId: 'presentation-1',
				terminalId: 'terminal-1',
				type: 'terminal-provision-completed',
			}).success,
		).toBe(false);
	});

	it('represents normal, signal, and unknown exit without impossible combinations', () => {
		for (const exit of [
			{ exitCode: 0, signal: null },
			{ exitCode: null, signal: 'SIGTERM' },
			{ exitCode: null, signal: null },
		]) {
			expect(
				TerminalDocumentControlEventSchema.safeParse({
					...exit,
					presentationId: 'presentation-1',
					type: 'terminal-exited',
				}).success,
			).toBe(true);
		}
		expect(
			TerminalDocumentControlEventSchema.safeParse({
				exitCode: 1,
				presentationId: 'presentation-1',
				signal: 'SIGTERM',
				type: 'terminal-exited',
			}).success,
		).toBe(false);
	});

	it('requires the preload security attestation and exact ready message', () => {
		expect(
			TerminalDocumentControlBootstrapSchema.safeParse({
				type: TERMINAL_DOCUMENT_CONTROL_IPC_CHANNEL,
			}).success,
		).toBe(true);
		expect(TerminalDocumentControlBootstrapSchema.safeParse({ type: 'generic-ipc' }).success).toBe(
			false,
		);
		expect(
			TerminalDocumentControlWindowMessageSchema.safeParse({
				security: { contextIsolated: true, sandboxed: true },
				type: 'orchbee.desktop.terminal-document-control-ready',
			}).success,
		).toBe(true);
		expect(
			TerminalDocumentControlWindowMessageSchema.safeParse({
				security: { contextIsolated: true, sandboxed: true },
				type: 'generic-ipc-ready',
			}).success,
		).toBe(false);
	});
});
