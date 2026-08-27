// @vitest-environment jsdom

import {
	DesktopBrowserAccessMainEventSchema,
	DesktopBrowserAccessRendererCommandSchema,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it } from 'vitest';

import { DesktopBrowserAccessClient } from '../DesktopBrowserAccessClient';

describe('DesktopBrowserAccessClient', () => {
	it('correlates sanitized states without exposing Browser authentication secrets', async () => {
		const channel = new MessageChannel();
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopBrowserAccessRendererCommandSchema.parse(event.data);
			channel.port2.postMessage(
				DesktopBrowserAccessMainEventSchema.parse({
					requestId: command.requestId,
					snapshot: {
						access: {
							localOrigin: 'http://127.0.0.1:43110',
							mode: 'loopback',
							networkOrigins: [],
							phase: 'ready',
						},
						personalAccessToken: 'configured',
					},
					type: 'desktop-browser-access-completed',
				}),
			);
		});
		channel.port2.start();
		const client = new DesktopBrowserAccessClient(channel.port1, () => 'request');

		const snapshot = await client.start('loopback');

		expect(snapshot).toEqual({
			access: {
				localOrigin: 'http://127.0.0.1:43110',
				mode: 'loopback',
				networkOrigins: [],
				phase: 'ready',
			},
			personalAccessToken: 'configured',
		});
		expect(JSON.stringify(snapshot)).not.toContain('#');
		client.dispose();
		channel.port2.close();
	});

	it('rejects bounded Main failures with the returned current state', async () => {
		const channel = new MessageChannel();
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopBrowserAccessRendererCommandSchema.parse(event.data);
			channel.port2.postMessage(
				DesktopBrowserAccessMainEventSchema.parse({
					code: 'network-address-unavailable',
					requestId: command.requestId,
					snapshot: {
						access: { phase: 'stopped' },
						personalAccessToken: 'configured',
					},
					type: 'desktop-browser-access-failed',
				}),
			);
		});
		channel.port2.start();
		const client = new DesktopBrowserAccessClient(channel.port1, () => 'request');

		await expect(client.start('local-network')).rejects.toEqual(
			expect.objectContaining({
				code: 'network-address-unavailable',
				snapshot: {
					access: { phase: 'stopped' },
					personalAccessToken: 'configured',
				},
			}),
		);
		client.dispose();
		channel.port2.close();
	});

	it('delivers a newly issued Personal Access Token only to the matching one-time request', async () => {
		const channel = new MessageChannel();
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopBrowserAccessRendererCommandSchema.parse(event.data);
			channel.port2.postMessage(
				DesktopBrowserAccessMainEventSchema.parse({
					personalAccessToken: 'p'.repeat(43),
					requestId: command.requestId,
					snapshot: {
						access: { phase: 'stopped' },
						personalAccessToken: 'configured',
					},
					type: 'desktop-browser-access-personal-access-token-issued',
				}),
			);
		});
		channel.port2.start();
		const client = new DesktopBrowserAccessClient(channel.port1, () => 'request');

		const result = await client.issuePersonalAccessToken();

		expect(result.personalAccessToken).toBe('p'.repeat(43));
		expect(result.snapshot.personalAccessToken).toBe('configured');
		client.dispose();
		channel.port2.close();
	});

	it('retires every pending request after a malformed publication', async () => {
		const channel = new MessageChannel();
		channel.port2.addEventListener('message', () => channel.port2.postMessage({ invalid: true }));
		channel.port2.start();
		const client = new DesktopBrowserAccessClient(channel.port1, () => 'request');

		await expect(client.getState()).rejects.toMatchObject({ code: 'gateway-unavailable' });
		await expect(client.getState()).rejects.toMatchObject({ code: 'desktop-quiescing' });
		channel.port2.close();
	});
});
