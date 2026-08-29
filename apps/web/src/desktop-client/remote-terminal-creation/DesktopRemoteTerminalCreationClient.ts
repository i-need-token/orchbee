import {
	DesktopRemoteTerminalCreationMainEventSchema,
	DesktopRemoteTerminalCreationRendererCommandSchema,
	DesktopRemoteTerminalCreationRequestIdSchema,
	type DesktopRemoteTerminalCreationRequestId,
} from '@orchbee/desktop-client-protocol';
import type {
	WorkbenchRemoteTerminalCreationCapability,
	WorkbenchRemoteTerminalCreationResult,
} from '@orchbee/workbench';

import { createBrowserRandomIdentifier } from '../../browser-random-identifier';

interface PendingRequest {
	readonly abortListener: () => void;
	dispatched: boolean;
	readonly requestId: DesktopRemoteTerminalCreationRequestId;
	resolve(result: WorkbenchRemoteTerminalCreationResult): void;
	readonly signal: AbortSignal;
}

export class DesktopRemoteTerminalCreationClient implements WorkbenchRemoteTerminalCreationCapability {
	readonly #createRequestId: () => DesktopRemoteTerminalCreationRequestId;
	#disposed = false;
	#pending: PendingRequest | undefined;
	readonly #port: MessagePort;
	#sequence = 0;

	constructor(port: MessagePort, createRequestId: () => string = defaultRequestId) {
		this.#port = port;
		this.#createRequestId = () =>
			DesktopRemoteTerminalCreationRequestIdSchema.parse(
				`${createRequestId()}:${++this.#sequence}`,
			);
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	create(
		input: Parameters<WorkbenchRemoteTerminalCreationCapability['create']>[0],
	): Promise<WorkbenchRemoteTerminalCreationResult> {
		if (input.signal.aborted) return Promise.resolve({ kind: 'cancelled' });
		if (this.#disposed) {
			return Promise.resolve({ kind: 'failed', reason: 'control-unavailable' });
		}
		if (this.#pending !== undefined) {
			return Promise.resolve({ kind: 'failed', reason: 'busy' });
		}
		const requestId = this.#createRequestId();
		const command = DesktopRemoteTerminalCreationRendererCommandSchema.parse({
			projectId: input.projectId,
			requestId,
			type: 'desktop-remote-terminal-creation-requested',
			workspaceId: input.workspaceId,
		});
		const deferred = createDeferred<WorkbenchRemoteTerminalCreationResult>();
		const abortListener = (): void => this.#requestCancellation(requestId);
		const pending: PendingRequest = {
			abortListener,
			dispatched: false,
			requestId,
			resolve: deferred.resolve,
			signal: input.signal,
		};
		this.#pending = pending;
		input.signal.addEventListener('abort', abortListener, { once: true });
		try {
			this.#port.postMessage(command);
			pending.dispatched = true;
		} catch {
			this.#retire(true, { kind: 'failed', reason: 'control-unavailable' });
		}
		return deferred.promise;
	}

	dispose(): void {
		this.#retire(true);
	}

	readonly #handleMessage = (event: MessageEvent<unknown>): void => {
		if (this.#disposed || event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = DesktopRemoteTerminalCreationMainEventSchema.safeParse(event.data);
		const pending = this.#pending;
		if (!parsed.success || pending === undefined || parsed.data.requestId !== pending.requestId) {
			this.#retire(true);
			return;
		}
		this.#settle(parsed.data.result);
	};

	readonly #handleMessageError = (): void => this.#retire(true);

	#requestCancellation(requestId: DesktopRemoteTerminalCreationRequestId): void {
		const pending = this.#pending;
		if (pending === undefined || pending.requestId !== requestId || !pending.dispatched) return;
		try {
			this.#port.postMessage(
				DesktopRemoteTerminalCreationRendererCommandSchema.parse({
					requestId,
					type: 'desktop-remote-terminal-creation-cancel-requested',
				}),
			);
		} catch {
			this.#retire(true);
		}
	}

	#settle(result: WorkbenchRemoteTerminalCreationResult): void {
		const pending = this.#pending;
		if (pending === undefined) return;
		this.#pending = undefined;
		pending.signal.removeEventListener('abort', pending.abortListener);
		pending.resolve(result);
	}

	#retire(closePort: boolean, preDispatchResult?: WorkbenchRemoteTerminalCreationResult): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		const pending = this.#pending;
		if (pending !== undefined) {
			this.#settle(
				preDispatchResult ?? {
					kind: 'indeterminate',
					reason: 'control-unavailable',
				},
			);
		}
		if (closePort) this.#port.close();
	}
}

function defaultRequestId(): string {
	return `remote-terminal:${createBrowserRandomIdentifier()}`;
}

function createDeferred<Value>(): {
	readonly promise: Promise<Value>;
	readonly resolve: (value: Value) => void;
} {
	let resolvePromise: (value: Value) => void = () => undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return { promise, resolve: resolvePromise };
}
