import { PROTOCOL_LIMITS } from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import {
	DesktopClipboardMainEventSchema,
	DesktopClipboardRendererCommandSchema,
} from '../desktop-clipboard-contract';

describe('Desktop clipboard contract', () => {
	it('carries one correlated bounded plain-text read', () => {
		const command = DesktopClipboardRendererCommandSchema.parse({
			requestId: 'clipboard:1',
			type: 'desktop-clipboard-read-text-requested',
		});

		expect(
			DesktopClipboardMainEventSchema.parse({
				requestId: command.requestId,
				result: { kind: 'completed', text: 'selected 中文' },
				type: 'desktop-clipboard-read-text-responded',
			}),
		).toMatchObject({ result: { kind: 'completed', text: 'selected 中文' } });
	});

	it('rejects unknown commands and text outside the terminal input boundary', () => {
		expect(
			DesktopClipboardRendererCommandSchema.safeParse({
				requestId: 'clipboard:1',
				type: 'desktop-clipboard-write-text-requested',
			}).success,
		).toBe(false);
		expect(
			DesktopClipboardMainEventSchema.safeParse({
				requestId: 'clipboard:1',
				result: { kind: 'completed', text: 'a'.repeat(PROTOCOL_LIMITS.maxInputBytes + 1) },
				type: 'desktop-clipboard-read-text-responded',
			}).success,
		).toBe(false);
		expect(
			DesktopClipboardMainEventSchema.safeParse({
				requestId: 'clipboard:1',
				result: { kind: 'completed', text: '\ud800' },
				type: 'desktop-clipboard-read-text-responded',
			}).success,
		).toBe(false);
	});
});
