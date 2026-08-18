import { describe, expect, it } from 'vitest';

import {
	TerminalProjectionControlCommandSchema,
	TerminalProjectionControlEventSchema,
} from '../terminal-projection-control-contract';

describe('terminal projection control contract', () => {
	it('admits only presentation-scoped open and release commands', () => {
		expect(
			TerminalProjectionControlCommandSchema.parse({
				presentationId: 'presentation-1',
				requestId: 'projection-1',
				type: 'terminal-projection-open-requested',
			}),
		).toMatchObject({ presentationId: 'presentation-1', requestId: 'projection-1' });
		expect(
			TerminalProjectionControlCommandSchema.safeParse({
				presentationId: 'presentation-1',
				requestId: 'projection-1',
				terminalId: '11'.repeat(16),
				type: 'terminal-projection-open-requested',
			}).success,
		).toBe(false);
	});

	it('requires a complete scoped attachment bootstrap for an opened projection', () => {
		expect(
			TerminalProjectionControlEventSchema.safeParse({
				bootstrap: {
					attachmentId: '11'.repeat(16),
					canInput: true,
					canResize: true,
					creationId: '22'.repeat(16),
					hostEpoch: '33'.repeat(16),
					terminalId: '44'.repeat(16),
					terminalOutcome: null,
				},
				presentationId: 'presentation-1',
				requestId: 'projection-1',
				type: 'terminal-projection-opened',
			}).success,
		).toBe(true);
		expect(
			TerminalProjectionControlEventSchema.safeParse({
				presentationId: 'presentation-1',
				requestId: 'projection-1',
				type: 'terminal-projection-opened',
			}).success,
		).toBe(false);
	});

	it('correlates successful release completion without an attachment capability', () => {
		expect(
			TerminalProjectionControlEventSchema.parse({
				presentationId: 'presentation-1',
				requestId: 'projection-1',
				type: 'terminal-projection-released',
			}),
		).toEqual({
			presentationId: 'presentation-1',
			requestId: 'projection-1',
			type: 'terminal-projection-released',
		});
	});
});
