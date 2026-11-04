import { describe, expect, it } from 'vitest';

import {
	TerminalPresentationIdSchema,
	TerminalPresentationOperationIdSchema,
	TerminalPresentationRemovalIdSchema,
	terminalPresentationId,
	terminalPresentationOperationId,
} from '../terminal-presentation-contract';

describe('terminal presentation contracts', () => {
	it('accepts bounded opaque identities for each separate identity domain', () => {
		expect(terminalPresentationId('presentation:desktop:1')).toBe('presentation:desktop:1');
		expect(terminalPresentationOperationId('operation:provision:1')).toBe('operation:provision:1');
		expect(TerminalPresentationIdSchema.safeParse(' contains spaces ').success).toBe(false);
		expect(TerminalPresentationOperationIdSchema.safeParse('').success).toBe(false);
		expect(TerminalPresentationRemovalIdSchema.safeParse('x'.repeat(129)).success).toBe(false);
	});
});
