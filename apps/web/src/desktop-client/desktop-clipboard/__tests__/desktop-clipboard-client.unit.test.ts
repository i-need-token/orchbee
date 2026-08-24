// @vitest-environment jsdom

import {
	DesktopClipboardMainEventSchema,
	DesktopClipboardRendererCommandSchema,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it } from 'vitest';

import {
	createDesktopTerminalPlatformEffects,
	DesktopClipboardClient,
} from '../desktop-clipboard-client';

describe('DesktopClipboardClient', () => {
	it('correlates a Main-owned clipboard read into the terminal platform effect', async () => {
		const channel = new MessageChannel();
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopClipboardRendererCommandSchema.parse(event.data);
			channel.port2.postMessage(
				DesktopClipboardMainEventSchema.parse({
					requestId: command.requestId,
					result: { kind: 'completed', text: 'clipboard text' },
					type: 'desktop-clipboard-read-text-responded',
				}),
			);
		});
		channel.port2.start();
		const client = new DesktopClipboardClient(channel.port1, () => 'request');
		const effects = createDesktopTerminalPlatformEffects(client);

		await expect(effects.readClipboardText?.()).resolves.toEqual({
			kind: 'completed',
			text: 'clipboard text',
		});

		client.dispose();
		channel.port2.close();
	});

	it('fails pending and future reads when the capability retires', async () => {
		const channel = new MessageChannel();
		const client = new DesktopClipboardClient(channel.port1, () => 'request');
		const effects = createDesktopTerminalPlatformEffects(client);
		const pending = effects.readClipboardText?.();

		client.dispose();

		await expect(pending).resolves.toEqual({
			code: 'desktop-clipboard-unavailable',
			kind: 'failed',
			retryable: false,
		});
		await expect(effects.readClipboardText?.()).resolves.toEqual({
			code: 'desktop-quiescing',
			kind: 'failed',
			retryable: false,
		});
		channel.port2.close();
	});
});
