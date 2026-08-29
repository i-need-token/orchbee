// @vitest-environment jsdom

import {
	DesktopRemoteTerminalCreationMainEventSchema,
	DesktopRemoteTerminalCreationRendererCommandSchema,
} from '@orchbee/desktop-client-protocol';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench/document';
import { describe, expect, it, vi } from 'vitest';

import { DesktopRemoteTerminalCreationClient } from '../DesktopRemoteTerminalCreationClient';

describe('DesktopRemoteTerminalCreationClient', () => {
	it('correlates one canonical result and rejects overlapping requests', async () => {
		const channel = new MessageChannel();
		const commands: unknown[] = [];
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopRemoteTerminalCreationRendererCommandSchema.parse(event.data);
			commands.push(command);
			if (command.type !== 'desktop-remote-terminal-creation-requested') return;
			channel.port2.postMessage(
				DesktopRemoteTerminalCreationMainEventSchema.parse({
					requestId: command.requestId,
					result: createdResult(),
					type: 'desktop-remote-terminal-creation-settled',
				}),
			);
		});
		channel.port2.start();
		const client = new DesktopRemoteTerminalCreationClient(channel.port1, () => 'request');
		const first = client.create(createInput());

		await expect(client.create(createInput())).resolves.toEqual({
			kind: 'failed',
			reason: 'busy',
		});
		await expect(first).resolves.toEqual(createdResult());
		expect(commands).toHaveLength(1);
		client.dispose();
		channel.port2.close();
	});

	it('sends cancellation but waits for Main confirmation', async () => {
		const channel = new MessageChannel();
		let cancellationObserved = false;
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopRemoteTerminalCreationRendererCommandSchema.parse(event.data);
			if (command.type !== 'desktop-remote-terminal-creation-cancel-requested') return;
			cancellationObserved = true;
			channel.port2.postMessage(
				DesktopRemoteTerminalCreationMainEventSchema.parse({
					requestId: command.requestId,
					result: { kind: 'cancelled' },
					type: 'desktop-remote-terminal-creation-settled',
				}),
			);
		});
		channel.port2.start();
		const controller = new AbortController();
		const client = new DesktopRemoteTerminalCreationClient(channel.port1, () => 'request');
		const result = client.create(createInput(controller.signal));

		controller.abort();
		await expect(result).resolves.toEqual({ kind: 'cancelled' });
		expect(cancellationObserved).toBe(true);
		client.dispose();
		channel.port2.close();
	});

	it('returns outcome unknown when a dispatched control is retired', async () => {
		const channel = new MessageChannel();
		const client = new DesktopRemoteTerminalCreationClient(channel.port1, () => 'request');
		const result = client.create(createInput());

		client.dispose();
		await expect(result).resolves.toEqual({
			kind: 'indeterminate',
			reason: 'control-unavailable',
		});
		await expect(client.create(createInput())).resolves.toEqual({
			kind: 'failed',
			reason: 'control-unavailable',
		});
		channel.port2.close();
	});

	it('does not dispatch an already-aborted request', async () => {
		const channel = new MessageChannel();
		const postMessage = vi.spyOn(channel.port1, 'postMessage');
		const controller = new AbortController();
		controller.abort();
		const client = new DesktopRemoteTerminalCreationClient(channel.port1, () => 'request');

		await expect(client.create(createInput(controller.signal))).resolves.toEqual({
			kind: 'cancelled',
		});
		expect(postMessage).not.toHaveBeenCalled();
		client.dispose();
		channel.port2.close();
	});

	it('retires malformed Main authority as an outcome-unknown result', async () => {
		const channel = new MessageChannel();
		channel.port2.addEventListener('message', () => channel.port2.postMessage({ invalid: true }));
		channel.port2.start();
		const client = new DesktopRemoteTerminalCreationClient(channel.port1, () => 'request');

		await expect(client.create(createInput())).resolves.toEqual({
			kind: 'indeterminate',
			reason: 'control-unavailable',
		});
		channel.port2.close();
	});
});

function createInput(signal = new AbortController().signal) {
	return { projectId: 'project-1', signal, workspaceId: 'workspace-1' };
}

function createdResult() {
	return {
		kind: 'created' as const,
		panelId: 'panel-1',
		presentationId: 'presentation-1',
		projectId: 'project-1',
		revision: workbenchSharedDocumentRevision('2'),
		tabId: 'tab-1',
		terminalContextId: 'terminal-context-1',
		workspaceId: 'workspace-1',
	};
}
