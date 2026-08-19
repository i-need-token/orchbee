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

const mountedGeometry = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 96, rows: 32 } as const;

describe('GatewayTerminalProjectionClient', () => {
	it('opens one dedicated attachment socket and propagates its exact retirement', async () => {
		const control = new FakeControlConnection();
		const socket = new FakeWebSocket();
		const connection = GatewayBrowserConnectionSchema.parse({
			capability: 'a'.repeat(43),
			port: 42_000,
			protocolVersion: '1.2',
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
		const request = client.open(presentationId, mountedGeometry);
		expect(control.commands).toEqual([
			{
				geometry: mountedGeometry,
				presentationId,
				requestId,
				type: 'terminal-projection-open-requested',
			},
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
		control.emit(
			GatewayControlServerMessageSchema.parse({
				event: {
					presentationId,
					requestId,
					type: 'terminal-projection-released',
				},
				type: 'terminal-projection-control-event',
			}),
		);
		client.dispose();
	});

	it('retains a failed bootstrap request until release acknowledgement without retiring siblings', async () => {
		const control = new FakeControlConnection();
		const firstSocket = new FakeWebSocket();
		const secondSocket = new FakeWebSocket();
		const sockets = [firstSocket, secondSocket];
		const connection = GatewayBrowserConnectionSchema.parse({
			capability: 'a'.repeat(43),
			port: 42_000,
			protocolVersion: '1.2',
		});
		const firstPresentationId = terminalPresentationId('presentation-bootstrap-failure');
		const secondPresentationId = terminalPresentationId('presentation-still-live');
		const firstRequestId = TerminalProjectionRequestIdSchema.parse('gateway:projection:failure');
		const secondRequestId = TerminalProjectionRequestIdSchema.parse('gateway:projection:sibling');
		const requestIds = [firstRequestId, secondRequestId];
		const client = new GatewayTerminalProjectionClient(
			control,
			connection,
			'192.168.1.14',
			() => {
				const socket = sockets.shift();
				if (socket === undefined) throw new Error('Unexpected attachment socket.');
				return socket.asWebSocket();
			},
			() => {
				const requestId = requestIds.shift();
				if (requestId === undefined) throw new Error('Unexpected projection request.');
				return requestId;
			},
		);
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: true,
			canResize: false,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			terminalOutcome: null,
		};
		const first = client.open(firstPresentationId, mountedGeometry);
		control.emit(
			GatewayControlServerMessageSchema.parse({
				attachmentCapability: 'b'.repeat(43),
				attachmentSocketId: 'gateway:attachment:failure',
				projection: {
					bootstrap,
					presentationId: firstPresentationId,
					requestId: firstRequestId,
					type: 'terminal-projection-opened',
				},
				type: 'terminal-projection-opened',
			}),
		);
		firstSocket.serverClose();
		await expect(first.result).rejects.toMatchObject({
			failure: { code: 'terminal-projection-unavailable', retryable: true },
		});
		expect(control.commands.at(-1)).toEqual({
			presentationId: firstPresentationId,
			requestId: firstRequestId,
			type: 'terminal-projection-release-requested',
		});

		const second = client.open(secondPresentationId, mountedGeometry);
		control.emit(
			GatewayControlServerMessageSchema.parse({
				event: {
					presentationId: firstPresentationId,
					requestId: firstRequestId,
					type: 'terminal-projection-released',
				},
				type: 'terminal-projection-control-event',
			}),
		);
		control.emit(
			GatewayControlServerMessageSchema.parse({
				attachmentCapability: 'c'.repeat(43),
				attachmentSocketId: 'gateway:attachment:sibling',
				projection: {
					bootstrap: { ...bootstrap, attachmentId: AttachmentIdSchema.parse('55'.repeat(16)) },
					presentationId: secondPresentationId,
					requestId: secondRequestId,
					type: 'terminal-projection-opened',
				},
				type: 'terminal-projection-opened',
			}),
		);
		secondSocket.open();
		secondSocket.receive(
			encodeGatewayAttachmentServerMessage(
				GatewayAttachmentServerMessageSchema.parse({
					attachmentSocketId: 'gateway:attachment:sibling',
					type: 'gateway-attachment-ready',
				}),
			),
		);

		await expect(second.result).resolves.toMatchObject({
			bootstrap: { attachmentId: '55'.repeat(16) },
		});
		expect(control.closeCalls).toBe(0);
		client.dispose();
	});

	it('releases an opened projection when its attachment socket cannot be constructed', async () => {
		const control = new FakeControlConnection();
		const connection = GatewayBrowserConnectionSchema.parse({
			capability: 'a'.repeat(43),
			port: 42_000,
			protocolVersion: '1.2',
		});
		const presentationId = terminalPresentationId('presentation-socket-construction-failure');
		const requestId = TerminalProjectionRequestIdSchema.parse(
			'gateway:projection:socket-construction-failure',
		);
		const client = new GatewayTerminalProjectionClient(
			control,
			connection,
			'192.168.1.14',
			() => {
				throw new Error('injected WebSocket construction failure');
			},
			() => requestId,
		);
		const request = client.open(presentationId, mountedGeometry);
		control.emit(
			GatewayControlServerMessageSchema.parse({
				attachmentCapability: 'b'.repeat(43),
				attachmentSocketId: 'gateway:attachment:construction-failure',
				projection: {
					bootstrap: {
						attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
						canInput: true,
						canResize: false,
						creationId: CreationIdSchema.parse('22'.repeat(16)),
						hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
						terminalId: TerminalIdSchema.parse('44'.repeat(16)),
						terminalOutcome: null,
					},
					presentationId,
					requestId,
					type: 'terminal-projection-opened',
				},
				type: 'terminal-projection-opened',
			}),
		);

		await expect(request.result).rejects.toMatchObject({
			failure: { code: 'terminal-projection-unavailable', retryable: true },
		});
		expect(control.commands.at(-1)).toEqual({
			presentationId,
			requestId,
			type: 'terminal-projection-release-requested',
		});
		control.emit(
			GatewayControlServerMessageSchema.parse({
				event: { presentationId, requestId, type: 'terminal-projection-released' },
				type: 'terminal-projection-control-event',
			}),
		);
		expect(control.closeCalls).toBe(0);
		client.dispose();
	});

	it('releases an opened projection when attachment authentication cannot be sent', async () => {
		const control = new FakeControlConnection();
		const socket = new FakeWebSocket();
		socket.sendFailure = new Error('injected attachment authentication send failure');
		const connection = GatewayBrowserConnectionSchema.parse({
			capability: 'a'.repeat(43),
			port: 42_000,
			protocolVersion: '1.2',
		});
		const presentationId = terminalPresentationId('presentation-authentication-send-failure');
		const requestId = TerminalProjectionRequestIdSchema.parse(
			'gateway:projection:authentication-send-failure',
		);
		const client = new GatewayTerminalProjectionClient(
			control,
			connection,
			'192.168.1.14',
			() => socket.asWebSocket(),
			() => requestId,
		);
		const request = client.open(presentationId, mountedGeometry);
		control.emit(
			GatewayControlServerMessageSchema.parse({
				attachmentCapability: 'b'.repeat(43),
				attachmentSocketId: 'gateway:attachment:authentication-send-failure',
				projection: {
					bootstrap: {
						attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
						canInput: true,
						canResize: false,
						creationId: CreationIdSchema.parse('22'.repeat(16)),
						hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
						terminalId: TerminalIdSchema.parse('44'.repeat(16)),
						terminalOutcome: null,
					},
					presentationId,
					requestId,
					type: 'terminal-projection-opened',
				},
				type: 'terminal-projection-opened',
			}),
		);

		socket.open();
		await expect(request.result).rejects.toMatchObject({
			failure: { code: 'terminal-projection-unavailable', retryable: true },
		});
		expect(socket.readyState).toBe(3);
		expect(control.commands.at(-1)).toEqual({
			presentationId,
			requestId,
			type: 'terminal-projection-release-requested',
		});
		control.emit(
			GatewayControlServerMessageSchema.parse({
				event: { presentationId, requestId, type: 'terminal-projection-released' },
				type: 'terminal-projection-control-event',
			}),
		);
		expect(control.closeCalls).toBe(0);
		client.dispose();
	});

	it('sends one release when the caller releases before attachment bootstrap settles', async () => {
		const control = new FakeControlConnection();
		const socket = new FakeWebSocket();
		const connection = GatewayBrowserConnectionSchema.parse({
			capability: 'a'.repeat(43),
			port: 42_000,
			protocolVersion: '1.2',
		});
		const presentationId = terminalPresentationId('presentation-release-before-ready');
		const requestId = TerminalProjectionRequestIdSchema.parse('gateway:projection:early-release');
		const client = new GatewayTerminalProjectionClient(
			control,
			connection,
			'192.168.1.14',
			() => socket.asWebSocket(),
			() => requestId,
		);
		const request = client.open(presentationId, mountedGeometry);
		control.emit(
			GatewayControlServerMessageSchema.parse({
				attachmentCapability: 'b'.repeat(43),
				attachmentSocketId: 'gateway:attachment:early-release',
				projection: {
					bootstrap: {
						attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
						canInput: true,
						canResize: false,
						creationId: CreationIdSchema.parse('22'.repeat(16)),
						hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
						terminalId: TerminalIdSchema.parse('44'.repeat(16)),
						terminalOutcome: null,
					},
					presentationId,
					requestId,
					type: 'terminal-projection-opened',
				},
				type: 'terminal-projection-opened',
			}),
		);

		request.release();
		await expect(request.result).rejects.toMatchObject({
			failure: { code: 'terminal-projection-released', retryable: false },
		});
		await Promise.resolve();
		expect(
			control.commands.filter(
				(command) =>
					(command as { readonly type?: string }).type === 'terminal-projection-release-requested',
			),
		).toEqual([{ presentationId, requestId, type: 'terminal-projection-release-requested' }]);
		control.emit(
			GatewayControlServerMessageSchema.parse({
				event: { presentationId, requestId, type: 'terminal-projection-released' },
				type: 'terminal-projection-control-event',
			}),
		);
		expect(control.closeCalls).toBe(0);
		client.dispose();
	});

	it('drops a buffered frame when the attachment becomes unavailable before subscription', async () => {
		const control = new FakeControlConnection();
		const socket = new FakeWebSocket();
		const connection = GatewayBrowserConnectionSchema.parse({
			capability: 'a'.repeat(43),
			port: 42_000,
			protocolVersion: '1.2',
		});
		const presentationId = terminalPresentationId('presentation-buffered-disconnect');
		const requestId = TerminalProjectionRequestIdSchema.parse(
			'gateway:projection:buffered-disconnect',
		);
		const client = new GatewayTerminalProjectionClient(
			control,
			connection,
			'192.168.1.14',
			() => socket.asWebSocket(),
			() => requestId,
		);
		const request = client.open(presentationId, mountedGeometry);
		control.emit(
			GatewayControlServerMessageSchema.parse({
				attachmentCapability: 'b'.repeat(43),
				attachmentSocketId: 'gateway:attachment:buffered-disconnect',
				projection: {
					bootstrap: {
						attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
						canInput: true,
						canResize: false,
						creationId: CreationIdSchema.parse('22'.repeat(16)),
						hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
						terminalId: TerminalIdSchema.parse('44'.repeat(16)),
						terminalOutcome: null,
					},
					presentationId,
					requestId,
					type: 'terminal-projection-opened',
				},
				type: 'terminal-projection-opened',
			}),
		);
		socket.open();
		socket.receive(
			encodeGatewayAttachmentServerMessage(
				GatewayAttachmentServerMessageSchema.parse({
					attachmentSocketId: 'gateway:attachment:buffered-disconnect',
					type: 'gateway-attachment-ready',
				}),
			),
		);
		const projection = await request.result;
		const frame = new Uint8Array([1, 2, 3]).buffer;
		socket.receive(encodeGatewayAttachmentServerFrame({ frame, kind: 'frame' }));
		socket.serverClose();
		const received: unknown[] = [];
		const unavailable = vi.fn<() => void>();

		projection.transport.subscribe((event) => received.push(event), unavailable);

		expect(unavailable).toHaveBeenCalledOnce();
		expect(received).toEqual([]);
		request.release();
		control.emit(
			GatewayControlServerMessageSchema.parse({
				event: { presentationId, requestId, type: 'terminal-projection-released' },
				type: 'terminal-projection-control-event',
			}),
		);
		client.dispose();
	});
});

class FakeControlConnection implements GatewayControlConnection {
	readonly bootstrap = {} as WorkbenchClientSessionBootstrap;
	closeCalls = 0;
	readonly commands: unknown[] = [];
	#projectionListener: ((message: GatewayControlServerMessage) => void) | undefined;

	close(): void {
		this.closeCalls += 1;
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
	sendFailure: Error | undefined;
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
		if (this.sendFailure !== undefined) throw this.sendFailure;
		this.sent.push(data);
	}

	serverClose(): void {
		this.readyState = 3;
		this.dispatchEvent(new Event('close'));
	}
}
