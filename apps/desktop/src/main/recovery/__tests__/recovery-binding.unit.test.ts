import { CreationIdSchema, HostEpochSchema, TerminalIdSchema } from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import { createDefaultTerminalProfile } from '../../host';
import { TerminalRecoveryLedgerDocumentSchema } from '../recovery-binding';

describe('terminal recovery binding schemas', () => {
	it('accepts one canonical versioned retained binding', () => {
		const identity = {
			creationId: CreationIdSchema.parse('11'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('22'.repeat(16)),
			terminalId: TerminalIdSchema.parse('33'.repeat(16)),
		};
		const document = {
			binding: {
				bindingId: 'desktop-terminal-foundation',
				creationId: identity.creationId,
				launch: createLaunch(),
				state: { identity, kind: 'retained' },
			},
			version: 1,
		};

		expect(TerminalRecoveryLedgerDocumentSchema.parse(document)).toEqual(document);
	});

	it('rejects an unknown version and non-canonical binding identity', () => {
		expect(
			TerminalRecoveryLedgerDocumentSchema.safeParse({ binding: {}, version: 2 }).success,
		).toBe(false);
		expect(
			TerminalRecoveryLedgerDocumentSchema.safeParse({
				binding: {
					bindingId: '../terminal',
					creationId: '11'.repeat(16),
					launch: createLaunch(),
					state: { kind: 'planned' },
				},
				version: 1,
			}).success,
		).toBe(false);
	});
});

function createLaunch() {
	return {
		arguments: [],
		cellPixelHeight: 16,
		cellPixelWidth: 8,
		columns: 80,
		currentDirectory: '/users/orchbee',
		environment: [{ name: 'TERM', value: 'xterm-256color' }],
		executable: '/bin/sh',
		rows: 24,
		terminalProfile: createDefaultTerminalProfile(),
	};
}
