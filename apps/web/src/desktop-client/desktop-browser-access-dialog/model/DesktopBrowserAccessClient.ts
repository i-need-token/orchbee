import {
	DesktopBrowserAccessMainEventSchema,
	DesktopBrowserAccessRendererCommandSchema,
	DesktopBrowserAccessRequestIdSchema,
	type DesktopBrowserAccessFailureCode,
	type DesktopBrowserAccessMode,
	type DesktopBrowserAccessRendererCommand,
	type DesktopBrowserAccessRequestId,
	type DesktopBrowserAccessState,
} from '@orchbee/desktop-client-protocol';

import { createBrowserRandomIdentifier } from '../../../browser-random-identifier';

interface PendingRequest {
	reject(error: DesktopBrowserAccessClientError): void;
	resolve(state: DesktopBrowserAccessState): void;
}

class DesktopBrowserAccessClientError extends Error {
	readonly code: DesktopBrowserAccessFailureCode;
	readonly state: DesktopBrowserAccessState;

	constructor(code: DesktopBrowserAccessFailureCode, state: DesktopBrowserAccessState) {
		super(`Desktop Browser access failed with ${code}.`);
		this.name = 'DesktopBrowserAccessClientError';
		this.code = code;
		this.state = state;
	}
}

export interface DesktopBrowserAccessClientPort {
	copyAccessLink(origin: string): Promise<DesktopBrowserAccessState>;
	dispose(): void;
	getState(): Promise<DesktopBrowserAccessState>;
	openLocalBrowser(): Promise<DesktopBrowserAccessState>;
	start(mode: DesktopBrowserAccessMode): Promise<DesktopBrowserAccessState>;
	stop(): Promise<DesktopBrowserAccessState>;
}

export class DesktopBrowserAccessClient implements DesktopBrowserAccessClientPort {
	readonly #createRequestId: () => DesktopBrowserAccessRequestId;
	#disposed = false;
	readonly #pending = new Map<DesktopBrowserAccessRequestId, PendingRequest>();
	readonly #port: MessagePort;
	#sequence = 0;
	#state: DesktopBrowserAccessState = { phase: 'stopped' };

	constructor(port: MessagePort, createRequestId: () => string = defaultRequestId) {
		this.#port = port;
		this.#createRequestId = () =>
			DesktopBrowserAccessRequestIdSchema.parse(`${createRequestId()}:${++this.#sequence}`);
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	getState(): Promise<DesktopBrowserAccessState> {
		return this.#request((requestId) => ({
			requestId,
			type: 'desktop-browser-access-state-requested',
		}));
	}

	start(mode: DesktopBrowserAccessMode): Promise<DesktopBrowserAccessState> {
		return this.#request((requestId) => ({
			mode,
			requestId,
			type: 'desktop-browser-access-start-requested',
		}));
	}

	openLocalBrowser(): Promise<DesktopBrowserAccessState> {
		return this.#request((requestId) => ({
			requestId,
			type: 'desktop-browser-access-open-requested',
		}));
	}

	copyAccessLink(origin: string): Promise<DesktopBrowserAccessState> {
		return this.#request((requestId) => ({
			origin,
			requestId,
			type: 'desktop-browser-access-copy-link-requested',
		}));
	}

	stop(): Promise<DesktopBrowserAccessState> {
		return this.#request((requestId) => ({
			requestId,
			type: 'desktop-browser-access-stop-requested',
		}));
	}

	dispose(): void {
		this.#retire('desktop-quiescing');
	}

	#request(
		createCommand: (
			requestId: DesktopBrowserAccessRequestId,
		) => DesktopBrowserAccessRendererCommand,
	): Promise<DesktopBrowserAccessState> {
		if (this.#disposed) {
			return Promise.reject(new DesktopBrowserAccessClientError('desktop-quiescing', this.#state));
		}
		const requestId = this.#createRequestId();
		const command = DesktopBrowserAccessRendererCommandSchema.parse(createCommand(requestId));
		const result = new Promise<DesktopBrowserAccessState>((resolve, reject) => {
			this.#pending.set(requestId, { reject, resolve });
		});
		try {
			this.#port.postMessage(command);
		} catch {
			this.#retire('gateway-unavailable');
		}
		return result;
	}

	readonly #handleMessage = (event: MessageEvent<unknown>): void => {
		if (this.#disposed || event.ports.length !== 0) {
			this.#retire('gateway-unavailable');
			return;
		}
		const parsed = DesktopBrowserAccessMainEventSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire('gateway-unavailable');
			return;
		}
		const publication = parsed.data;
		const pending = this.#pending.get(publication.requestId);
		if (pending === undefined) {
			this.#retire('gateway-unavailable');
			return;
		}
		this.#pending.delete(publication.requestId);
		this.#state = publication.state;
		if (publication.type === 'desktop-browser-access-completed') {
			pending.resolve(publication.state);
		} else {
			pending.reject(new DesktopBrowserAccessClientError(publication.code, publication.state));
		}
	};

	readonly #handleMessageError = (): void => {
		this.#retire('gateway-unavailable');
	};

	#retire(code: DesktopBrowserAccessFailureCode): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		this.#port.close();
		for (const pending of this.#pending.values()) {
			pending.reject(new DesktopBrowserAccessClientError(code, this.#state));
		}
		this.#pending.clear();
	}
}

function defaultRequestId(): string {
	return `browser-access:${createBrowserRandomIdentifier()}`;
}
