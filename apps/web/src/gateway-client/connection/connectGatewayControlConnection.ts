import {
	encodeGatewayControlClientMessage,
	GATEWAY_CONTROL_PATH,
	GatewayControlClientMessageSchema,
	decodeGatewayControlServerMessage,
	type GatewayBrowserConnection,
	type GatewayControlServerMessage,
} from '@orchbee/gateway-protocol';
import type {
	TerminalProjectionControlCommandSchema,
	WorkbenchClientSessionBootstrap,
	WorkbenchClientSessionCommandSchema,
} from '@orchbee/workbench-client-protocol';

export interface GatewayControlConnection {
	readonly bootstrap: WorkbenchClientSessionBootstrap;
	close(): void;
	sendClientSession(command: ReturnType<typeof WorkbenchClientSessionCommandSchema.parse>): void;
	sendProjection(command: ReturnType<typeof TerminalProjectionControlCommandSchema.parse>): void;
	subscribeClientSession(listener: (value: unknown) => void, unavailable: () => void): () => void;
	subscribeProjection(
		listener: (message: GatewayControlServerMessage) => void,
		unavailable: () => void,
	): () => void;
}

export type GatewayWebSocketFactory = (url: string) => WebSocket;

export async function connectGatewayControlConnection(
	connection: GatewayBrowserConnection,
	createWebSocket: GatewayWebSocketFactory = (url) => new WebSocket(url),
): Promise<GatewayControlConnection> {
	const socket = createWebSocket(`ws://127.0.0.1:${connection.port}${GATEWAY_CONTROL_PATH}`);
	const control = new BrowserGatewayControlConnection(socket, connection);
	await control.ready;
	return control;
}

class BrowserGatewayControlConnection implements GatewayControlConnection {
	#bootstrap: WorkbenchClientSessionBootstrap | undefined;
	readonly #clientListeners = new Set<(value: unknown) => void>();
	readonly #clientUnavailableListeners = new Set<() => void>();
	readonly #connection: GatewayBrowserConnection;
	#disposed = false;
	readonly #projectionListeners = new Set<(message: GatewayControlServerMessage) => void>();
	readonly #projectionUnavailableListeners = new Set<() => void>();
	readonly ready: Promise<void>;
	#rejectReady: ((error: Error) => void) | undefined;
	#resolveReady: (() => void) | undefined;
	readonly #socket: WebSocket;

	constructor(socket: WebSocket, connection: GatewayBrowserConnection) {
		this.#socket = socket;
		this.#connection = connection;
		this.ready = new Promise((resolve, reject) => {
			this.#resolveReady = resolve;
			this.#rejectReady = reject;
		});
		socket.addEventListener('open', this.#handleOpen);
		socket.addEventListener('message', this.#handleMessage);
		socket.addEventListener('error', this.#handleUnavailable);
		socket.addEventListener('close', this.#handleUnavailable);
	}

	get bootstrap(): WorkbenchClientSessionBootstrap {
		if (this.#bootstrap === undefined) {
			throw new Error('Gateway control bootstrap is unavailable.');
		}
		return this.#bootstrap;
	}

	close(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#removeSocketListeners();
		this.#socket.close();
		this.#clientListeners.clear();
		this.#clientUnavailableListeners.clear();
		this.#projectionListeners.clear();
		this.#projectionUnavailableListeners.clear();
		this.#rejectReady?.(new Error('Gateway control closed before bootstrap.'));
		this.#resolveReady = undefined;
		this.#rejectReady = undefined;
	}

	sendClientSession(command: ReturnType<typeof WorkbenchClientSessionCommandSchema.parse>): void {
		this.#send({ command, type: 'workbench-client-session-command' });
	}

	sendProjection(command: ReturnType<typeof TerminalProjectionControlCommandSchema.parse>): void {
		this.#send({ command, type: 'terminal-projection-control-command' });
	}

	subscribeClientSession(listener: (value: unknown) => void, unavailable: () => void): () => void {
		if (this.#disposed) {
			unavailable();
			return () => undefined;
		}
		this.#clientListeners.add(listener);
		this.#clientUnavailableListeners.add(unavailable);
		return () => {
			this.#clientListeners.delete(listener);
			this.#clientUnavailableListeners.delete(unavailable);
		};
	}

	subscribeProjection(
		listener: (message: GatewayControlServerMessage) => void,
		unavailable: () => void,
	): () => void {
		if (this.#disposed) {
			unavailable();
			return () => undefined;
		}
		this.#projectionListeners.add(listener);
		this.#projectionUnavailableListeners.add(unavailable);
		return () => {
			this.#projectionListeners.delete(listener);
			this.#projectionUnavailableListeners.delete(unavailable);
		};
	}

	readonly #handleOpen = (): void => {
		this.#send({
			capability: this.#connection.capability,
			protocolVersion: this.#connection.protocolVersion,
			type: 'gateway-control-authenticate',
		});
	};

	readonly #handleMessage = (event: MessageEvent<unknown>): void => {
		if (this.#disposed || typeof event.data !== 'string') {
			this.#becomeUnavailable();
			return;
		}
		const decoded = decodeGatewayControlServerMessage(event.data);
		if (!decoded.ok) {
			this.#becomeUnavailable();
			return;
		}
		const message = decoded.value;
		if (this.#bootstrap === undefined) {
			if (message.type !== 'gateway-control-ready') {
				this.#becomeUnavailable();
				return;
			}
			this.#bootstrap = message.bootstrap;
			this.#resolveReady?.();
			this.#resolveReady = undefined;
			this.#rejectReady = undefined;
			return;
		}
		switch (message.type) {
			case 'workbench-client-session-event':
				for (const listener of this.#clientListeners) listener(message.event);
				return;
			case 'terminal-projection-control-event':
			case 'terminal-projection-opened':
				for (const listener of this.#projectionListeners) listener(message);
				return;
			case 'gateway-control-failed':
			case 'gateway-control-ready':
				this.#becomeUnavailable();
				return;
		}
	};

	readonly #handleUnavailable = (): void => this.#becomeUnavailable();

	#send(message: Parameters<typeof GatewayControlClientMessageSchema.parse>[0]): void {
		if (this.#disposed || this.#socket.readyState !== 1) {
			throw new Error('Gateway control connection is unavailable.');
		}
		this.#socket.send(
			encodeGatewayControlClientMessage(GatewayControlClientMessageSchema.parse(message)),
		);
	}

	#becomeUnavailable(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#removeSocketListeners();
		this.#socket.close();
		this.#rejectReady?.(new Error('Gateway control connection is unavailable.'));
		this.#resolveReady = undefined;
		this.#rejectReady = undefined;
		for (const unavailable of this.#clientUnavailableListeners) unavailable();
		for (const unavailable of this.#projectionUnavailableListeners) unavailable();
		this.#clientListeners.clear();
		this.#clientUnavailableListeners.clear();
		this.#projectionListeners.clear();
		this.#projectionUnavailableListeners.clear();
	}

	#removeSocketListeners(): void {
		this.#socket.removeEventListener('open', this.#handleOpen);
		this.#socket.removeEventListener('message', this.#handleMessage);
		this.#socket.removeEventListener('error', this.#handleUnavailable);
		this.#socket.removeEventListener('close', this.#handleUnavailable);
	}
}
