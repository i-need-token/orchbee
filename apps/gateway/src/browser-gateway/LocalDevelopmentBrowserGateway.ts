import { createHmac, randomBytes, timingSafeEqual } from 'node:crypto';
import { createServer, type Server } from 'node:http';
import type { Duplex } from 'node:stream';

import {
	decodeGatewayAttachmentClientFrame,
	decodeGatewayAttachmentClientMessage,
	decodeGatewayControlClientMessage,
	encodeGatewayAttachmentServerFrame,
	encodeGatewayAttachmentServerMessage,
	encodeGatewayControlServerMessage,
	GATEWAY_ATTACHMENT_PATH,
	GATEWAY_CONTROL_PATH,
	GATEWAY_LIMITS,
	GatewayAttachmentSocketIdSchema,
	GatewayCapabilitySchema,
	GatewayClientConnectionIdSchema,
	GatewayDesktopCommandSchema,
	GatewayUtilityEventSchema,
	type GatewayAttachmentClientMessage,
	type GatewayCapability,
	type GatewayClientConnectionId,
	type GatewayControlServerMessage,
	type GatewayUtilityEvent,
} from '@orchbee/gateway-protocol';
import {
	ProjectedTerminalCommandSchema,
	ProjectedTerminalEventSchema,
	TerminalProjectionControlEventSchema,
	WorkbenchClientSessionEventSchema,
	type ProjectedAttachmentBootstrap,
	type ProjectedTerminalCommand,
	type TerminalProjectionRequestId,
	type WorkbenchClientSessionBootstrap,
} from '@orchbee/workbench-client-protocol';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { WebSocket as NodeWebSocket, WebSocketServer, type RawData } from 'ws';

import {
	transitionGatewayClientLifecycle,
	type GatewayClientLifecycleCommand,
	type GatewayClientLifecycleEvent,
	type GatewayClientLifecycleState,
} from '../lifecycle';

interface GatewayPortMessageEvent {
	readonly data: unknown;
	readonly ports: readonly GatewayMessagePort[];
}

export interface GatewayMessagePort {
	close(): void;
	off(event: 'close', listener: () => void): void;
	off(event: 'message', listener: (event: GatewayPortMessageEvent) => void): void;
	on(event: 'close', listener: () => void): void;
	on(event: 'message', listener: (event: GatewayPortMessageEvent) => void): void;
	postMessage(message: unknown): void;
	start(): void;
}

export interface GatewayParentMessageEvent {
	readonly data: unknown;
	readonly ports: readonly GatewayMessagePort[];
}

export interface GatewayParentPort {
	off(event: 'message', listener: (event: GatewayParentMessageEvent) => void): void;
	on(event: 'message', listener: (event: GatewayParentMessageEvent) => void): void;
	postMessage(message: unknown): void;
}

interface AttachmentRegistration {
	accept(socket: NodeWebSocket, authentication: GatewayAttachmentClientMessage): void;
	reject(): void;
}

interface GatewayPageConnectionDependencies {
	readonly capability: GatewayCapability;
	readonly createAttachmentSocketId: () => ReturnType<typeof GatewayAttachmentSocketIdSchema.parse>;
	readonly deriveAttachmentCapability: (
		socketId: ReturnType<typeof GatewayAttachmentSocketIdSchema.parse>,
	) => GatewayCapability;
	readonly postParent: (event: GatewayUtilityEvent) => void;
	readonly registerAttachment: (
		socketId: ReturnType<typeof GatewayAttachmentSocketIdSchema.parse>,
		registration: AttachmentRegistration,
	) => void;
	readonly retired: (connectionId: GatewayClientConnectionId) => void;
	readonly unregisterAttachment: (
		socketId: ReturnType<typeof GatewayAttachmentSocketIdSchema.parse>,
	) => void;
}

interface PendingProjection {
	readonly presentationId: TerminalPresentationId;
	readonly requestId: TerminalProjectionRequestId;
}

export class LocalDevelopmentBrowserGateway {
	#allowedOrigin: string | undefined;
	readonly #attachments = new Map<string, AttachmentRegistration>();
	#capability: GatewayCapability | undefined;
	#disposePromise: Promise<void> | undefined;
	#disposed = false;
	readonly #pages = new Map<GatewayClientConnectionId, GatewayPageConnection>();
	readonly #parentPort: GatewayParentPort;
	#server: Server | undefined;
	#started = false;
	#webSocketServer: WebSocketServer | undefined;

	constructor(parentPort: GatewayParentPort) {
		this.#parentPort = parentPort;
	}

	start(): void {
		if (this.#started || this.#disposed) return;
		this.#started = true;
		this.#parentPort.on('message', this.#handleParentMessage);
	}

	dispose(): Promise<void> {
		return (this.#disposePromise ??= this.#dispose());
	}

	readonly #handleParentMessage = (message: GatewayParentMessageEvent): void => {
		if (this.#disposed) {
			closePorts(message.ports);
			return;
		}
		const parsed = GatewayDesktopCommandSchema.safeParse(message.data);
		if (!parsed.success) {
			closePorts(message.ports);
			this.#postParent({ code: 'parent-protocol-rejected', type: 'gateway-failed' });
			void this.dispose();
			return;
		}
		const command = parsed.data;
		switch (command.type) {
			case 'gateway-start':
				if (message.ports.length !== 0 || this.#server !== undefined) {
					closePorts(message.ports);
					this.#postParent({ code: 'invalid-configuration', type: 'gateway-failed' });
					void this.dispose();
					return;
				}
				void this.#startServer(command.allowedOrigin, command.capability);
				return;
			case 'gateway-client-opened': {
				const page = this.#pages.get(command.connectionId);
				const clientSessionPort = message.ports[0];
				const projectionPort = message.ports[1];
				if (
					page === undefined ||
					clientSessionPort === undefined ||
					projectionPort === undefined ||
					message.ports.length !== 2
				) {
					closePorts(message.ports);
					this.#postParent({
						connectionId: command.connectionId,
						type: 'gateway-client-close-requested',
					});
					return;
				}
				page.acceptDesktopClient(command.bootstrap, clientSessionPort, projectionPort);
				return;
			}
			case 'gateway-client-open-failed':
				closePorts(message.ports);
				this.#pages.get(command.connectionId)?.rejectDesktopClient();
				return;
			case 'gateway-stop':
				closePorts(message.ports);
				void this.dispose();
				return;
		}
	};

	async #startServer(allowedOrigin: string, capability: GatewayCapability): Promise<void> {
		if (!isExactLoopbackOrigin(allowedOrigin)) {
			this.#postParent({ code: 'invalid-configuration', type: 'gateway-failed' });
			await this.dispose();
			return;
		}
		this.#allowedOrigin = allowedOrigin;
		this.#capability = capability;
		const server = createServer((_request, response) => {
			response.statusCode = 404;
			response.end();
		});
		const webSocketServer = new WebSocketServer({
			maxPayload: GATEWAY_LIMITS.maxControlMessageBytes,
			noServer: true,
		});
		this.#server = server;
		this.#webSocketServer = webSocketServer;
		server.on('upgrade', (request, socket, head) => {
			const pathname = requestPathname(request.url);
			if (
				this.#disposed ||
				request.headers.origin !== this.#allowedOrigin ||
				(pathname !== GATEWAY_CONTROL_PATH && pathname !== GATEWAY_ATTACHMENT_PATH)
			) {
				rejectUpgrade(socket);
				return;
			}
			webSocketServer.handleUpgrade(request, socket, head, (webSocket) => {
				webSocketServer.emit('connection', webSocket, request);
			});
		});
		webSocketServer.on('connection', (socket, request) => {
			const pathname = requestPathname(request.url);
			if (pathname === GATEWAY_CONTROL_PATH) this.#acceptControlSocket(socket);
			else if (pathname === GATEWAY_ATTACHMENT_PATH) this.#acceptAttachmentSocket(socket);
			else socket.close(1008, 'Gateway channel rejected.');
		});
		try {
			await listenLoopback(server);
		} catch {
			this.#postParent({ code: 'listen-failed', type: 'gateway-failed' });
			await this.dispose();
			return;
		}
		if (this.#disposed) return;
		const address = server.address();
		if (address === null || typeof address === 'string') {
			this.#postParent({ code: 'listen-failed', type: 'gateway-failed' });
			await this.dispose();
			return;
		}
		this.#postParent({ port: address.port, type: 'gateway-listening' });
	}

	#acceptControlSocket(socket: NodeWebSocket): void {
		const capability = this.#capability;
		if (capability === undefined || this.#disposed) {
			socket.close(1011, 'Gateway unavailable.');
			return;
		}
		const connectionId = GatewayClientConnectionIdSchema.parse(
			`gateway:client:${randomBytes(16).toString('hex')}`,
		);
		const page = new GatewayPageConnection(socket, connectionId, {
			capability,
			createAttachmentSocketId: () =>
				GatewayAttachmentSocketIdSchema.parse(
					`gateway:attachment:${randomBytes(16).toString('hex')}`,
				),
			deriveAttachmentCapability: (socketId) =>
				GatewayCapabilitySchema.parse(
					createHmac('sha256', capability).update(socketId).digest('base64url'),
				),
			postParent: (event) => this.#postParent(event),
			registerAttachment: (socketId, registration) => {
				this.#attachments.set(socketId, registration);
			},
			retired: (retiredConnectionId) => {
				if (this.#pages.get(retiredConnectionId) === page) {
					this.#pages.delete(retiredConnectionId);
				}
			},
			unregisterAttachment: (socketId) => this.#attachments.delete(socketId),
		});
		this.#pages.set(connectionId, page);
		page.start();
	}

	#acceptAttachmentSocket(socket: NodeWebSocket): void {
		let settled = false;
		const reject = (): void => {
			if (settled) return;
			settled = true;
			sendAttachmentFailure(socket, 'authentication-rejected', false);
		};
		const handleError = (): void => reject();
		const handleMessage = (data: RawData, isBinary: boolean): void => {
			if (settled) return;
			settled = true;
			socket.off('error', handleError);
			if (isBinary) {
				sendAttachmentFailure(socket, 'protocol-rejected', false);
				return;
			}
			const text = rawDataToText(data);
			const authentication = text === null ? null : decodeGatewayAttachmentClientMessage(text);
			if (authentication === null || !authentication.ok) {
				sendAttachmentFailure(socket, 'authentication-rejected', false);
				return;
			}
			const registration = this.#attachments.get(authentication.value.attachmentSocketId);
			if (registration === undefined) {
				sendAttachmentFailure(socket, 'authentication-rejected', false);
				return;
			}
			registration.accept(socket, authentication.value);
		};
		socket.once('error', handleError);
		socket.once('message', handleMessage);
	}

	#postParent(event: GatewayUtilityEvent): void {
		if (this.#disposed && event.type !== 'gateway-stopped') return;
		this.#parentPort.postMessage(GatewayUtilityEventSchema.parse(event));
	}

	async #dispose(): Promise<void> {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#parentPort.off('message', this.#handleParentMessage);
		for (const page of this.#pages.values()) page.dispose();
		this.#pages.clear();
		for (const registration of this.#attachments.values()) registration.reject();
		this.#attachments.clear();
		const webSocketServer = this.#webSocketServer;
		this.#webSocketServer = undefined;
		webSocketServer?.close();
		const server = this.#server;
		this.#server = undefined;
		if (server !== undefined) await closeServer(server);
		this.#parentPort.postMessage(GatewayUtilityEventSchema.parse({ type: 'gateway-stopped' }));
	}
}

class GatewayPageConnection {
	readonly #attachments = new Map<TerminalProjectionRequestId, GatewayAttachment>();
	#clientSessionPort: GatewayMessagePort | undefined;
	readonly #connectionId: GatewayClientConnectionId;
	readonly #dependencies: GatewayPageConnectionDependencies;
	readonly #pendingProjections = new Map<TerminalProjectionRequestId, PendingProjection>();
	#projectionPort: GatewayMessagePort | undefined;
	readonly #socket: NodeWebSocket;
	#state: GatewayClientLifecycleState = { phase: 'awaiting-authentication' };
	#started = false;

	constructor(
		socket: NodeWebSocket,
		connectionId: GatewayClientConnectionId,
		dependencies: GatewayPageConnectionDependencies,
	) {
		this.#socket = socket;
		this.#connectionId = connectionId;
		this.#dependencies = dependencies;
	}

	start(): void {
		if (this.#started || this.#state.phase === 'closed') return;
		this.#started = true;
		this.#socket.on('message', this.#handleControlMessage);
		this.#socket.on('close', this.#handleSocketClose);
		this.#socket.on('error', this.#handleSocketError);
	}

	acceptDesktopClient(
		bootstrap: WorkbenchClientSessionBootstrap,
		clientSessionPort: GatewayMessagePort,
		projectionPort: GatewayMessagePort,
	): void {
		this.#transition(
			{ connectionId: this.#connectionId, type: 'desktop-client-opened' },
			{ bootstrap, clientSessionPort, projectionPort },
		);
	}

	rejectDesktopClient(): void {
		this.#postControl({
			code: 'client-session-unavailable',
			retryable: true,
			type: 'gateway-control-failed',
		});
		this.#transition({ type: 'desktop-client-open-failed' });
	}

	dispose(): void {
		this.#transition({ type: 'gateway-stopping' });
	}

	readonly #handleControlMessage = (data: RawData, isBinary: boolean): void => {
		if (this.#state.phase === 'closed') return;
		if (isBinary) {
			this.#rejectProtocol();
			return;
		}
		const text = rawDataToText(data);
		const decoded = text === null ? null : decodeGatewayControlClientMessage(text);
		if (decoded === null || !decoded.ok) {
			this.#rejectProtocol();
			return;
		}
		const message = decoded.value;
		if (this.#state.phase === 'awaiting-authentication') {
			if (message.type !== 'gateway-control-authenticate') {
				this.#rejectProtocol();
				return;
			}
			this.#transition({
				accepted: capabilitiesEqual(message.capability, this.#dependencies.capability),
				connectionId: this.#connectionId,
				type: 'authentication-completed',
			});
			return;
		}
		if (this.#state.phase !== 'active') {
			this.#rejectProtocol();
			return;
		}
		if (message.type === 'workbench-client-session-command') {
			try {
				this.#clientSessionPort?.postMessage(message.command);
			} catch {
				this.#transition({ type: 'protocol-rejected' });
			}
			return;
		}
		if (message.type !== 'terminal-projection-control-command') {
			this.#rejectProtocol();
			return;
		}
		const command = message.command;
		const existing = this.#pendingProjections.get(command.requestId);
		if (command.type === 'terminal-projection-open-requested') {
			if (existing !== undefined) {
				if (existing.presentationId !== command.presentationId) this.#rejectProtocol();
				return;
			}
			this.#pendingProjections.set(command.requestId, {
				presentationId: command.presentationId,
				requestId: command.requestId,
			});
			try {
				this.#projectionPort?.postMessage(command);
			} catch {
				this.#transition({ type: 'protocol-rejected' });
			}
			return;
		}
		if (existing === undefined || existing.presentationId !== command.presentationId) {
			this.#rejectProtocol();
			return;
		}
		this.#pendingProjections.delete(command.requestId);
		try {
			this.#projectionPort?.postMessage(command);
		} catch {
			this.#transition({ type: 'protocol-rejected' });
			return;
		}
		this.#attachments.get(command.requestId)?.retire(false);
	};

	readonly #handleSocketClose = (): void => {
		this.#transition({ type: 'socket-closed' });
	};

	readonly #handleSocketError = (): void => {
		this.#transition({ type: 'protocol-rejected' });
	};

	readonly #handleClientSessionMessage = (message: GatewayPortMessageEvent): void => {
		if (message.ports.length !== 0) {
			closePorts(message.ports);
			this.#transition({ type: 'protocol-rejected' });
			return;
		}
		const event = WorkbenchClientSessionEventSchema.safeParse(message.data);
		if (!event.success) {
			this.#transition({ type: 'protocol-rejected' });
			return;
		}
		if (
			!this.#postControl({
				event: event.data,
				type: 'workbench-client-session-event',
			})
		) {
			this.#transition({ type: 'protocol-rejected' });
		}
	};

	readonly #handleProjectionMessage = (message: GatewayPortMessageEvent): void => {
		const parsed = TerminalProjectionControlEventSchema.safeParse(message.data);
		if (!parsed.success) {
			closePorts(message.ports);
			this.#transition({ type: 'protocol-rejected' });
			return;
		}
		const event = parsed.data;
		const pending = this.#pendingProjections.get(event.requestId);
		if (pending === undefined || pending.presentationId !== event.presentationId) {
			closePorts(message.ports);
			this.#transition({ type: 'protocol-rejected' });
			return;
		}
		if (event.type === 'terminal-projection-failed') {
			if (message.ports.length !== 0) {
				closePorts(message.ports);
				this.#transition({ type: 'protocol-rejected' });
				return;
			}
			this.#pendingProjections.delete(event.requestId);
			if (!this.#postControl({ event, type: 'terminal-projection-control-event' })) {
				this.#transition({ type: 'protocol-rejected' });
			}
			return;
		}
		const attachmentPort = message.ports[0];
		if (attachmentPort === undefined || message.ports.length !== 1) {
			closePorts(message.ports);
			this.#transition({ type: 'protocol-rejected' });
			return;
		}
		const socketId = this.#dependencies.createAttachmentSocketId();
		const attachmentCapability = this.#dependencies.deriveAttachmentCapability(socketId);
		const attachment = new GatewayAttachment({
			attachmentCapability,
			bootstrap: event.bootstrap,
			port: attachmentPort,
			retired: (releaseProjection) => {
				if (this.#attachments.get(event.requestId) === attachment) {
					this.#attachments.delete(event.requestId);
				}
				this.#dependencies.unregisterAttachment(socketId);
				if (releaseProjection) this.#releaseProjection(pending);
			},
			socketId,
		});
		this.#attachments.set(event.requestId, attachment);
		this.#dependencies.registerAttachment(socketId, attachment);
		if (
			!this.#postControl({
				attachmentCapability,
				attachmentSocketId: socketId,
				projection: event,
				type: 'terminal-projection-opened',
			})
		) {
			attachment.retire(true);
		}
	};

	readonly #handleClientSessionPortClose = (): void => {
		this.#transition({ type: 'protocol-rejected' });
	};

	readonly #handleProjectionPortClose = (): void => {
		this.#transition({ type: 'protocol-rejected' });
	};

	#releaseProjection(projection: PendingProjection): void {
		if (this.#pendingProjections.get(projection.requestId) !== projection) return;
		this.#pendingProjections.delete(projection.requestId);
		if (this.#state.phase !== 'active') return;
		try {
			this.#projectionPort?.postMessage({
				presentationId: projection.presentationId,
				requestId: projection.requestId,
				type: 'terminal-projection-release-requested',
			});
		} catch {
			this.#transition({ type: 'protocol-rejected' });
		}
	}

	#rejectProtocol(): void {
		this.#postControl({
			code: 'protocol-rejected',
			retryable: false,
			type: 'gateway-control-failed',
		});
		this.#transition({ type: 'protocol-rejected' });
	}

	#transition(
		event: GatewayClientLifecycleEvent,
		ports?: {
			readonly bootstrap: WorkbenchClientSessionBootstrap;
			readonly clientSessionPort: GatewayMessagePort;
			readonly projectionPort: GatewayMessagePort;
		},
	): void {
		const transition = transitionGatewayClientLifecycle(this.#state, event);
		this.#state = transition.state;
		for (const command of transition.commands) this.#applyLifecycleCommand(command, ports);
		if (this.#state.phase === 'closed') this.#retireLocalResources();
	}

	#applyLifecycleCommand(
		command: GatewayClientLifecycleCommand,
		ports:
			| {
					readonly bootstrap: WorkbenchClientSessionBootstrap;
					readonly clientSessionPort: GatewayMessagePort;
					readonly projectionPort: GatewayMessagePort;
			  }
			| undefined,
	): void {
		switch (command.type) {
			case 'request-client-open':
				this.#dependencies.postParent({
					connectionId: command.connectionId,
					type: 'gateway-client-open-requested',
				});
				return;
			case 'request-client-close':
				this.#dependencies.postParent({
					connectionId: command.connectionId,
					type: 'gateway-client-close-requested',
				});
				return;
			case 'accept-desktop-ports':
				if (ports === undefined) {
					this.#transition({ type: 'protocol-rejected' });
					return;
				}
				this.#clientSessionPort = ports.clientSessionPort;
				this.#projectionPort = ports.projectionPort;
				ports.clientSessionPort.on('message', this.#handleClientSessionMessage);
				ports.clientSessionPort.on('close', this.#handleClientSessionPortClose);
				ports.projectionPort.on('message', this.#handleProjectionMessage);
				ports.projectionPort.on('close', this.#handleProjectionPortClose);
				if (!this.#postControl({ bootstrap: ports.bootstrap, type: 'gateway-control-ready' })) {
					this.#transition({ type: 'protocol-rejected' });
					return;
				}
				ports.clientSessionPort.start();
				ports.projectionPort.start();
				return;
			case 'dispose-desktop-ports':
				if (ports !== undefined) closePorts([ports.clientSessionPort, ports.projectionPort]);
				return;
			case 'close-control-socket':
				if (
					this.#socket.readyState === NodeWebSocket.OPEN ||
					this.#socket.readyState === NodeWebSocket.CONNECTING
				) {
					this.#socket.close(1008, 'Gateway control closed.');
				}
				return;
		}
	}

	#postControl(message: GatewayControlServerMessage): boolean {
		if (this.#socket.readyState !== NodeWebSocket.OPEN) return false;
		let encoded: string;
		try {
			encoded = encodeGatewayControlServerMessage(message);
		} catch {
			return false;
		}
		if (
			this.#socket.bufferedAmount + Buffer.byteLength(encoded) >
			GATEWAY_LIMITS.maxControlBufferedBytes
		) {
			return false;
		}
		this.#socket.send(encoded, (error) => {
			if (error !== undefined && error !== null) {
				this.#transition({ type: 'protocol-rejected' });
			}
		});
		return true;
	}

	#retireLocalResources(): void {
		if (this.#started) {
			this.#socket.off('message', this.#handleControlMessage);
			this.#socket.off('close', this.#handleSocketClose);
			this.#socket.off('error', this.#handleSocketError);
		}
		for (const attachment of this.#attachments.values()) attachment.retire(false);
		this.#attachments.clear();
		this.#pendingProjections.clear();
		const clientSessionPort = this.#clientSessionPort;
		this.#clientSessionPort = undefined;
		const projectionPort = this.#projectionPort;
		this.#projectionPort = undefined;
		if (clientSessionPort !== undefined) {
			clientSessionPort.off('message', this.#handleClientSessionMessage);
			clientSessionPort.off('close', this.#handleClientSessionPortClose);
			clientSessionPort.close();
		}
		if (projectionPort !== undefined) {
			projectionPort.off('message', this.#handleProjectionMessage);
			projectionPort.off('close', this.#handleProjectionPortClose);
			projectionPort.close();
		}
		this.#dependencies.retired(this.#connectionId);
	}
}

interface GatewayAttachmentOptions {
	readonly attachmentCapability: GatewayCapability;
	readonly bootstrap: ProjectedAttachmentBootstrap;
	readonly port: GatewayMessagePort;
	readonly retired: (releaseProjection: boolean) => void;
	readonly socketId: ReturnType<typeof GatewayAttachmentSocketIdSchema.parse>;
}

class GatewayAttachment implements AttachmentRegistration {
	readonly #options: GatewayAttachmentOptions;
	#retired = false;
	#socket: NodeWebSocket | undefined;

	constructor(options: GatewayAttachmentOptions) {
		this.#options = options;
	}

	accept(socket: NodeWebSocket, authentication: GatewayAttachmentClientMessage): void {
		if (
			this.#retired ||
			this.#socket !== undefined ||
			authentication.attachmentSocketId !== this.#options.socketId ||
			!capabilitiesEqual(authentication.attachmentCapability, this.#options.attachmentCapability)
		) {
			sendAttachmentFailure(socket, 'authentication-rejected', false);
			return;
		}
		this.#socket = socket;
		socket.on('message', this.#handleSocketMessage);
		socket.on('close', this.#handleSocketClose);
		socket.on('error', this.#handleSocketError);
		this.#options.port.on('message', this.#handlePortMessage);
		this.#options.port.on('close', this.#handlePortClose);
		const ready = encodeGatewayAttachmentServerMessage({
			attachmentSocketId: this.#options.socketId,
			type: 'gateway-attachment-ready',
		});
		socket.send(ready, (error) => {
			if (error !== undefined && error !== null) this.retire(true);
		});
		this.#options.port.start();
	}

	reject(): void {
		this.retire(false);
	}

	retire(releaseProjection: boolean): void {
		if (this.#retired) return;
		this.#retired = true;
		const socket = this.#socket;
		this.#socket = undefined;
		if (socket !== undefined) {
			socket.off('message', this.#handleSocketMessage);
			socket.off('close', this.#handleSocketClose);
			socket.off('error', this.#handleSocketError);
			if (socket.readyState === NodeWebSocket.OPEN) {
				socket.close(1008, 'Gateway attachment retired.');
			}
		}
		this.#options.port.off('message', this.#handlePortMessage);
		this.#options.port.off('close', this.#handlePortClose);
		this.#options.port.close();
		this.#options.retired(releaseProjection);
	}

	readonly #handleSocketMessage = (data: RawData, isBinary: boolean): void => {
		if (!isBinary) {
			this.retire(true);
			return;
		}
		const frame = rawDataToArrayBuffer(data);
		const decoded = decodeGatewayAttachmentClientFrame(frame);
		if (
			!decoded.ok ||
			!commandMatchesAttachment(decoded.value, this.#options.bootstrap.attachmentId)
		) {
			this.retire(true);
			return;
		}
		try {
			this.#options.port.postMessage(ProjectedTerminalCommandSchema.parse(decoded.value));
		} catch {
			this.retire(true);
		}
	};

	readonly #handleSocketClose = (): void => this.retire(true);
	readonly #handleSocketError = (): void => this.retire(true);
	readonly #handlePortClose = (): void => this.retire(false);

	readonly #handlePortMessage = (message: GatewayPortMessageEvent): void => {
		if (message.ports.length !== 0) {
			closePorts(message.ports);
			this.retire(true);
			return;
		}
		const event = ProjectedTerminalEventSchema.safeParse(message.data);
		const socket = this.#socket;
		if (!event.success || socket === undefined || socket.readyState !== NodeWebSocket.OPEN) {
			this.retire(true);
			return;
		}
		let frame: ArrayBuffer;
		try {
			frame = encodeGatewayAttachmentServerFrame(event.data);
		} catch {
			this.retire(true);
			return;
		}
		if (socket.bufferedAmount + frame.byteLength > GATEWAY_LIMITS.maxAttachmentBufferedBytes) {
			this.retire(true);
			return;
		}
		socket.send(frame, { binary: true }, (error) => {
			if (error !== undefined && error !== null) this.retire(true);
		});
	};
}

function commandMatchesAttachment(
	command: ProjectedTerminalCommand,
	attachmentId: ProjectedAttachmentBootstrap['attachmentId'],
): boolean {
	return command.payload.attachmentId === attachmentId;
}

function capabilitiesEqual(left: GatewayCapability, right: GatewayCapability): boolean {
	const leftBytes = Buffer.from(left);
	const rightBytes = Buffer.from(right);
	return leftBytes.byteLength === rightBytes.byteLength && timingSafeEqual(leftBytes, rightBytes);
}

function isExactLoopbackOrigin(value: string): boolean {
	try {
		const url = new URL(value);
		return (
			url.origin === value &&
			url.protocol === 'http:' &&
			url.hostname === '127.0.0.1' &&
			url.username === '' &&
			url.password === ''
		);
	} catch {
		return false;
	}
}

function requestPathname(value: string | undefined): string | null {
	try {
		return new URL(value ?? '/', 'http://127.0.0.1').pathname;
	} catch {
		return null;
	}
}

function rejectUpgrade(socket: Duplex): void {
	socket.end('HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n');
}

function listenLoopback(server: Server): Promise<void> {
	return new Promise((resolve, reject) => {
		const handleError = (error: Error): void => reject(error);
		server.once('error', handleError);
		server.listen(0, '127.0.0.1', () => {
			server.off('error', handleError);
			resolve();
		});
	});
}

function closeServer(server: Server): Promise<void> {
	if (!server.listening) return Promise.resolve();
	return new Promise((resolve) => server.close(() => resolve()));
}

function closePorts(ports: readonly GatewayMessagePort[]): void {
	for (const port of ports) port.close();
}

function rawDataToText(data: RawData): string | null {
	try {
		return new TextDecoder('utf-8', { fatal: true }).decode(rawDataToBytes(data));
	} catch {
		return null;
	}
}

function rawDataToArrayBuffer(data: RawData): ArrayBuffer {
	const bytes = rawDataToBytes(data);
	const copy = new Uint8Array(bytes.byteLength);
	copy.set(bytes);
	return copy.buffer;
}

function rawDataToBytes(data: RawData): Uint8Array {
	if (Array.isArray(data)) return Buffer.concat(data);
	if (data instanceof ArrayBuffer) return new Uint8Array(data);
	return data;
}

function sendAttachmentFailure(
	socket: NodeWebSocket,
	code: 'authentication-rejected' | 'attachment-retired' | 'protocol-rejected',
	retryable: boolean,
): void {
	if (socket.readyState !== NodeWebSocket.OPEN) return;
	socket.send(
		encodeGatewayAttachmentServerMessage({ code, retryable, type: 'gateway-attachment-failed' }),
		() => socket.close(1008, 'Gateway attachment rejected.'),
	);
}
