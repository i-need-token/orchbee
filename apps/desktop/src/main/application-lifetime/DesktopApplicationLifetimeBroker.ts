import type { WorkbenchSharedDocumentRevision } from '@orchbee/workbench/document';
import type { MessageEvent, MessagePortMain } from 'electron';

import {
	DesktopApplicationLifetimeRendererCommandSchema,
	DesktopApplicationLifetimeMainEventSchema,
	type DesktopApplicationExitFailure,
	type DesktopApplicationExitReason,
	type DesktopApplicationExitRequestId,
	type DesktopApplicationLifetimeMainEvent,
} from '../../shared';

export type DesktopApplicationLifetimeDrainResult =
	| { readonly closedUnreferencedCount: number; readonly kind: 'completed' }
	| { readonly failures: DesktopApplicationExitFailure[]; readonly kind: 'failed' };

export type DesktopApplicationLifetimeExitResult =
	| {
			readonly forced: boolean;
			readonly kind: 'ready';
			readonly mode: 'preserve' | 'stop';
	  }
	| { readonly kind: 'cancelled' }
	| { readonly kind: 'unavailable' };

export interface DesktopApplicationLifetimeBrokerDependencies {
	readonly drainForStop: (
		documentRevision: WorkbenchSharedDocumentRevision,
	) => Promise<DesktopApplicationLifetimeDrainResult>;
	readonly prepareMode?: ((mode: 'preserve' | 'stop') => boolean) | undefined;
}

interface ActiveExitRequest {
	drainPending: boolean;
	failed: boolean;
	readonly promise: Promise<DesktopApplicationLifetimeExitResult>;
	readonly reason: DesktopApplicationExitReason;
	readonly requestId: DesktopApplicationExitRequestId;
	resolve(result: DesktopApplicationLifetimeExitResult): void;
	mode: 'preserve' | 'stop' | null;
	stopRevision: WorkbenchSharedDocumentRevision | null;
}

export class DesktopApplicationLifetimeBroker {
	#activeRequest: ActiveExitRequest | undefined;
	readonly #dependencies: DesktopApplicationLifetimeBrokerDependencies;
	#disposed = false;
	readonly #port: MessagePortMain;
	#started = false;

	constructor(port: MessagePortMain, dependencies: DesktopApplicationLifetimeBrokerDependencies) {
		this.#port = port;
		this.#dependencies = dependencies;
	}

	start(): void {
		if (this.#disposed || this.#started) return;
		this.#started = true;
		this.#port.on('message', this.#handleMessage);
		this.#port.on('close', this.#handlePortClose);
		this.#port.start();
	}

	requestExit(
		requestId: DesktopApplicationExitRequestId,
		reason: DesktopApplicationExitReason,
	): Promise<DesktopApplicationLifetimeExitResult> {
		if (this.#disposed || !this.#started) return Promise.resolve({ kind: 'unavailable' });
		if (this.#activeRequest !== undefined) return this.#activeRequest.promise;
		let resolveRequest: (result: DesktopApplicationLifetimeExitResult) => void = () => undefined;
		const promise = new Promise<DesktopApplicationLifetimeExitResult>((resolve) => {
			resolveRequest = resolve;
		});
		this.#activeRequest = {
			drainPending: false,
			failed: false,
			promise,
			reason,
			requestId,
			resolve: resolveRequest,
			mode: null,
			stopRevision: null,
		};
		this.#post({ reason, requestId, type: 'desktop-application-exit-requested' });
		return promise;
	}

	dispose(): void {
		this.#retire(true);
	}

	readonly #handlePortClose = (): void => {
		this.#retire(false);
	};

	readonly #handleMessage = (event: MessageEvent): void => {
		if (event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = DesktopApplicationLifetimeRendererCommandSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const command = parsed.data;
		const request = this.#activeRequest;
		if (request === undefined || command.requestId !== request.requestId) return;
		switch (command.type) {
			case 'desktop-application-exit-choice': {
				if (command.choice === 'cancel' || request.mode !== null) return;
				if (this.#dependencies.prepareMode?.(command.choice) === false) {
					this.#post({
						requestId: request.requestId,
						result: {
							failures: [
								{
									code: 'shared-document-control-unavailable',
									count: 1,
									retryable: false,
								},
							],
							kind: 'failed',
						},
						type: 'desktop-application-exit-main-completed',
					});
					request.failed = true;
					return;
				}
				request.mode = command.choice;
				this.#post({
					mode: command.choice,
					requestId: request.requestId,
					type: 'desktop-application-exit-mode-accepted',
				});
				return;
			}
			case 'desktop-application-exit-progress':
				if (command.progress.phase === 'failed') request.failed = true;
				return;
			case 'desktop-application-exit-cancelled':
				if (
					request.mode === null &&
					!request.drainPending &&
					!request.failed &&
					request.stopRevision === null
				) {
					this.#finish({ kind: 'cancelled' });
				}
				return;
			case 'desktop-application-exit-renderer-prepared':
				if (request.mode !== command.mode) return;
				request.failed = false;
				if (command.mode === 'preserve') {
					this.#post({
						requestId: request.requestId,
						result: { closedUnreferencedCount: 0, kind: 'completed' },
						type: 'desktop-application-exit-main-completed',
					});
					this.#finish({ forced: false, kind: 'ready', mode: 'preserve' });
					return;
				}
				if (request.stopRevision !== null || request.drainPending) return;
				request.stopRevision = command.documentRevision;
				void this.#drain(request);
				return;
			case 'desktop-application-exit-main-retry-requested':
				if (!request.failed || request.drainPending || request.stopRevision === null) return;
				void this.#drain(request);
				return;
			case 'desktop-application-exit-quit-anyway-requested':
				if (!request.failed || request.drainPending) return;
				this.#finish({ forced: true, kind: 'ready', mode: 'stop' });
		}
	};

	async #drain(request: ActiveExitRequest): Promise<void> {
		const revision = request.stopRevision;
		if (revision === null || request.drainPending) return;
		request.drainPending = true;
		request.failed = false;
		let result: DesktopApplicationLifetimeDrainResult;
		try {
			result = await this.#dependencies.drainForStop(revision);
		} catch {
			result = {
				failures: [{ code: 'host-inventory-unavailable', count: 1, retryable: true }],
				kind: 'failed',
			};
		}
		if (this.#activeRequest !== request || this.#disposed) return;
		request.drainPending = false;
		this.#post({
			requestId: request.requestId,
			result:
				result.kind === 'completed' ? result : { failures: [...result.failures], kind: 'failed' },
			type: 'desktop-application-exit-main-completed',
		});
		if (result.kind === 'completed') {
			this.#finish({ forced: false, kind: 'ready', mode: 'stop' });
			return;
		}
		request.failed = true;
	}

	#finish(result: DesktopApplicationLifetimeExitResult): void {
		const request = this.#activeRequest;
		if (request === undefined) return;
		this.#activeRequest = undefined;
		request.resolve(result);
	}

	#post(event: DesktopApplicationLifetimeMainEvent): void {
		if (this.#disposed) return;
		const parsed = DesktopApplicationLifetimeMainEventSchema.safeParse(event);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		try {
			this.#port.postMessage(parsed.data);
		} catch {
			this.#retire(true);
		}
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.off('message', this.#handleMessage);
		this.#port.off('close', this.#handlePortClose);
		if (closePort) this.#port.close();
		this.#finish({ kind: 'unavailable' });
	}
}
