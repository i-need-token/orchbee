import { PROTOCOL_LIMITS } from '@orchbee/terminal-protocol';
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

	it('bounds link targets by their UTF-8 representation', () => {
		const prefix = 'https://example.com/';
		const exactAsciiTarget = `${prefix}${'a'.repeat(
			PROTOCOL_LIMITS.maxStringBytes - prefix.length,
		)}`;
		const target = `https://example.com/${'界'.repeat(
			Math.ceil(PROTOCOL_LIMITS.maxStringBytes / 3),
		)}`;
		expect(new TextEncoder().encode(exactAsciiTarget).byteLength).toBe(
			PROTOCOL_LIMITS.maxStringBytes,
		);
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				kind: 'link-open',
				payload: { attachmentId, requestSequence: '1', target: exactAsciiTarget },
			}).success,
		).toBe(true);
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				kind: 'link-open',
				payload: { attachmentId, requestSequence: '1', target: `${exactAsciiTarget}a` },
			}).success,
		).toBe(false);
		expect(target.length).toBeLessThan(PROTOCOL_LIMITS.maxStringBytes);
		expect(new TextEncoder().encode(target).byteLength).toBeGreaterThan(
			PROTOCOL_LIMITS.maxStringBytes,
		);
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				kind: 'link-open',
				payload: { attachmentId, requestSequence: '1', target },
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
