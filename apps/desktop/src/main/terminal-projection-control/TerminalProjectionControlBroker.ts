import type { MessageEvent, MessagePortMain } from 'electron';
import {
	TerminalProjectionControlCommandSchema,
	TerminalProjectionControlEventSchema,
	type TerminalProjectionControlEvent,
	type TerminalProjectionRequestId,
} from '@orchbee/workbench-client-protocol';
import {
	TerminalPresentationFailureSchema,
	type TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';

import type {
	DesktopTerminalProjectionLease,
	DesktopTerminalProjectionService,
} from './DesktopTerminalProjectionService';

interface ProjectionRequest {
	disposalPromise: Promise<void> | undefined;
	lease: DesktopTerminalProjectionLease | null;
	readonly presentationId: TerminalPresentationId;
	releaseRequested: boolean;
}

export class TerminalProjectionControlBroker {
	readonly #openTasks = new Set<Promise<void>>();
	readonly #port: MessagePortMain;
	readonly #requests = new Map<TerminalProjectionRequestId, ProjectionRequest>();
	readonly #service: Pick<DesktopTerminalProjectionService, 'open'>;
	#disposePromise: Promise<void> | undefined;
	#disposed = false;
	#started = false;

	constructor(port: MessagePortMain, service: Pick<DesktopTerminalProjectionService, 'open'>) {
		this.#port = port;
		this.#service = service;
	}

	start(): void {
		if (this.#disposed || this.#started) return;
		this.#started = true;
		this.#port.on('message', this.#handleRendererMessage);
		this.#port.on('close', this.#handlePortClose);
		this.#port.start();
	}

	dispose(): Promise<void> {
		return this.#startRetire(true);
	}

	readonly #handlePortClose = (): void => {
		void this.#startRetire(false).catch(() => undefined);
	};

	#startRetire(closePort: boolean): Promise<void> {
		if (this.#disposePromise !== undefined) return this.#disposePromise;
		const operation = this.#retire(closePort);
		let tracked: Promise<void>;
		tracked = operation.catch((error: unknown) => {
			if (this.#disposePromise === tracked) this.#disposePromise = undefined;
			throw error;
		});
		this.#disposePromise = tracked;
		return tracked;
	}

	readonly #handleRendererMessage = (event: MessageEvent): void => {
		if (this.#disposed) return;
		const parsed = TerminalProjectionControlCommandSchema.safeParse(event.data);
		if (!parsed.success) {
			void this.#startRetire(true).catch(() => undefined);
			return;
		}
		const command = parsed.data;
		const existing = this.#requests.get(command.requestId);
		if (command.type === 'terminal-projection-open-requested') {
			if (existing !== undefined) {
				if (existing.presentationId !== command.presentationId) {
					void this.#startRetire(true).catch(() => undefined);
				}
				return;
			}
			const request: ProjectionRequest = {
				disposalPromise: undefined,
				lease: null,
				presentationId: command.presentationId,
				releaseRequested: false,
			};
			this.#requests.set(command.requestId, request);
			const task = this.#open(command.requestId, command.presentationId, request);
			this.#openTasks.add(task);
			void task.then(
				() => this.#openTasks.delete(task),
				() => this.#openTasks.delete(task),
			);
			return;
		}
		if (existing === undefined || existing.presentationId !== command.presentationId) {
			void this.#startRetire(true).catch(() => undefined);
			return;
		}
		existing.releaseRequested = true;
		void this.#disposeRequest(command.requestId, existing).catch(() => undefined);
	};

	async #disposeRequest(
		requestId: TerminalProjectionRequestId,
		request: ProjectionRequest,
	): Promise<void> {
		if (request.disposalPromise !== undefined) return request.disposalPromise;
		const lease = request.lease;
		if (lease === null) return;
		const operation = lease
			.dispose()
			.then(() => {
				request.lease = null;
				if (this.#requests.get(requestId) === request) this.#requests.delete(requestId);
				this.#post({
					presentationId: request.presentationId,
					requestId,
					type: 'terminal-projection-released',
				});
			})
			.catch((error: unknown) => {
				request.disposalPromise = undefined;
				this.#post({
					failure: { code: 'terminal-projection-cleanup-failed', retryable: true },
					presentationId: request.presentationId,
					requestId,
					type: 'terminal-projection-failed',
				});
				throw error;
			});
		request.disposalPromise = operation;
		return operation;
	}

	async #open(
		requestId: TerminalProjectionRequestId,
		presentationId: Parameters<DesktopTerminalProjectionService['open']>[0],
		request: ProjectionRequest,
	): Promise<void> {
		try {
			const lease = await this.#service.open(presentationId);
			request.lease = lease;
			if (this.#disposed || request.releaseRequested) {
				try {
					await this.#disposeRequest(requestId, request);
				} catch {
					return;
				}
				return;
			}
			const event = TerminalProjectionControlEventSchema.parse({
				bootstrap: lease.bootstrap,
				presentationId,
				requestId,
				type: 'terminal-projection-opened',
			});
			try {
				this.#port.postMessage(event, [lease.rendererPort]);
			} catch {
				request.releaseRequested = true;
				try {
					await this.#disposeRequest(requestId, request);
				} catch {
					// The request and lease remain owned by this broker and can be
					// retried by a later release or broker retirement.
				}
				void this.#startRetire(true).catch(() => undefined);
			}
		} catch (error) {
			if (this.#disposed) throw error;
			if (request.releaseRequested) {
				if (request.lease === null && this.#requests.get(requestId) === request) {
					this.#requests.delete(requestId);
					this.#post({ presentationId, requestId, type: 'terminal-projection-released' });
				}
				return;
			}
			const failure = TerminalPresentationFailureSchema.safeParse(
				error instanceof Error ? error.cause : undefined,
			);
			this.#post({
				failure: failure.success
					? failure.data
					: { code: 'terminal-projection-unavailable', retryable: true },
				presentationId,
				requestId,
				type: 'terminal-projection-failed',
			});
			this.#requests.delete(requestId);
		}
	}

	#post(event: TerminalProjectionControlEvent): void {
		if (this.#disposed) return;
		const parsed = TerminalProjectionControlEventSchema.safeParse(event);
		if (!parsed.success) {
			void this.#startRetire(true).catch(() => undefined);
			return;
		}
		try {
			this.#port.postMessage(parsed.data);
		} catch {
			void this.#startRetire(true).catch(() => undefined);
		}
	}

	async #retire(closePort: boolean): Promise<void> {
		this.#disposed = true;
		if (this.#started) {
			this.#port.off('message', this.#handleRendererMessage);
			this.#port.off('close', this.#handlePortClose);
		}
		if (closePort) this.#port.close();
		let failed = false;
		let firstFailure: unknown;
		for (const [, request] of this.#requests) request.releaseRequested = true;
		await Promise.all(
			[...this.#openTasks].map(async (task) => {
				try {
					await task;
				} catch (error: unknown) {
					failed = true;
					firstFailure ??= error;
				}
			}),
		);
		for (const [requestId, request] of this.#requests) {
			try {
				await this.#disposeRequest(requestId, request);
			} catch (error: unknown) {
				failed = true;
				firstFailure ??= error;
			}
		}
		if (failed) throw firstFailure;
		this.#requests.clear();
	}
}
