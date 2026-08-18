// @vitest-environment jsdom

import {
	DesktopProjectFolderAcquisitionMainEventSchema,
	DesktopProjectFolderAcquisitionRendererCommandSchema,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it } from 'vitest';

import { DesktopProjectFolderAcquisitionClient } from '../DesktopProjectFolderAcquisitionClient';

describe('DesktopProjectFolderAcquisitionClient', () => {
	it('correlates one selected path and rejects overlapping local requests', async () => {
		const channel = new MessageChannel();
		let commandCount = 0;
		channel.port2.addEventListener('message', (event) => {
			commandCount += 1;
			const command = DesktopProjectFolderAcquisitionRendererCommandSchema.parse(event.data);
			channel.port2.postMessage(
				DesktopProjectFolderAcquisitionMainEventSchema.parse({
					requestId: command.requestId,
					result: { kind: 'selected', path: '/workspace/project' },
					type: 'desktop-project-folder-acquisition-completed',
				}),
			);
		});
		channel.port2.start();
		const client = new DesktopProjectFolderAcquisitionClient(channel.port1, () => 'request');

		const selected = client.selectExistingFolder();
		await expect(client.selectExistingFolder()).resolves.toEqual({ code: 'busy', kind: 'failed' });
		await expect(selected).resolves.toEqual({ kind: 'selected', path: '/workspace/project' });
		expect(commandCount).toBe(1);
		client.dispose();
		channel.port2.close();
	});

	it('maps bounded Main failures into Workbench results', async () => {
		const channel = new MessageChannel();
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopProjectFolderAcquisitionRendererCommandSchema.parse(event.data);
			channel.port2.postMessage(
				DesktopProjectFolderAcquisitionMainEventSchema.parse({
					code: 'permission-denied',
					requestId: command.requestId,
					type: 'desktop-project-folder-acquisition-failed',
				}),
			);
		});
		channel.port2.start();
		const client = new DesktopProjectFolderAcquisitionClient(channel.port1, () => 'request');

		await expect(client.selectExistingFolder()).resolves.toEqual({
			code: 'permission-denied',
			kind: 'failed',
		});
		client.dispose();
		channel.port2.close();
	});

	it('retires pending and future requests after malformed publication', async () => {
		const channel = new MessageChannel();
		channel.port2.addEventListener('message', () => channel.port2.postMessage({ invalid: true }));
		channel.port2.start();
		const client = new DesktopProjectFolderAcquisitionClient(channel.port1, () => 'request');

		await expect(client.selectExistingFolder()).resolves.toEqual({
			code: 'control-unavailable',
			kind: 'failed',
		});
		await expect(client.selectExistingFolder()).resolves.toEqual({
			code: 'control-unavailable',
			kind: 'failed',
		});
		channel.port2.close();
	});
});
