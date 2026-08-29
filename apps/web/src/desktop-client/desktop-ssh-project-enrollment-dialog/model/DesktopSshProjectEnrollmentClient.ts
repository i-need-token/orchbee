import {
	DesktopSshProjectEnrollmentMainEventSchema,
	DesktopSshProjectEnrollmentRendererCommandSchema,
	DesktopSshProjectEnrollmentRequestIdSchema,
	type DesktopSshEnvironmentAddResult,
	type DesktopSshEnvironmentCatalog,
	type DesktopSshEnvironmentConnectionResult,
	type DesktopSshEnvironmentId,
	type DesktopSshProjectConnectionResult,
	type DesktopSshProjectConnectionStatus,
	type DesktopSshProjectDirectorySelectionResult,
	type DesktopSshProjectDirectorySnapshot,
	type DesktopSshProjectEnrollmentInput,
	type DesktopSshProjectEnrollmentProgressStage,
	type DesktopSshProjectEnrollmentRequestId,
	type DesktopSshProjectEnrollmentResult,
	type DesktopSshProjectFolderAttachmentResult,
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
	selectDirectory(
		snapshot: DesktopSshProjectDirectorySnapshot,
	): Promise<DesktopSshProjectDirectorySelectionResult>;
}

interface EnrollmentPendingRequest {
	readonly abortListener: () => void;
	readonly kind: 'enrollment';
	readonly observer: DesktopSshProjectOperationObserver;
	readonly requestId: DesktopSshProjectEnrollmentRequestId;
	resolve(result: DesktopSshProjectEnrollmentResult): void;
	readonly signal: AbortSignal;
}

interface AttachmentPendingRequest {
	readonly abortListener: () => void;
	readonly kind: 'attachment';
	readonly observer: DesktopSshProjectOperationObserver;
	readonly requestId: DesktopSshProjectEnrollmentRequestId;
	resolve(result: DesktopSshProjectFolderAttachmentResult): void;
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

interface EnvironmentAddPendingRequest {
	readonly abortListener: () => void;
	readonly kind: 'environment-add';
	readonly requestId: DesktopSshProjectEnrollmentRequestId;
	resolve(result: DesktopSshEnvironmentAddResult): void;
	readonly signal: AbortSignal;
}

interface EnvironmentConnectionPendingRequest {
	readonly abortListener: () => void;
	readonly kind: 'environment-connection';
	readonly observer: Pick<DesktopSshProjectOperationObserver, 'interact'>;
	readonly requestId: DesktopSshProjectEnrollmentRequestId;
	resolve(result: DesktopSshEnvironmentConnectionResult): void;
	readonly signal: AbortSignal;
}

type PendingRequest =
	| AttachmentPendingRequest
	| ConnectionPendingRequest
	| EnvironmentAddPendingRequest
	| EnvironmentConnectionPendingRequest
	| EnrollmentPendingRequest;
interface PendingPromptInteraction {
	readonly generation: number;
	readonly handle: string;
	readonly kind: 'prompt';
	readonly request: Exclude<PendingRequest, EnvironmentAddPendingRequest>;
}
interface PendingDirectoryInteraction {
	readonly kind: 'directory';
	readonly request: AttachmentPendingRequest | EnrollmentPendingRequest;
	readonly revision: number;
}
type PendingInteraction = PendingDirectoryInteraction | PendingPromptInteraction;
type StatusListener = () => void;

const EMPTY_ENVIRONMENT_CATALOG: DesktopSshEnvironmentCatalog = {
	discoveryIssue: null,
	managed: [],
	registryIssue: null,
	suggestions: [],
};

export class DesktopSshProjectEnrollmentClient {
	readonly #createRequestId: () => DesktopSshProjectEnrollmentRequestId;
	#disposed = false;
	readonly #listeners = new Set<StatusListener>();
	readonly #environmentCatalogListeners = new Set<StatusListener>();
	#environmentCatalog: DesktopSshEnvironmentCatalog = EMPTY_ENVIRONMENT_CATALOG;
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
		this.refreshEnvironmentCatalog(false);
	}

	addEnvironment(
		input: DesktopSshProjectEnrollmentInput,
		signal: AbortSignal,
	): Promise<DesktopSshEnvironmentAddResult> {
		if (signal.aborted) return Promise.resolve({ kind: 'cancelled' });
		if (this.#disposed) {
			return Promise.resolve({ kind: 'indeterminate', reason: 'control-unavailable' });
		}
		if (this.#pending !== undefined) {
			return Promise.resolve({ kind: 'failed', reason: 'busy' });
		}
		const requestId = this.#createRequestId();
		const deferred = createDeferred<DesktopSshEnvironmentAddResult>();
		const abortListener = (): void => this.#cancel(requestId);
		this.#pending = {
			abortListener,
			kind: 'environment-add',
			requestId,
			resolve: deferred.resolve,
			signal,
		};
		signal.addEventListener('abort', abortListener, { once: true });
		this.#post({ input, requestId, type: 'desktop-ssh-environment-add-requested' });
		return deferred.promise;
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

	attachFolder(
		projectId: string,
		input: DesktopSshProjectEnrollmentInput,
		observer: DesktopSshProjectOperationObserver,
		signal: AbortSignal,
	): Promise<DesktopSshProjectFolderAttachmentResult> {
		if (signal.aborted) return Promise.resolve({ kind: 'cancelled' });
		if (this.#disposed) {
			return Promise.resolve({ kind: 'indeterminate', reason: 'control-unavailable' });
		}
		if (this.#pending !== undefined) {
			return Promise.resolve({ kind: 'failed', reason: 'busy' });
		}
		const requestId = this.#createRequestId();
		const deferred = createDeferred<DesktopSshProjectFolderAttachmentResult>();
		const abortListener = (): void => this.#cancel(requestId);
		this.#pending = {
			abortListener,
			kind: 'attachment',
			observer,
			requestId,
			resolve: deferred.resolve,
			signal,
		};
		signal.addEventListener('abort', abortListener, { once: true });
		this.#post({
			input,
			projectId,
			requestId,
			type: 'desktop-ssh-project-folder-attachment-requested',
		});
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

	connectEnvironment(
		environmentId: DesktopSshEnvironmentId,
		observer: Pick<DesktopSshProjectOperationObserver, 'interact'>,
		signal: AbortSignal,
	): Promise<DesktopSshEnvironmentConnectionResult> {
		return this.#environmentConnectionOperation(environmentId, observer, signal, 'connect');
	}

	disconnectEnvironment(
		environmentId: DesktopSshEnvironmentId,
		signal: AbortSignal,
	): Promise<DesktopSshEnvironmentConnectionResult> {
		return this.#environmentConnectionOperation(
			environmentId,
			{ interact: async () => ({ kind: 'cancelled' }) },
			signal,
			'disconnect',
		);
	}

	getEnvironmentCatalog(): DesktopSshEnvironmentCatalog {
		return this.#environmentCatalog;
	}

	subscribeEnvironmentCatalog(listener: StatusListener): () => void {
		this.#environmentCatalogListeners.add(listener);
		return () => this.#environmentCatalogListeners.delete(listener);
	}

	refreshEnvironmentCatalog(refreshDiscovery = true): void {
		if (this.#disposed) return;
		this.#post({
			refreshDiscovery,
			requestId: this.#createRequestId(),
			type: 'desktop-ssh-environment-catalog-requested',
		});
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

	#environmentConnectionOperation(
		environmentId: DesktopSshEnvironmentId,
		observer: Pick<DesktopSshProjectOperationObserver, 'interact'>,
		signal: AbortSignal,
		kind: 'connect' | 'disconnect',
	): Promise<DesktopSshEnvironmentConnectionResult> {
		if (signal.aborted) return Promise.resolve({ kind: 'cancelled' });
		if (this.#disposed) {
			return Promise.resolve({ kind: 'indeterminate', reason: 'control-unavailable' });
		}
		if (this.#pending !== undefined) {
			return Promise.resolve({ kind: 'failed', reason: 'busy' });
		}
		const requestId = this.#createRequestId();
		const deferred = createDeferred<DesktopSshEnvironmentConnectionResult>();
		const abortListener = (): void => this.#cancel(requestId);
		this.#pending = {
			abortListener,
			kind: 'environment-connection',
			observer,
			requestId,
			resolve: deferred.resolve,
			signal,
		};
		signal.addEventListener('abort', abortListener, { once: true });
		this.#post({
			environmentId,
			requestId,
			type:
				kind === 'connect'
					? 'desktop-ssh-environment-connect-requested'
					: 'desktop-ssh-environment-disconnect-requested',
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
		if (message.type === 'desktop-ssh-environment-catalog-published') {
			this.#environmentCatalog = message.catalog;
			for (const listener of this.#environmentCatalogListeners) listener();
			return;
		}
		const pending = this.#pending;
		if (pending === undefined || pending.requestId !== message.requestId) {
			this.#retire(true);
			return;
		}
		switch (message.type) {
			case 'desktop-ssh-project-folder-attachment-progressed':
				if (pending.kind !== 'attachment') this.#retire(true);
				else pending.observer.progress?.(message.stage);
				return;
			case 'desktop-ssh-project-enrollment-progressed':
				if (pending.kind !== 'enrollment') this.#retire(true);
				else pending.observer.progress?.(message.stage);
				return;
			case 'desktop-ssh-project-interaction-requested':
				if (pending.kind === 'environment-add' || this.#pendingInteraction !== undefined) {
					this.#retire(true);
					return;
				}
				const interaction = {
					generation: message.generation,
					handle: message.handle,
					kind: 'prompt' as const,
					request: pending,
				};
				this.#pendingInteraction = interaction;
				void this.#interact(pending, message, interaction);
				return;
			case 'desktop-ssh-project-directory-selection-requested':
				if (
					(pending.kind !== 'attachment' && pending.kind !== 'enrollment') ||
					this.#pendingInteraction !== undefined
				) {
					this.#retire(true);
					return;
				}
				const directoryInteraction: PendingDirectoryInteraction = {
					kind: 'directory',
					request: pending,
					revision: message.revision,
				};
				this.#pendingInteraction = directoryInteraction;
				void this.#selectDirectory(pending, message, directoryInteraction);
				return;
			case 'desktop-ssh-project-enrollment-settled':
				if (pending.kind !== 'enrollment' || this.#pendingInteraction !== undefined) {
					this.#retire(true);
				} else this.#settleEnrollment(pending, message.result);
				return;
			case 'desktop-ssh-project-folder-attachment-settled':
				if (pending.kind !== 'attachment' || this.#pendingInteraction !== undefined) {
					this.#retire(true);
				} else this.#settleFolderAttachment(pending, message.result);
				return;
			case 'desktop-ssh-project-connection-settled':
				if (pending.kind !== 'connection' || this.#pendingInteraction !== undefined) {
					this.#retire(true);
				} else this.#settleConnection(pending, message.result);
				return;
			case 'desktop-ssh-environment-add-settled':
				if (pending.kind !== 'environment-add' || this.#pendingInteraction !== undefined) {
					this.#retire(true);
				} else this.#settleEnvironmentAdd(pending, message.result);
				return;
			case 'desktop-ssh-environment-connection-settled':
				if (pending.kind !== 'environment-connection' || this.#pendingInteraction !== undefined) {
					this.#retire(true);
				} else this.#settleEnvironmentConnection(pending, message.result);
				return;
		}
	};

	readonly #handleMessageError = (): void => this.#retire(true);

	async #interact(
		pending: Exclude<PendingRequest, EnvironmentAddPendingRequest>,
		message: Extract<
			ReturnType<typeof DesktopSshProjectEnrollmentMainEventSchema.parse>,
			{ readonly type: 'desktop-ssh-project-interaction-requested' }
		>,
		interaction: PendingPromptInteraction,
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

	async #selectDirectory(
		pending: AttachmentPendingRequest | EnrollmentPendingRequest,
		message: Extract<
			ReturnType<typeof DesktopSshProjectEnrollmentMainEventSchema.parse>,
			{ readonly type: 'desktop-ssh-project-directory-selection-requested' }
		>,
		interaction: PendingDirectoryInteraction,
	): Promise<void> {
		let result: DesktopSshProjectDirectorySelectionResult;
		try {
			result = await pending.observer.selectDirectory(message.snapshot);
		} catch {
			result = { kind: 'cancelled' };
		}
		if (this.#disposed || this.#pending !== pending || this.#pendingInteraction !== interaction) {
			return;
		}
		this.#pendingInteraction = undefined;
		this.#post({
			requestId: pending.requestId,
			result,
			revision: message.revision,
			type: 'desktop-ssh-project-directory-selection-resolved',
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

	#settleFolderAttachment(
		pending: AttachmentPendingRequest,
		result: DesktopSshProjectFolderAttachmentResult,
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

	#settleEnvironmentAdd(
		pending: EnvironmentAddPendingRequest,
		result: DesktopSshEnvironmentAddResult,
	): void {
		if (this.#pending !== pending) return;
		this.#pending = undefined;
		pending.signal.removeEventListener('abort', pending.abortListener);
		pending.resolve(result);
	}

	#settleEnvironmentConnection(
		pending: EnvironmentConnectionPendingRequest,
		result: DesktopSshEnvironmentConnectionResult,
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
			if (pending.kind === 'connection') {
				pending.resolve({ kind: 'indeterminate', reason: 'control-unavailable' });
			} else {
				pending.resolve({ kind: 'indeterminate', reason: 'control-unavailable' });
			}
		}
		this.#listeners.clear();
		this.#environmentCatalogListeners.clear();
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
