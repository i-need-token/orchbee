import { describe, expect, it } from 'vitest';

import {
	DesktopBrowserAccessMainEventSchema,
	DesktopBrowserAccessRendererCommandSchema,
	DesktopBrowserAccessSnapshotSchema,
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
				snapshot: { access: { phase: 'stopped' }, personalAccessToken: 'missing' },
				type: 'desktop-browser-access-failed',
			}),
		).toMatchObject({
			code: 'network-address-unavailable',
			snapshot: { access: { phase: 'stopped' }, personalAccessToken: 'missing' },
		});
		expect(
			DesktopBrowserAccessMainEventSchema.safeParse({
				code: 'raw-node-error',
				requestId: 'browser-access:request:2',
				snapshot: { access: { phase: 'stopped' }, personalAccessToken: 'missing' },
				type: 'desktop-browser-access-failed',
			}).success,
		).toBe(false);
	});

	it('delivers a newly issued Personal Access Token only in its correlated event', () => {
		const snapshot = DesktopBrowserAccessSnapshotSchema.parse({
			access: { phase: 'stopped' },
			personalAccessToken: 'configured',
		});
		const event = DesktopBrowserAccessMainEventSchema.parse({
			personalAccessToken: 't'.repeat(43),
			requestId: 'browser-access:request:3',
			snapshot,
			type: 'desktop-browser-access-personal-access-token-issued',
		});

		expect(event).toMatchObject({ personalAccessToken: 't'.repeat(43), snapshot });
		expect(
			DesktopBrowserAccessRendererCommandSchema.parse({
				requestId: 'browser-access:request:3',
				type: 'desktop-browser-access-personal-access-token-issue-requested',
			}),
		).toBeDefined();
	});
});
