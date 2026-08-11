import {
	decodeGatewayAttachmentServerFrame,
	decodeGatewayAttachmentServerMessage,
	encodeGatewayAttachmentClientFrame,
	encodeGatewayAttachmentClientMessage,
	GATEWAY_ATTACHMENT_PATH,
	GATEWAY_LIMITS,
	GatewayAttachmentClientMessageSchema,
	type GatewayBrowserConnection,
	type GatewayControlServerMessage,
} from '@orchbee/gateway-protocol';
import type {
	TerminalPresentationFailure,
	TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';
import {
	TerminalProjectionRequestIdSchema,
	type ProjectedTerminalCommand,
	type ProjectedTerminalEvent,
	type TerminalProjectionRequestId,
} from '@orchbee/workbench-client-protocol';

import { createBrowserRandomIdentifier } from '../../browser-random-identifier';
import type {
	ProjectedTerminalTransport,
	TerminalProjection,
	TerminalProjectionClient,
	TerminalProjectionRequest,
} from '../../workbench-client';
import {
	createGatewayWebSocketUrl,
	type GatewayControlConnection,
	type GatewayWebSocketFactory,
} from '../connection';

class GatewayTerminalProjectionRequestError extends Error {
	readonly failure: TerminalPresentationFailure;

	constructor(failure: TerminalPresentationFailure) {
		super(failure.code);
		this.name = 'GatewayTerminalProjectionRequestError';
		this.failure = failure;
	}
}

interface PendingProjection {
	readonly presentationId: TerminalPresentationId;
	readonly reject: (error: Error) => void;
	released: boolean;
	readonly resolve: (projection: TerminalProjection) => void;
	transport: GatewayAttachmentTransport | null;
}

export class GatewayTerminalProjectionClient implements TerminalProjectionClient {
	readonly #connection: GatewayBrowserConnection;
	readonly #control: GatewayControlConnection;
	readonly #createRequestId: () => TerminalProjectionRequestId;
	readonly #createWebSocket: GatewayWebSocketFactory;
	readonly #hostname: string;
	#disposed = false;
	readonly #pending = new Map<TerminalProjectionRequestId, PendingProjection>();
	readonly #unsubscribeControl: () => void;

	constructor(
		control: GatewayControlConnection,
		connection: GatewayBrowserConnection,
		hostname: string,
		createWebSocket: GatewayWebSocketFactory = (url) => new WebSocket(url),
		createRequestId: () => TerminalProjectionRequestId = () =>
			TerminalProjectionRequestIdSchema.parse(
				`gateway:projection:${createBrowserRandomIdentifier()}`,
			),
	) {
		this.#control = control;
		this.#connection = connection;
		this.#hostname = hostname;
		this.#createWebSocket = createWebSocket;
		this.#createRequestId = createRequestId;
		this.#unsubscribeControl = control.subscribeProjection(
			this.#handleControlMessage,
			this.#handleControlUnavailable,
		);
	}

	open(presentationId: TerminalPresentationId): TerminalProjectionRequest {
		const requestId = this.#createRequestId();
		let resolve = (_projection: TerminalProjection): void => undefined;
		let reject = (_error: Error): void => undefined;
		const result = new Promise<TerminalProjection>((nextResolve, nextReject) => {
			resolve = nextResolve;
			reject = nextReject;
		});
		const pending: PendingProjection = {
			presentationId,
			reject,
			released: false,
			resolve,
			transport: null,
		};
		if (this.#disposed || this.#pending.has(requestId)) {
			reject(unavailableError());
			return { release: () => undefined, result };
		}
		this.#pending.set(requestId, pending);
		try {
			this.#control.sendProjection({
				presentationId,
				requestId,
				type: 'terminal-projection-open-requested',
			});
		} catch {
			this.#pending.delete(requestId);
			reject(unavailableError());
		}
		return { release: () => this.#release(requestId, pending), result };
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#unsubscribeControl();
		for (const pending of this.#pending.values()) {
			pending.released = true;
			pending.transport?.close();
			pending.reject(unavailableError());
		}
		this.#pending.clear();
	}

	readonly #handleControlMessage = (message: GatewayControlServerMessage): void => {
		if (this.#disposed) return;
		if (message.type === 'terminal-projection-control-event') {
			const event = message.event;
			if (event.type !== 'terminal-projection-failed') {
				this.#rejectControl();
				return;
			}
			const pending = this.#pending.get(event.requestId);
			if (
				pending === undefined ||
				pending.presentationId !== event.presentationId ||
				pending.released
			) {
				this.#rejectControl();
				return;
			}
			this.#pending.delete(event.requestId);
			pending.reject(new GatewayTerminalProjectionRequestError(event.failure));
			return;
		}
		if (message.type !== 'terminal-projection-opened') return;
		const event = message.projection;
		if (event.type !== 'terminal-projection-opened') {
			this.#rejectControl();
			return;
		}
		const pending = this.#pending.get(event.requestId);
		if (
			pending === undefined ||
			pending.presentationId !== event.presentationId ||
			pending.released ||
			pending.transport !== null
		) {
			this.#rejectControl();
			return;
		}
		const transport = new GatewayAttachmentTransport({
			attachmentCapability: message.attachmentCapability,
			attachmentSocketId: message.attachmentSocketId,
			connection: this.#connection,
			createWebSocket: this.#createWebSocket,
			hostname: this.#hostname,
		});
		pending.transport = transport;
		void transport.ready.then(
			() => {
				if (this.#disposed || pending.released) {
					transport.close();
					return;
				}
				pending.resolve({ bootstrap: event.bootstrap, transport });
			},
			() => {
				if (this.#pending.get(event.requestId) === pending) {
					this.#pending.delete(event.requestId);
				}
				pending.released = true;
				transport.close();
				if (!this.#disposed) {
					try {
						this.#control.sendProjection({
							presentationId: pending.presentationId,
							requestId: event.requestId,
							type: 'terminal-projection-release-requested',
						});
					} catch {
						this.#rejectControl();
					}
				}
				pending.reject(unavailableError());
			},
		);
	};

	readonly #handleControlUnavailable = (): void => {
		this.dispose();
	};

	#release(requestId: TerminalProjectionRequestId, pending: PendingProjection): void {
		if (pending.released) return;
		pending.released = true;
		this.#pending.delete(requestId);
		pending.transport?.close();
		if (pending.transport === null) pending.reject(releasedError());
		if (this.#disposed) return;
		try {
			this.#control.sendProjection({
				presentationId: pending.presentationId,
				requestId,
				type: 'terminal-projection-release-requested',
			});
		} catch {
			this.#rejectControl();
		}
	}

	#rejectControl(): void {
		this.dispose();
		this.#control.close();
	}
}

interface GatewayAttachmentTransportOptions {
	readonly attachmentCapability: ReturnType<
		typeof GatewayAttachmentClientMessageSchema.parse
	>['attachmentCapability'];
	readonly attachmentSocketId: ReturnType<
		typeof GatewayAttachmentClientMessageSchema.parse
	>['attachmentSocketId'];
	readonly connection: GatewayBrowserConnection;
	readonly createWebSocket: GatewayWebSocketFactory;
	readonly hostname: string;
}

class GatewayAttachmentTransport implements ProjectedTerminalTransport {
	#disposed = false;
	#failed = false;
	#listener: ((value: unknown) => void) | undefined;
	readonly #options: GatewayAttachmentTransportOptions;
	#pendingBytes = 0;
	readonly #pendingEvents: Array<{
		readonly byteLength: number;
		readonly event: ProjectedTerminalEvent;
	}> = [];
	readonly ready: Promise<void>;
	readonly projectionBufferTrust = 'external' as const;
	#rejectReady: ((error: Error) => void) | undefined;
	#resolveReady: (() => void) | undefined;
	readonly #socket: WebSocket;
	#unavailable: (() => void) | undefined;

	constructor(options: GatewayAttachmentTransportOptions) {
		this.#options = options;
		this.#socket = options.createWebSocket(
			createGatewayWebSocketUrl(options.hostname, options.connection.port, GATEWAY_ATTACHMENT_PATH),
		);
		this.#socket.binaryType = 'arraybuffer';
		this.ready = new Promise((resolve, reject) => {
			this.#resolveReady = resolve;
			this.#rejectReady = reject;
		});
		this.#socket.addEventListener('open', this.#handleOpen);
		this.#socket.addEventListener('message', this.#handleMessage);
		this.#socket.addEventListener('close', this.#handleUnavailable);
		this.#socket.addEventListener('error', this.#handleUnavailable);
	}

	close(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#removeSocketListeners();
		this.#socket.close();
		this.#pendingEvents.length = 0;
		this.#pendingBytes = 0;
		this.#listener = undefined;
		this.#unavailable = undefined;
		this.#rejectReady?.(new Error('Gateway attachment closed before bootstrap.'));
		this.#resolveReady = undefined;
		this.#rejectReady = undefined;
	}

	send(command: ProjectedTerminalCommand): void {
		if (this.#disposed || this.#failed || this.#socket.readyState !== 1) {
			throw new Error('Gateway attachment is unavailable.');
		}
		const frame = encodeGatewayAttachmentClientFrame(command);
		if (
			this.#socket.bufferedAmount + frame.byteLength >
			GATEWAY_LIMITS.maxAttachmentBufferedBytes
		) {
			this.#becomeUnavailable();
			throw new Error('Gateway attachment input exceeded its bounded queue.');
		}
		this.#socket.send(frame);
	}

	subscribe(listener: (value: unknown) => void, unavailable: () => void): () => void {
		if (this.#disposed || this.#listener !== undefined) {
			unavailable();
			return () => undefined;
		}
		this.#listener = listener;
		this.#unavailable = unavailable;
		if (this.#failed) unavailable();
		for (const pending of this.#pendingEvents.splice(0)) listener(pending.event);
		this.#pendingBytes = 0;
		return () => {
			if (this.#listener === listener) this.#listener = undefined;
			if (this.#unavailable === unavailable) this.#unavailable = undefined;
		};
	}

	readonly #handleOpen = (): void => {
		this.#socket.send(
			encodeGatewayAttachmentClientMessage(
				GatewayAttachmentClientMessageSchema.parse({
					attachmentCapability: this.#options.attachmentCapability,
					attachmentSocketId: this.#options.attachmentSocketId,
					protocolVersion: this.#options.connection.protocolVersion,
					type: 'gateway-attachment-authenticate',
				}),
			),
		);
	};

	readonly #handleMessage = (message: MessageEvent<unknown>): void => {
		if (this.#disposed) return;
		if (typeof message.data === 'string') {
			const decoded = decodeGatewayAttachmentServerMessage(message.data);
			if (
				!decoded.ok ||
				decoded.value.type !== 'gateway-attachment-ready' ||
				decoded.value.attachmentSocketId !== this.#options.attachmentSocketId ||
				this.#resolveReady === undefined
			) {
				this.#becomeUnavailable();
				return;
			}
			this.#resolveReady();
			this.#resolveReady = undefined;
			this.#rejectReady = undefined;
			return;
		}
		if (!(message.data instanceof ArrayBuffer) || this.#resolveReady !== undefined) {
			this.#becomeUnavailable();
			return;
		}
		const event = decodeGatewayAttachmentServerFrame(message.data);
		if (!event.ok) {
			this.#becomeUnavailable();
			return;
		}
		if (this.#listener !== undefined) {
			this.#listener(event.value);
			return;
		}
		if (this.#pendingBytes + message.data.byteLength > GATEWAY_LIMITS.maxAttachmentBufferedBytes) {
			this.#becomeUnavailable();
			return;
		}
		this.#pendingBytes += message.data.byteLength;
		this.#pendingEvents.push({ byteLength: message.data.byteLength, event: event.value });
	};

	readonly #handleUnavailable = (): void => this.#becomeUnavailable();

	#becomeUnavailable(): void {
		if (this.#disposed || this.#failed) return;
		this.#failed = true;
		this.#removeSocketListeners();
		this.#socket.close();
		this.#rejectReady?.(new Error('Gateway attachment is unavailable.'));
		this.#resolveReady = undefined;
		this.#rejectReady = undefined;
		this.#unavailable?.();
	}

	#removeSocketListeners(): void {
		this.#socket.removeEventListener('open', this.#handleOpen);
		this.#socket.removeEventListener('message', this.#handleMessage);
		this.#socket.removeEventListener('close', this.#handleUnavailable);
		this.#socket.removeEventListener('error', this.#handleUnavailable);
	}
}

function unavailableError(): GatewayTerminalProjectionRequestError {
	return new GatewayTerminalProjectionRequestError({
		code: 'terminal-projection-unavailable',
		retryable: true,
	});
}

function releasedError(): GatewayTerminalProjectionRequestError {
	return new GatewayTerminalProjectionRequestError({
		code: 'terminal-projection-released',
		retryable: false,
	});
}
