// @vitest-environment jsdom

import {
	DesktopProjectFolderAcquisitionMainEventSchema,
	DesktopProjectFolderAcquisitionRendererCommandSchema,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import { DesktopProjectFolderAcquisitionClient } from '../DesktopProjectFolderAcquisitionClient';

describe('DesktopProjectFolderAcquisitionClient', () => {
	it('correlates creation attempts, blocks replay after a created result and acknowledges only that request', async () => {
		const channel = new MessageChannel();
		const commands: unknown[] = [];
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopProjectFolderAcquisitionRendererCommandSchema.parse(event.data);
			commands.push(command);
			if (command.type === 'desktop-project-folder-parent-requested')
				channel.port2.postMessage({
					requestId: command.requestId,
					result: { kind: 'parent-selected', path: '/workspace' },
					type: 'desktop-project-folder-acquisition-completed',
				});
			if (command.type === 'desktop-project-folder-creation-requested') {
				channel.port2.postMessage(
					command.attempt === 1
						? {
								code: 'already-exists',
								requestId: command.requestId,
								type: 'desktop-project-folder-acquisition-failed',
							}
						: {
								requestId: command.requestId,
								result: { kind: 'created', path: '/workspace/new' },
								type: 'desktop-project-folder-acquisition-completed',
							},
				);
			}
		});
		channel.port2.start();
		const client = new DesktopProjectFolderAcquisitionClient(channel.port1, () => 'request');
		const selected = await client.newFolder.selectParent();
		if (selected.kind !== 'parent-selected') throw new Error('Expected the selected parent.');
		await expect(client.newFolder.create(selected.parent, 'taken')).resolves.toEqual({
			code: 'already-exists',
			kind: 'failed',
		});
		const created = await client.newFolder.create(selected.parent, 'new');
		if (created.kind !== 'created') throw new Error('Expected a created Folder.');
		await expect(client.newFolder.create(selected.parent, 'again')).resolves.toEqual({
			code: 'busy',
			kind: 'failed',
		});
		client.newFolder.acknowledge('stale');
		client.newFolder.acknowledge(created.requestId);
		await vi.waitFor(() => expect(commands).toHaveLength(4));
		expect(commands.at(-1)).toEqual({
			requestId: created.requestId,
			type: 'desktop-project-folder-creation-acknowledged',
		});
		await expect(client.newFolder.create(selected.parent, 'again')).resolves.toMatchObject({
			code: 'stale-parent',
			kind: 'failed',
		});
		client.dispose();
		channel.port2.close();
	});

	it('reports unknown effects and aborts workflow ownership on a mismatched creation result', async () => {
		const channel = new MessageChannel();
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopProjectFolderAcquisitionRendererCommandSchema.parse(event.data);
			channel.port2.postMessage({
				requestId: command.requestId,
				result: {
					kind:
						command.type === 'desktop-project-folder-parent-requested'
							? 'parent-selected'
							: 'selected',
					path: '/workspace',
				},
				type: 'desktop-project-folder-acquisition-completed',
			});
		});
		channel.port2.start();
		const client = new DesktopProjectFolderAcquisitionClient(channel.port1, () => 'request');
		const selected = await client.newFolder.selectParent();
		if (selected.kind !== 'parent-selected') throw new Error('Expected the selected parent.');
		await expect(client.newFolder.create(selected.parent, 'new')).resolves.toMatchObject({
			kind: 'outcome-unknown',
		});
		expect(client.newFolder.signal.aborted).toBe(true);
		channel.port2.close();
	});
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
