import {
	DesktopSshProjectEnrollmentMainEventSchema,
	DesktopSshProjectEnrollmentRendererCommandSchema,
	DesktopSshProjectEnrollmentRequestIdSchema,
	type DesktopSshProjectConnectionResult,
	type DesktopSshProjectConnectionStatus,
	type DesktopSshProjectEnrollmentInput,
	type DesktopSshProjectEnrollmentProgressStage,
	type DesktopSshProjectEnrollmentRequestId,
	type DesktopSshProjectEnrollmentResult,
	type DesktopSshProjectInteractionResult,
} from '@orchbee/desktop-client-protocol';

import { createBrowserRandomIdentifier } from '../../../browser-random-identifier';

export interface DesktopSshProjectInteractionPrompt {
	readonly generation: number;
	readonly handle: string;
	readonly kind: 'confirm' | 'none' | 'unknown';
	readonly prompt: string;
}

export interface DesktopSshProjectOperationObserver {
	interact(prompt: DesktopSshProjectInteractionPrompt): Promise<DesktopSshProjectInteractionResult>;
	progress?(stage: DesktopSshProjectEnrollmentProgressStage): void;
}

interface EnrollmentPendingRequest {
	readonly abortListener: () => void;
	readonly kind: 'enrollment';
	readonly observer: DesktopSshProjectOperationObserver;
	readonly requestId: DesktopSshProjectEnrollmentRequestId;
	resolve(result: DesktopSshProjectEnrollmentResult): void;
	readonly signal: AbortSignal;
}

interface ConnectionPendingRequest {
	readonly abortListener: () => void;
	readonly kind: 'connection';
	readonly observer: Pick<DesktopSshProjectOperationObserver, 'interact'>;
	readonly requestId: DesktopSshProjectEnrollmentRequestId;
	resolve(result: DesktopSshProjectConnectionResult): void;
	readonly signal: AbortSignal;
}

type PendingRequest = ConnectionPendingRequest | EnrollmentPendingRequest;
interface PendingInteraction {
	readonly generation: number;
	readonly handle: string;
	readonly request: PendingRequest;
}
type StatusListener = () => void;

export class DesktopSshProjectEnrollmentClient {
	readonly #createRequestId: () => DesktopSshProjectEnrollmentRequestId;
	#disposed = false;
	readonly #listeners = new Set<StatusListener>();
	#pending: PendingRequest | undefined;
	#pendingInteraction: PendingInteraction | undefined;
	readonly #port: MessagePort;
	#sequence = 0;
	#statuses: readonly DesktopSshProjectConnectionStatus[] = [];

	constructor(port: MessagePort, createRequestId: () => string = defaultRequestId) {
		this.#port = port;
		this.#createRequestId = () =>
			DesktopSshProjectEnrollmentRequestIdSchema.parse(`${createRequestId()}:${++this.#sequence}`);
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
		this.#requestStatuses();
	}

	enroll(
		input: DesktopSshProjectEnrollmentInput,
		observer: DesktopSshProjectOperationObserver,
		signal: AbortSignal,
	): Promise<DesktopSshProjectEnrollmentResult> {
		if (signal.aborted) return Promise.resolve({ kind: 'cancelled' });
		if (this.#disposed) {
			return Promise.resolve({ kind: 'indeterminate', reason: 'control-unavailable' });
		}
		if (this.#pending !== undefined) {
			return Promise.resolve({ kind: 'failed', reason: 'busy' });
		}
		const requestId = this.#createRequestId();
		const deferred = createDeferred<DesktopSshProjectEnrollmentResult>();
		const abortListener = (): void => this.#cancel(requestId);
		this.#pending = {
			abortListener,
			kind: 'enrollment',
			observer,
			requestId,
			resolve: deferred.resolve,
			signal,
		};
		signal.addEventListener('abort', abortListener, { once: true });
		this.#post({ input, requestId, type: 'desktop-ssh-project-enrollment-requested' });
		return deferred.promise;
	}

	connect(
		projectId: string,
		observer: Pick<DesktopSshProjectOperationObserver, 'interact'>,
		signal: AbortSignal,
	): Promise<DesktopSshProjectConnectionResult> {
		return this.#connectionOperation(projectId, observer, signal, 'connect');
	}

	disconnect(projectId: string, signal: AbortSignal): Promise<DesktopSshProjectConnectionResult> {
		return this.#connectionOperation(
			projectId,
			{ interact: async () => ({ kind: 'cancelled' }) },
			signal,
			'disconnect',
		);
	}

	getStatuses(): readonly DesktopSshProjectConnectionStatus[] {
		return this.#statuses;
	}

	subscribeStatuses(listener: StatusListener): () => void {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	dispose(): void {
		this.#retire(true);
	}

	#connectionOperation(
		projectId: string,
		observer: Pick<DesktopSshProjectOperationObserver, 'interact'>,
		signal: AbortSignal,
		kind: 'connect' | 'disconnect',
	): Promise<DesktopSshProjectConnectionResult> {
		if (signal.aborted) return Promise.resolve({ kind: 'cancelled' });
		if (this.#disposed) {
			return Promise.resolve({ kind: 'indeterminate', reason: 'control-unavailable' });
		}
		if (this.#pending !== undefined) {
			return Promise.resolve({ kind: 'failed', reason: 'busy' });
		}
		const requestId = this.#createRequestId();
		const deferred = createDeferred<DesktopSshProjectConnectionResult>();
		const abortListener = (): void => this.#cancel(requestId);
		this.#pending = {
			abortListener,
			kind: 'connection',
			observer,
			requestId,
			resolve: deferred.resolve,
			signal,
		};
		signal.addEventListener('abort', abortListener, { once: true });
		this.#post({
			projectId,
			requestId,
			type:
				kind === 'connect'
					? 'desktop-ssh-project-connect-requested'
					: 'desktop-ssh-project-disconnect-requested',
		});
		return deferred.promise;
	}

	readonly #handleMessage = (event: MessageEvent<unknown>): void => {
		if (this.#disposed || event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = DesktopSshProjectEnrollmentMainEventSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const message = parsed.data;
		if (message.type === 'desktop-ssh-project-status-published') {
			this.#statuses = message.statuses;
			for (const listener of this.#listeners) listener();
			return;
		}
		const pending = this.#pending;
		if (pending === undefined || pending.requestId !== message.requestId) {
			this.#retire(true);
			return;
		}
		switch (message.type) {
			case 'desktop-ssh-project-enrollment-progressed':
				if (pending.kind !== 'enrollment') this.#retire(true);
				else pending.observer.progress?.(message.stage);
				return;
			case 'desktop-ssh-project-interaction-requested':
				if (this.#pendingInteraction !== undefined) {
					this.#retire(true);
					return;
				}
				const interaction = {
					generation: message.generation,
					handle: message.handle,
					request: pending,
				};
				this.#pendingInteraction = interaction;
				void this.#interact(pending, message, interaction);
				return;
			case 'desktop-ssh-project-enrollment-settled':
				if (pending.kind !== 'enrollment' || this.#pendingInteraction !== undefined) {
					this.#retire(true);
				} else this.#settleEnrollment(pending, message.result);
				return;
			case 'desktop-ssh-project-connection-settled':
				if (pending.kind !== 'connection' || this.#pendingInteraction !== undefined) {
					this.#retire(true);
				} else this.#settleConnection(pending, message.result);
				return;
		}
	};

	readonly #handleMessageError = (): void => this.#retire(true);

	async #interact(
		pending: PendingRequest,
		message: Extract<
			ReturnType<typeof DesktopSshProjectEnrollmentMainEventSchema.parse>,
			{ readonly type: 'desktop-ssh-project-interaction-requested' }
		>,
		interaction: PendingInteraction,
	): Promise<void> {
		let result: DesktopSshProjectInteractionResult;
		try {
			result = await pending.observer.interact({
				generation: message.generation,
				handle: message.handle,
				kind: message.kind,
				prompt: message.prompt,
			});
		} catch {
			result = { kind: 'cancelled' };
		}
		if (this.#disposed || this.#pending !== pending || this.#pendingInteraction !== interaction) {
			return;
		}
		this.#pendingInteraction = undefined;
		this.#post({
			generation: message.generation,
			handle: message.handle,
			requestId: pending.requestId,
			result,
			type: 'desktop-ssh-project-interaction-resolved',
		});
	}

	#settleEnrollment(
		pending: EnrollmentPendingRequest,
		result: DesktopSshProjectEnrollmentResult,
	): void {
		if (this.#pending !== pending) return;
		this.#pending = undefined;
		pending.signal.removeEventListener('abort', pending.abortListener);
		pending.resolve(result);
	}

	#settleConnection(
		pending: ConnectionPendingRequest,
		result: DesktopSshProjectConnectionResult,
	): void {
		if (this.#pending !== pending) return;
		this.#pending = undefined;
		pending.signal.removeEventListener('abort', pending.abortListener);
		pending.resolve(result);
	}

	#cancel(requestId: DesktopSshProjectEnrollmentRequestId): void {
		if (this.#pending?.requestId !== requestId || this.#disposed) return;
		this.#post({ requestId, type: 'desktop-ssh-project-enrollment-cancel-requested' });
	}

	#requestStatuses(): void {
		if (this.#disposed) return;
		this.#post({
			requestId: this.#createRequestId(),
			type: 'desktop-ssh-project-status-requested',
		});
	}

	#post(
		command: Parameters<typeof DesktopSshProjectEnrollmentRendererCommandSchema.parse>[0],
	): void {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(DesktopSshProjectEnrollmentRendererCommandSchema.parse(command));
		} catch {
			this.#retire(true);
		}
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		const pending = this.#pending;
		this.#pending = undefined;
		this.#pendingInteraction = undefined;
		if (pending !== undefined) {
			pending.signal.removeEventListener('abort', pending.abortListener);
			if (pending.kind === 'enrollment') {
				pending.resolve({ kind: 'indeterminate', reason: 'control-unavailable' });
			} else {
				pending.resolve({ kind: 'indeterminate', reason: 'control-unavailable' });
			}
		}
		this.#listeners.clear();
		if (closePort) this.#port.close();
	}
}

function defaultRequestId(): string {
	return `ssh-project:${createBrowserRandomIdentifier()}`;
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
