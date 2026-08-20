import { describe, expect, it } from 'vitest';

import { projectedTerminalLinkActionInput } from '../projected-terminal-action-ids';

describe('projectedTerminalLinkActionInput', () => {
	it('accepts bounded URL and file targets without widening their shape', () => {
		expect(
			projectedTerminalLinkActionInput.parse({
				target: { kind: 'url', url: 'https://orchbee.dev/docs' },
			}),
		).toEqual({
			data: { target: { kind: 'url', url: 'https://orchbee.dev/docs' } },
			success: true,
		});
		expect(
			projectedTerminalLinkActionInput.parse({
				target: { column: 3, kind: 'file', line: 7, path: '/tmp/example.ts' },
			}),
		).toEqual({
			data: {
				target: { column: 3, kind: 'file', line: 7, path: '/tmp/example.ts' },
			},
			success: true,
		});
	});

	it('copies canonical revision-pinned link context', () => {
		const input = {
			context: {
				column: 2,
				generation: '3',
				linkEpoch: '4',
				revision: '5',
				row: '6',
			},
			target: { kind: 'url', url: 'https://orchbee.dev/docs' },
		};
		const result = projectedTerminalLinkActionInput.parse(input);

		expect(result).toEqual({ data: input, success: true });
		input.context.revision = '7';
		expect(result).toMatchObject({ data: { context: { revision: '5' } } });
	});

	it('rejects missing, unbounded, and malformed targets', () => {
		for (const input of [
			null,
			{},
			{ target: { kind: 'url', url: '' } },
			{ target: { kind: 'file', line: 0, path: '/tmp/example.ts' } },
			{ target: { kind: 'unknown', value: 'x'.repeat(16_385) } },
			{
				context: { column: 0, generation: '01', linkEpoch: '1', revision: '1', row: '1' },
				target: { kind: 'url', url: 'https://orchbee.dev/docs' },
			},
		]) {
			expect(projectedTerminalLinkActionInput.parse(input)).toEqual({
				code: 'invalid-terminal-link-target',
				success: false,
			});
		}
	});
});
