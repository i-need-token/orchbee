import {
	TerminalProjectionControlEventSchema,
	TerminalProjectionRequestIdSchema,
	type ProjectedTerminalCommand,
	type TerminalProjectionRequestId,
	type TerminalPresentationFailure,
	type TerminalPresentationId,
} from '@orchbee/workbench-client-protocol';
import type { TerminalGeometry } from '@orchbee/terminal-protocol';

import { createBrowserRandomIdentifier } from '../../browser-random-identifier';
import type {
	ProjectedTerminalTransport,
	TerminalProjection,
	TerminalProjectionClient,
	TerminalProjectionRequest,
} from '../../workbench-client';

class DesktopTerminalProjectionRequestError extends Error {
	readonly failure: TerminalPresentationFailure;

	constructor(failure: TerminalPresentationFailure) {
		super(failure.code);
		this.name = 'DesktopTerminalProjectionRequestError';
		this.failure = failure;
	}
}

interface PendingProjection {
	readonly presentationId: TerminalPresentationId;
	readonly reject: (error: Error) => void;
	released: boolean;
	readonly resolve: (projection: TerminalProjection) => void;
	transport: ProjectedTerminalTransport | null;
}

export class DesktopTerminalProjectionClient implements TerminalProjectionClient {
	readonly #createRequestId: () => TerminalProjectionRequestId;
	readonly #pending = new Map<TerminalProjectionRequestId, PendingProjection>();
	readonly #port: MessagePort;
	#disposed = false;

	constructor(
		port: MessagePort,
		createRequestId: () => TerminalProjectionRequestId = () =>
			TerminalProjectionRequestIdSchema.parse(
				`desktop:projection:${createBrowserRandomIdentifier()}`,
			),
	) {
		this.#port = port;
		this.#createRequestId = createRequestId;
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	open(
		presentationId: TerminalPresentationId,
		geometry: TerminalGeometry,
	): TerminalProjectionRequest {
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
			this.#port.postMessage({
				geometry,
				presentationId,
				requestId,
				type: 'terminal-projection-open-requested',
			});
		} catch {
			this.#pending.delete(requestId);
			reject(unavailableError());
		}
		return {
			release: () => this.#release(requestId, pending),
			result,
		};
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		for (const pending of this.#pending.values()) {
			pending.released = true;
			pending.transport?.close();
			pending.reject(unavailableError());
		}
		this.#pending.clear();
		this.#port.close();
	}

	readonly #handleMessage = (message: MessageEvent<unknown>): void => {
		if (this.#disposed) return;
		const event = TerminalProjectionControlEventSchema.safeParse(message.data);
		if (!event.success) {
			closePorts(message.ports);
			this.dispose();
			return;
		}
		const pending = this.#pending.get(event.data.requestId);
		if (pending === undefined || pending.presentationId !== event.data.presentationId) {
			closePorts(message.ports);
			this.dispose();
			return;
		}
		if (event.data.type === 'terminal-projection-released') {
			if (message.ports.length !== 0) {
				closePorts(message.ports);
				this.dispose();
				return;
			}
			if (!pending.released) {
				pending.released = true;
				pending.transport?.close();
				pending.reject(releasedError());
			}
			this.#pending.delete(event.data.requestId);
			return;
		}
		if (event.data.type === 'terminal-projection-failed') {
			if (message.ports.length !== 0) {
				closePorts(message.ports);
				this.dispose();
				return;
			}
			this.#pending.delete(event.data.requestId);
			if (!pending.released) {
				pending.reject(new DesktopTerminalProjectionRequestError(event.data.failure));
			}
			return;
		}
		if (pending.released) {
			if (message.ports.length !== 1 || pending.transport !== null) {
				closePorts(message.ports);
				this.dispose();
				return;
			}
			closePorts(message.ports);
			return;
		}
		const rendererPort = message.ports[0];
		if (rendererPort === undefined || message.ports.length !== 1 || pending.transport !== null) {
			closePorts(message.ports);
			this.dispose();
			return;
		}
		const transport = new MessagePortProjectedTerminalTransport(rendererPort);
		pending.transport = transport;
		pending.resolve({ bootstrap: event.data.bootstrap, transport });
	};

	readonly #handleMessageError = (): void => {
		this.dispose();
	};

	#release(requestId: TerminalProjectionRequestId, pending: PendingProjection): void {
		if (pending.released) return;
		pending.released = true;
		pending.transport?.close();
		pending.reject(releasedError());
		if (this.#disposed) return;
		try {
			this.#port.postMessage({
				presentationId: pending.presentationId,
				requestId,
				type: 'terminal-projection-release-requested',
			});
		} catch {
			this.dispose();
		}
	}
}

class MessagePortProjectedTerminalTransport implements ProjectedTerminalTransport {
	readonly #port: MessagePort;
	readonly projectionBufferTrust = 'local-host' as const;

	constructor(port: MessagePort) {
		this.#port = port;
	}

	close(): void {
		this.#port.close();
	}

	send(command: ProjectedTerminalCommand): void {
		this.#port.postMessage(command);
	}

	subscribe(listener: (value: unknown) => void, unavailable: () => void): () => void {
		const handleMessage = (message: MessageEvent<unknown>): void => {
			if (message.ports.length !== 0) {
				closePorts(message.ports);
				unavailable();
				return;
			}
			listener(message.data);
		};
		const handleMessageError = (): void => unavailable();
		this.#port.addEventListener('message', handleMessage);
		this.#port.addEventListener('messageerror', handleMessageError);
		this.#port.start();
		return () => {
			this.#port.removeEventListener('message', handleMessage);
			this.#port.removeEventListener('messageerror', handleMessageError);
		};
	}
}

function closePorts(ports: readonly MessagePort[]): void {
	for (const port of ports) port.close();
}

function unavailableError(): DesktopTerminalProjectionRequestError {
	return new DesktopTerminalProjectionRequestError({
		code: 'terminal-projection-unavailable',
		retryable: true,
	});
}

function releasedError(): DesktopTerminalProjectionRequestError {
	return new DesktopTerminalProjectionRequestError({
		code: 'terminal-projection-released',
		retryable: false,
	});
}
