import { describe, expect, it } from 'vitest';

import {
	DesktopTerminalDefaultSettingsMainEventSchema,
	DesktopTerminalDefaultSettingsRendererCommandSchema,
} from '../desktop-terminal-default-settings-contract';

describe('Desktop terminal-default settings contract', () => {
	it('parses correlated snapshot and update messages', () => {
		const command = DesktopTerminalDefaultSettingsRendererCommandSchema.parse({
			configuration: { scrollbackBytes: 25_000_000 },
			requestId: 'terminal-defaults:request:1',
			type: 'desktop-terminal-default-settings-update-requested',
		});
		if (command.type !== 'desktop-terminal-default-settings-update-requested') {
			throw new Error('Expected the parsed command to retain its update discriminator.');
		}

		expect(
			DesktopTerminalDefaultSettingsMainEventSchema.parse({
				requestId: command.requestId,
				result: {
					kind: 'completed',
					snapshot: {
						configuration: command.configuration,
						persistenceIssue: null,
						source: 'stored',
					},
				},
				type: 'desktop-terminal-default-settings-responded',
			}),
		).toMatchObject({ result: { kind: 'completed', snapshot: { source: 'stored' } } });
	});

	it('rejects out-of-range settings and unknown fields', () => {
		expect(
			DesktopTerminalDefaultSettingsRendererCommandSchema.safeParse({
				configuration: { scrollbackBytes: 1_000_000_001 },
				requestId: 'terminal-defaults:request:2',
				type: 'desktop-terminal-default-settings-update-requested',
			}).success,
		).toBe(false);
		expect(
			DesktopTerminalDefaultSettingsRendererCommandSchema.safeParse({
				requestId: 'terminal-defaults:request:3',
				type: 'desktop-terminal-default-settings-snapshot-requested',
				unknown: true,
			}).success,
		).toBe(false);
	});
});
