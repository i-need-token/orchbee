import { describe, expect, it } from 'vitest';

import {
	TerminalPresentationFailureSchema,
	TerminalPresentationEventSchema,
	TerminalPresentationIdSchema,
	TerminalPresentationOperationIdSchema,
	TerminalPresentationRemovalIdSchema,
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '../terminal-presentation-contract';

describe('terminal presentation contracts', () => {
	it('accepts bounded opaque identities for each separate identity domain', () => {
		expect(terminalPresentationId('presentation:desktop:1')).toBe('presentation:desktop:1');
		expect(terminalPresentationOperationId('operation:provision:1')).toBe('operation:provision:1');
		expect(terminalPresentationRemovalId('removal:panel:1')).toBe('removal:panel:1');
		expect(TerminalPresentationIdSchema.safeParse(' contains spaces ').success).toBe(false);
		expect(TerminalPresentationOperationIdSchema.safeParse('').success).toBe(false);
		expect(TerminalPresentationRemovalIdSchema.safeParse('x'.repeat(129)).success).toBe(false);
	});

	it('normalizes failures to a bounded code and explicit retry policy', () => {
		expect(
			TerminalPresentationFailureSchema.parse({ code: 'host-unavailable', retryable: true }),
		).toEqual({ code: 'host-unavailable', retryable: true });
		expect(
			TerminalPresentationFailureSchema.safeParse({
				code: 'host-unavailable',
				retryable: true,
				unexpected: true,
			}).success,
		).toBe(false);
		expect(TerminalPresentationFailureSchema.safeParse({ code: '', retryable: true }).success).toBe(
			false,
		);
		expect(TerminalPresentationFailureSchema.safeParse({ code: 'host-unavailable' }).success).toBe(
			false,
		);
	});

	it('parses every terminal observation and rejects commands or ambiguous exits', () => {
		const operationId = terminalPresentationOperationId('operation-1');
		const presentationId = terminalPresentationId('presentation-1');
		const events = [
			{ operationId, presentationId, type: 'terminal-provision-completed' },
			{ operationId, presentationId, type: 'terminal-restore-started' },
			{ operationId, presentationId, type: 'terminal-restore-completed' },
			{ operationId, presentationId, type: 'terminal-close-completed' },
			{ exitCode: 0, presentationId, signal: null, type: 'terminal-exited' },
			{
				failure: { code: 'terminal-runtime-pty-read-read-output', retryable: false },
				presentationId,
				type: 'terminal-failed',
			},
		] as const;

		expect(events.every((event) => TerminalPresentationEventSchema.safeParse(event).success)).toBe(
			true,
		);
		expect(
			TerminalPresentationEventSchema.safeParse({
				exitCode: 1,
				presentationId,
				signal: 'SIGTERM',
				type: 'terminal-exited',
			}).success,
		).toBe(false);
		expect(
			TerminalPresentationEventSchema.safeParse({
				operationId,
				presentationId,
				type: 'terminal-provision-requested',
			}).success,
		).toBe(false);
	});
});
