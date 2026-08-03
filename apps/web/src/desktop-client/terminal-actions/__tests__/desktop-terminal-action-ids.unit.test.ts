import { describe, expect, it } from 'vitest';

import { desktopTerminalLinkActionInput } from '../desktop-terminal-action-ids';

describe('desktopTerminalLinkActionInput', () => {
	it('accepts bounded URL and file targets without widening their shape', () => {
		expect(
			desktopTerminalLinkActionInput.parse({
				target: { kind: 'url', url: 'https://orchbee.dev/docs' },
			}),
		).toEqual({
			data: { target: { kind: 'url', url: 'https://orchbee.dev/docs' } },
			success: true,
		});
		expect(
			desktopTerminalLinkActionInput.parse({
				target: { column: 3, kind: 'file', line: 7, path: '/tmp/example.ts' },
			}),
		).toEqual({
			data: {
				target: { column: 3, kind: 'file', line: 7, path: '/tmp/example.ts' },
			},
			success: true,
		});
	});

	it('rejects missing, unbounded, and malformed targets', () => {
		for (const input of [
			null,
			{},
			{ target: { kind: 'url', url: '' } },
			{ target: { kind: 'file', line: 0, path: '/tmp/example.ts' } },
			{ target: { kind: 'unknown', value: 'x'.repeat(16_385) } },
		]) {
			expect(desktopTerminalLinkActionInput.parse(input)).toEqual({
				code: 'invalid-terminal-link-target',
				success: false,
			});
		}
	});
});
