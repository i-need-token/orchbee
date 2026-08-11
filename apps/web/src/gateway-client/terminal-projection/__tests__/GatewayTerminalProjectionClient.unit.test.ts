import {
	encodeGatewayAttachmentServerFrame,
	encodeGatewayAttachmentServerMessage,
	GatewayAttachmentServerMessageSchema,
	GatewayBrowserConnectionSchema,
	GatewayControlServerMessageSchema,
	type GatewayControlServerMessage,
} from '@orchbee/gateway-protocol';
import {
	AttachmentIdSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
} from '@orchbee/terminal-protocol';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import {
	TerminalProjectionRequestIdSchema,
	type WorkbenchClientSessionBootstrap,
} from '@orchbee/workbench-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import type { GatewayControlConnection } from '../../connection';
import { GatewayTerminalProjectionClient } from '../GatewayTerminalProjectionClient';

describe('GatewayTerminalProjectionClient', () => {
	it('opens one dedicated attachment socket and propagates its exact retirement', async () => {
		const control = new FakeControlConnection();
		const socket = new FakeWebSocket();
		const connection = GatewayBrowserConnectionSchema.parse({
			capability: 'a'.repeat(43),
			port: 42_000,
			protocolVersion: '1.0',
		});
		const presentationId = terminalPresentationId('presentation-1');
		const requestId = TerminalProjectionRequestIdSchema.parse('gateway:projection:1');
		const client = new GatewayTerminalProjectionClient(
			control,
			connection,
			'192.168.1.14',
			(url) => {
				expect(url).toBe('ws://192.168.1.14:42000/v1/attachment');
				return socket.asWebSocket();
			},
			() => requestId,
		);
		const request = client.open(presentationId);
		expect(control.commands).toEqual([
			{ presentationId, requestId, type: 'terminal-projection-open-requested' },
		]);
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: false,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
		};
		control.emit(
			GatewayControlServerMessageSchema.parse({
				attachmentCapability: 'b'.repeat(43),
				attachmentSocketId: 'gateway:attachment:1',
				projection: {
					bootstrap,
					presentationId,
					requestId,
					type: 'terminal-projection-opened',
				},
				type: 'terminal-projection-opened',
			}),
		);
		socket.open();
		expect(socket.sent[0]).toContain('gateway-attachment-authenticate');
		socket.receive(
			encodeGatewayAttachmentServerMessage(
				GatewayAttachmentServerMessageSchema.parse({
					attachmentSocketId: 'gateway:attachment:1',
					type: 'gateway-attachment-ready',
				}),
			),
		);
		const projection = await request.result;
		const frames: unknown[] = [];
		const unavailable = vi.fn<() => void>();
		projection.transport.subscribe((event) => frames.push(event), unavailable);
		const frame = new Uint8Array([1, 2, 3]).buffer;
		socket.receive(encodeGatewayAttachmentServerFrame({ frame, kind: 'frame' }));

		expect(frames).toEqual([{ frame, kind: 'frame' }]);
		socket.serverClose();
		expect(unavailable).toHaveBeenCalledOnce();
		request.release();
		expect(control.commands.at(-1)).toEqual({
			presentationId,
			requestId,
			type: 'terminal-projection-release-requested',
		});
		client.dispose();
	});
});

class FakeControlConnection implements GatewayControlConnection {
	readonly bootstrap = {} as WorkbenchClientSessionBootstrap;
	readonly commands: unknown[] = [];
	#projectionListener: ((message: GatewayControlServerMessage) => void) | undefined;

	close(): void {
		return;
	}

	emit(message: GatewayControlServerMessage): void {
		this.#projectionListener?.(message);
	}

	sendClientSession(): void {
		return;
	}

	sendProjection(command: Parameters<GatewayControlConnection['sendProjection']>[0]): void {
		this.commands.push(command);
	}

	subscribeClientSession(): () => void {
		return () => undefined;
	}

	subscribeProjection(
		listener: Parameters<GatewayControlConnection['subscribeProjection']>[0],
	): () => void {
		this.#projectionListener = listener;
		return () => {
			this.#projectionListener = undefined;
		};
	}
}

class FakeWebSocket extends EventTarget {
	binaryType: BinaryType = 'blob';
	bufferedAmount = 0;
	readyState = 0;
	readonly sent: Array<string | ArrayBufferLike | Blob | ArrayBufferView> = [];

	asWebSocket(): WebSocket {
		return this as unknown as WebSocket;
	}

	close(): void {
		this.readyState = 3;
	}

	open(): void {
		this.readyState = 1;
		this.dispatchEvent(new Event('open'));
	}

	receive(data: string | ArrayBuffer): void {
		this.dispatchEvent(new MessageEvent('message', { data }));
	}

	send(data: string | ArrayBufferLike | Blob | ArrayBufferView): void {
		this.sent.push(data);
	}

	serverClose(): void {
		this.readyState = 3;
		this.dispatchEvent(new Event('close'));
	}
}
