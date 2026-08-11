// @vitest-environment jsdom

import {
	DesktopBrowserAccessMainEventSchema,
	DesktopBrowserAccessRendererCommandSchema,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it } from 'vitest';

import { DesktopBrowserAccessClient } from '../DesktopBrowserAccessClient';

describe('DesktopBrowserAccessClient', () => {
	it('correlates sanitized states without exposing a capability link', async () => {
		const channel = new MessageChannel();
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopBrowserAccessRendererCommandSchema.parse(event.data);
			channel.port2.postMessage(
				DesktopBrowserAccessMainEventSchema.parse({
					requestId: command.requestId,
					state: {
						localOrigin: 'http://127.0.0.1:43110',
						mode: 'loopback',
						networkOrigins: [],
						phase: 'ready',
					},
					type: 'desktop-browser-access-completed',
				}),
			);
		});
		channel.port2.start();
		const client = new DesktopBrowserAccessClient(channel.port1, () => 'request');

		const state = await client.start('loopback');

		expect(state).toEqual({
			localOrigin: 'http://127.0.0.1:43110',
			mode: 'loopback',
			networkOrigins: [],
			phase: 'ready',
		});
		expect(JSON.stringify(state)).not.toContain('#');
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
					state: { phase: 'stopped' },
					type: 'desktop-browser-access-failed',
				}),
			);
		});
		channel.port2.start();
		const client = new DesktopBrowserAccessClient(channel.port1, () => 'request');

		await expect(client.start('local-network')).rejects.toEqual(
			expect.objectContaining({
				code: 'network-address-unavailable',
				state: { phase: 'stopped' },
			}),
		);
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
