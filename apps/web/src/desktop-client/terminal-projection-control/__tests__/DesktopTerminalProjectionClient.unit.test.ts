import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import { TerminalProjectionRequestIdSchema } from '@orchbee/workbench-client-protocol';
import { DesktopTerminalProjectionClient } from '../DesktopTerminalProjectionClient';

const mountedGeometry = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 96, rows: 32 } as const;

const bootstrap = {
	attachmentId: '11'.repeat(16),
	canInput: true,
	canResize: true,
	capabilityRevision: '1',
	creationId: '22'.repeat(16),
	hostEpoch: '33'.repeat(16),
	terminalId: '44'.repeat(16),
	terminalOutcome: null,
	theme: {
		operationId: null,
		revision: '0',
		terminal: {
			colorScheme: 'dark',
			defaultBackgroundRgb: 0x1e_1e_1e,
			defaultCursorRgb: null,
			defaultForegroundRgb: 0xd4_d4_d4,
			palette: Array.from({ length: 256 }, (_, index) => index),
		},
	},
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

		const request = client.open(presentationId, mountedGeometry);
		await vi.waitFor(() => expect(commands).toHaveLength(1));
		expect(commands[0]).toEqual({
			geometry: mountedGeometry,
			presentationId,
			requestId,
			type: 'terminal-projection-open-requested',
		});
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
		control.port2.postMessage({
			presentationId,
			requestId,
			type: 'terminal-projection-released',
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
		const request = client.open(terminalPresentationId('presentation-2'), mountedGeometry);

		request.release();

		await expect(request.result).rejects.toMatchObject({
			failure: { code: 'terminal-projection-released', retryable: false },
		});
		await vi.waitFor(() => expect(commands).toHaveLength(2));
		control.port2.postMessage({
			presentationId: terminalPresentationId('presentation-2'),
			requestId,
			type: 'terminal-projection-released',
		});
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
		const first = client.open(firstPresentation, mountedGeometry);
		const second = client.open(secondPresentation, mountedGeometry);
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

	it('does not let a released projection cleanup failure retire an unrelated pending open', async () => {
		const control = new MessageChannel();
		const firstTerminal = new MessageChannel();
		const secondTerminal = new MessageChannel();
		const requestIds = [
			TerminalProjectionRequestIdSchema.parse('projection-release'),
			TerminalProjectionRequestIdSchema.parse('projection-pending'),
		];
		const client = new DesktopTerminalProjectionClient(
			control.port1,
			() => requestIds.shift() ?? TerminalProjectionRequestIdSchema.parse('projection-fallback'),
		);
		const releasedPresentation = terminalPresentationId('presentation-release');
		const pendingPresentation = terminalPresentationId('presentation-pending');
		const commands: unknown[] = [];
		control.port2.addEventListener('message', (event) => commands.push(event.data));
		control.port2.start();
		const released = client.open(releasedPresentation, mountedGeometry);
		const pending = client.open(pendingPresentation, mountedGeometry);
		control.port2.postMessage(
			{
				bootstrap,
				presentationId: releasedPresentation,
				requestId: 'projection-release',
				type: 'terminal-projection-opened',
			},
			[firstTerminal.port1],
		);
		await expect(released.result).resolves.toMatchObject({ bootstrap });

		released.release();
		await vi.waitFor(() => expect(commands).toHaveLength(3));
		control.port2.postMessage({
			failure: { code: 'terminal-projection-cleanup-failed', retryable: true },
			presentationId: releasedPresentation,
			requestId: 'projection-release',
			type: 'terminal-projection-failed',
		});
		control.port2.postMessage(
			{
				bootstrap: { ...bootstrap, attachmentId: '55'.repeat(16) },
				presentationId: pendingPresentation,
				requestId: 'projection-pending',
				type: 'terminal-projection-opened',
			},
			[secondTerminal.port1],
		);

		await expect(pending.result).resolves.toMatchObject({
			bootstrap: { attachmentId: '55'.repeat(16) },
		});
		pending.release();
		client.dispose();
		control.port2.close();
		firstTerminal.port2.close();
		secondTerminal.port2.close();
	});

	it('rejects a duplicate generated request identity without replacing the original request', async () => {
		const control = new MessageChannel();
		const terminal = new MessageChannel();
		const requestId = TerminalProjectionRequestIdSchema.parse('projection-duplicate');
		const client = new DesktopTerminalProjectionClient(control.port1, () => requestId);
		const firstPresentation = terminalPresentationId('presentation-original');
		const secondPresentation = terminalPresentationId('presentation-collision');
		const first = client.open(firstPresentation, mountedGeometry);
		const second = client.open(secondPresentation, mountedGeometry);

		await expect(second.result).rejects.toMatchObject({
			failure: { code: 'terminal-projection-unavailable', retryable: true },
		});
		control.port2.postMessage(
			{
				bootstrap,
				presentationId: firstPresentation,
				requestId,
				type: 'terminal-projection-opened',
			},
			[terminal.port1],
		);
		await expect(first.result).resolves.toMatchObject({ bootstrap });

		first.release();
		control.port2.postMessage({
			presentationId: firstPresentation,
			requestId,
			type: 'terminal-projection-released',
		});
		client.dispose();
		control.port2.close();
		terminal.port2.close();
	});
});
