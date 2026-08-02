import {
	terminalPresentationId,
	terminalPresentationOperationId,
	type TerminalPresentationEvent,
} from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import { DesktopTerminalDocumentClient } from '../DesktopTerminalDocumentClient';

describe('DesktopTerminalDocumentClient', () => {
	it('sends scoped commands and publishes typed results without exposing generic IPC', async () => {
		const channel = new MessageChannel();
		const client = new DesktopTerminalDocumentClient(channel.port1);
		const command = {
			launchCwd: '/workspace',
			operationId: terminalPresentationOperationId('operation-1'),
			presentationId: terminalPresentationId('presentation-1'),
			type: 'terminal-provision-requested' as const,
		};
		const received: unknown[] = [];
		channel.port2.addEventListener('message', (event) => received.push(event.data));
		channel.port2.start();
		const listener = vi.fn<(event: TerminalPresentationEvent) => void>();
		client.subscribe(listener);

		client.dispatch([command]);
		await vi.waitFor(() => expect(received).toEqual([command]));
		channel.port2.postMessage({
			operationId: command.operationId,
			presentationId: command.presentationId,
			type: 'terminal-provision-completed',
		});
		await vi.waitFor(() => expect(listener).toHaveBeenCalledOnce());
		expect(listener).toHaveBeenCalledWith(
			expect.objectContaining({ presentationId: command.presentationId }),
		);
		const restoreCommand = {
			launchCwd: '/workspace',
			operationId: terminalPresentationOperationId('restore-1'),
			presentationId: terminalPresentationId('presentation-1'),
			type: 'terminal-restore-requested' as const,
		};
		client.dispatch([restoreCommand]);
		await vi.waitFor(() => expect(received).toEqual([command, restoreCommand]));
		channel.port2.postMessage({
			operationId: restoreCommand.operationId,
			presentationId: restoreCommand.presentationId,
			type: 'terminal-restore-completed',
		});
		await vi.waitFor(() => expect(listener).toHaveBeenCalledTimes(2));
		client.dispose();
		channel.port2.close();
	});

	it('retires its capability after a malformed Main event', async () => {
		const channel = new MessageChannel();
		const close = vi.spyOn(channel.port1, 'close');
		const client = new DesktopTerminalDocumentClient(channel.port1);
		const listener = vi.fn<(event: TerminalPresentationEvent) => void>();
		client.subscribe(listener);

		channel.port2.postMessage({ channel: 'generic-ipc', type: 'send' });

		await vi.waitFor(() => expect(close).toHaveBeenCalledOnce());
		expect(listener).not.toHaveBeenCalled();
		channel.port2.close();
	});
});
