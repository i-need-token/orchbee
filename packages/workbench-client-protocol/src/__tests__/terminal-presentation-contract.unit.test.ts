import { describe, expect, it } from 'vitest';

import {
	TerminalPresentationIdSchema,
	terminalPresentationId,
} from '../terminal-presentation-contract';

describe('terminal presentation contracts', () => {
	it('accepts only bounded opaque presentation identities', () => {
		expect(terminalPresentationId('presentation:desktop:1')).toBe('presentation:desktop:1');
		expect(TerminalPresentationIdSchema.safeParse(' contains spaces ').success).toBe(false);
		expect(TerminalPresentationIdSchema.safeParse('').success).toBe(false);
		expect(TerminalPresentationIdSchema.safeParse('x'.repeat(129)).success).toBe(false);
	});
});
