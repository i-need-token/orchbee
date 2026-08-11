import { describe, expect, it } from 'vitest';

import {
	DesktopBrowserAccessMainEventSchema,
	DesktopBrowserAccessRendererCommandSchema,
	DesktopBrowserAccessStateSchema,
} from '../desktop-browser-access-contract';

describe('Desktop Browser access contract', () => {
	it('projects ready state without a Gateway capability or listener address', () => {
		const state = DesktopBrowserAccessStateSchema.parse({
			localOrigin: 'http://127.0.0.1:43110',
			mode: 'local-network',
			networkOrigins: ['http://192.168.1.14:43110'],
			phase: 'ready',
		});

		expect(state).toEqual({
			localOrigin: 'http://127.0.0.1:43110',
			mode: 'local-network',
			networkOrigins: ['http://192.168.1.14:43110'],
			phase: 'ready',
		});
		expect(JSON.stringify(state)).not.toContain('capability');
		expect(JSON.stringify(state)).not.toContain('0.0.0.0');
	});

	it('rejects non-origin, credential-bearing, and oversized renderer inputs', () => {
		for (const origin of [
			'http://192.168.1.14:43110/path',
			'http://user:password@192.168.1.14:43110',
			'https://192.168.1.14:43110',
		]) {
			expect(
				DesktopBrowserAccessRendererCommandSchema.safeParse({
					origin,
					requestId: 'browser-access:request:1',
					type: 'desktop-browser-access-copy-link-requested',
				}).success,
			).toBe(false);
		}
		expect(
			DesktopBrowserAccessStateSchema.safeParse({
				localOrigin: 'http://127.0.0.1:43110',
				mode: 'local-network',
				networkOrigins: Array.from(
					{ length: 33 },
					(_, index) => `http://192.168.1.${index + 1}:43110`,
				),
				phase: 'ready',
			}).success,
		).toBe(false);
	});

	it('requires typed request correlation and bounded failure codes', () => {
		expect(
			DesktopBrowserAccessMainEventSchema.parse({
				code: 'network-address-unavailable',
				requestId: 'browser-access:request:2',
				state: { phase: 'stopped' },
				type: 'desktop-browser-access-failed',
			}),
		).toMatchObject({ code: 'network-address-unavailable', state: { phase: 'stopped' } });
		expect(
			DesktopBrowserAccessMainEventSchema.safeParse({
				code: 'raw-node-error',
				requestId: 'browser-access:request:2',
				state: { phase: 'stopped' },
				type: 'desktop-browser-access-failed',
			}).success,
		).toBe(false);
	});
});
