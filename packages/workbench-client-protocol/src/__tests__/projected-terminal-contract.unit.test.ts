import { describe, expect, it } from 'vitest';

import {
	ProjectedTerminalCommandSchema,
	ProjectedTerminalEventSchema,
} from '../projected-terminal-contract';

const attachmentId = '11'.repeat(16);

describe('projected terminal contract', () => {
	it('keeps commands scoped to their attachment identity', () => {
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				kind: 'selection-copy',
				payload: { attachmentId, requestSequence: '1' },
			}).success,
		).toBe(true);
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				kind: 'selection-copy',
				payload: { requestSequence: '1' },
			}).success,
		).toBe(false);
	});

	it('preserves binary frame payloads without JSON representation', () => {
		const frame = new ArrayBuffer(32);
		const parsed = ProjectedTerminalEventSchema.parse({ frame, kind: 'frame' });

		expect(parsed.kind).toBe('frame');
		if (parsed.kind !== 'frame') throw new Error('Expected a projected terminal frame event.');
		expect(parsed.frame).toBe(frame);

		const rows = new Uint8Array(new ArrayBuffer(48), 8, 32);
		const parsedRows = ProjectedTerminalEventSchema.parse({ kind: 'rows', rows });
		expect(parsedRows).toMatchObject({ kind: 'rows', rows });
	});

	it('bounds Browser-owned selection copy content', () => {
		const content = new ArrayBuffer(32);

		expect(
			ProjectedTerminalEventSchema.parse({
				content,
				kind: 'selection-copy-content',
				requestSequence: '1',
			}),
		).toMatchObject({ content, kind: 'selection-copy-content', requestSequence: '1' });
	});
});
