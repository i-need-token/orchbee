import { describe, expect, it } from 'vitest';

import {
	TerminalPresentationFailureSchema,
	TerminalPresentationIdSchema,
	TerminalPresentationOperationIdSchema,
	TerminalPresentationRemovalIdSchema,
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '../terminal-presentation-contracts';

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
		expect(TerminalPresentationFailureSchema.safeParse({ code: '', retryable: true }).success).toBe(
			false,
		);
		expect(TerminalPresentationFailureSchema.safeParse({ code: 'host-unavailable' }).success).toBe(
			false,
		);
	});
});
