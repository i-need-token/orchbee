import type {
	TerminalPresentationFailure,
	TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';

import {
	TerminalProjectionControlEventSchema,
	TerminalProjectionRequestIdSchema,
	type ProjectedAttachmentBootstrap,
	type TerminalProjectionRequestId,
} from '@orchbee/desktop-client-protocol';

interface DesktopTerminalProjection {
	readonly bootstrap: ProjectedAttachmentBootstrap;
	readonly port: MessagePort;
}

interface DesktopTerminalProjectionRequest {
	readonly result: Promise<DesktopTerminalProjection>;
	release(): void;
}

class DesktopTerminalProjectionRequestError extends Error {
	readonly failure: TerminalPresentationFailure;

	constructor(failure: TerminalPresentationFailure) {
		super(failure.code);
		this.name = 'DesktopTerminalProjectionRequestError';
		this.failure = failure;
	}
}

interface PendingProjection {
	port: MessagePort | null;
	readonly presentationId: TerminalPresentationId;
	readonly reject: (error: Error) => void;
	released: boolean;
	readonly resolve: (projection: DesktopTerminalProjection) => void;
}

export class DesktopTerminalProjectionClient {
	readonly #createRequestId: () => TerminalProjectionRequestId;
	readonly #pending = new Map<TerminalProjectionRequestId, PendingProjection>();
	readonly #port: MessagePort;
	#disposed = false;

	constructor(
		port: MessagePort,
		createRequestId: () => TerminalProjectionRequestId = () =>
			TerminalProjectionRequestIdSchema.parse(`desktop:projection:${crypto.randomUUID()}`),
	) {
		this.#port = port;
		this.#createRequestId = createRequestId;
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	open(presentationId: TerminalPresentationId): DesktopTerminalProjectionRequest {
		const requestId = this.#createRequestId();
		let resolve = (_projection: DesktopTerminalProjection): void => undefined;
		let reject = (_error: Error): void => undefined;
		const result = new Promise<DesktopTerminalProjection>((nextResolve, nextReject) => {
			resolve = nextResolve;
			reject = nextReject;
		});
		const pending: PendingProjection = {
			port: null,
			presentationId,
			reject,
			released: false,
			resolve,
		};
		this.#pending.set(requestId, pending);
		if (this.#disposed) {
			this.#pending.delete(requestId);
			reject(unavailableError());
		} else {
			try {
				this.#port.postMessage({
					presentationId,
					requestId,
					type: 'terminal-projection-open-requested',
				});
			} catch {
				this.#pending.delete(requestId);
				reject(unavailableError());
			}
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
			pending.port?.close();
			pending.reject(unavailableError());
		}
		this.#pending.clear();
		this.#port.close();
	}

	readonly #handleMessage = (message: MessageEvent<unknown>): void => {
		if (this.#disposed) return;
		const event = TerminalProjectionControlEventSchema.safeParse(message.data);
		if (!event.success) {
			this.dispose();
			return;
		}
		const pending = this.#pending.get(event.data.requestId);
		if (
			pending === undefined ||
			pending.presentationId !== event.data.presentationId ||
			pending.released
		) {
			for (const port of message.ports) port.close();
			this.dispose();
			return;
		}
		if (event.data.type === 'terminal-projection-failed') {
			if (message.ports.length !== 0) {
				for (const port of message.ports) port.close();
				this.dispose();
				return;
			}
			this.#pending.delete(event.data.requestId);
			pending.reject(new DesktopTerminalProjectionRequestError(event.data.failure));
			return;
		}
		const rendererPort = message.ports[0];
		if (rendererPort === undefined || message.ports.length !== 1 || pending.port !== null) {
			for (const port of message.ports) port.close();
			this.dispose();
			return;
		}
		pending.port = rendererPort;
		pending.resolve({ bootstrap: event.data.bootstrap, port: rendererPort });
	};

	readonly #handleMessageError = (): void => {
		this.dispose();
	};

	#release(requestId: TerminalProjectionRequestId, pending: PendingProjection): void {
		if (pending.released) return;
		pending.released = true;
		this.#pending.delete(requestId);
		pending.port?.close();
		if (pending.port === null) pending.reject(releasedError());
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
