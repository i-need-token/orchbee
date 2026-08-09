import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import { TerminalProjectionRequestIdSchema } from '@orchbee/workbench-client-protocol';
import { DesktopTerminalProjectionClient } from '../DesktopTerminalProjectionClient';

const bootstrap = {
	attachmentId: '11'.repeat(16),
	canInput: true,
	canResize: true,
	creationId: '22'.repeat(16),
	hostEpoch: '33'.repeat(16),
	terminalId: '44'.repeat(16),
	terminalOutcome: null,
};

describe('DesktopTerminalProjectionClient', () => {
	it('correlates an opened attachment and releases only that request', async () => {
		const control = new MessageChannel();
		const terminal = new MessageChannel();
		const requestId = TerminalProjectionRequestIdSchema.parse('projection-1');
		const client = new DesktopTerminalProjectionClient(control.port1, () => requestId);
		const presentationId = terminalPresentationId('presentation-1');
		const commands: unknown[] = [];
		control.port2.addEventListener('message', (event) => commands.push(event.data));
		control.port2.start();

		const request = client.open(presentationId);
		await vi.waitFor(() => expect(commands).toHaveLength(1));
		control.port2.postMessage(
			{
				bootstrap,
				presentationId,
				requestId,
				type: 'terminal-projection-opened',
			},
			[terminal.port1],
		);

		await expect(request.result).resolves.toMatchObject({ bootstrap });
		request.release();
		await vi.waitFor(() => expect(commands).toHaveLength(2));
		expect(commands[1]).toEqual({
			presentationId,
			requestId,
			type: 'terminal-projection-release-requested',
		});
		client.dispose();
		control.port2.close();
		terminal.port2.close();
	});

	it('cancels an unopened request and rejects its pending result', async () => {
		const control = new MessageChannel();
		const requestId = TerminalProjectionRequestIdSchema.parse('projection-2');
		const client = new DesktopTerminalProjectionClient(control.port1, () => requestId);
		const commands: unknown[] = [];
		control.port2.addEventListener('message', (event) => commands.push(event.data));
		control.port2.start();
		const request = client.open(terminalPresentationId('presentation-2'));

		request.release();

		await expect(request.result).rejects.toMatchObject({
			failure: { code: 'terminal-projection-released', retryable: false },
		});
		await vi.waitFor(() => expect(commands).toHaveLength(2));
		client.dispose();
		control.port2.close();
	});

	it('keeps unrelated requests correlated when Main completes them out of order', async () => {
		const control = new MessageChannel();
		const requestIds = [
			TerminalProjectionRequestIdSchema.parse('projection-3'),
			TerminalProjectionRequestIdSchema.parse('projection-4'),
		];
		const client = new DesktopTerminalProjectionClient(
			control.port1,
			() => requestIds.shift() ?? TerminalProjectionRequestIdSchema.parse('projection-fallback'),
		);
		const firstPresentation = terminalPresentationId('presentation-3');
		const secondPresentation = terminalPresentationId('presentation-4');
		const first = client.open(firstPresentation);
		const second = client.open(secondPresentation);
		const firstTerminal = new MessageChannel();
		const secondTerminal = new MessageChannel();
		control.port2.postMessage(
			{
				bootstrap: { ...bootstrap, attachmentId: '55'.repeat(16) },
				presentationId: secondPresentation,
				requestId: 'projection-4',
				type: 'terminal-projection-opened',
			},
			[secondTerminal.port1],
		);
		control.port2.postMessage(
			{
				bootstrap,
				presentationId: firstPresentation,
				requestId: 'projection-3',
				type: 'terminal-projection-opened',
			},
			[firstTerminal.port1],
		);

		await expect(first.result).resolves.toMatchObject({ bootstrap });
		await expect(second.result).resolves.toMatchObject({
			bootstrap: { attachmentId: '55'.repeat(16) },
		});
		first.release();
		second.release();
		client.dispose();
		control.port2.close();
		firstTerminal.port2.close();
		secondTerminal.port2.close();
	});
});
