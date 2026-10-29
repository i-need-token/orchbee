import type { MessageEvent, MessagePortMain } from 'electron';
import {
	TerminalProjectionControlCommandSchema,
	TerminalProjectionControlEventSchema,
	type TerminalProjectionControlEvent,
	type TerminalProjectionRequestId,
	TerminalPresentationFailureSchema,
	type TerminalPresentationId,
} from '@orchbee/workbench-client-protocol';
import type { TerminalGeometry } from '@orchbee/terminal-protocol';

import type { DesktopTerminalProjectionLease } from './DesktopTerminalProjectionService';

interface ProjectionRequest {
	disposalPromise: Promise<void> | undefined;
	lease: DesktopTerminalProjectionLease | null;
	readonly geometry: TerminalGeometry;
	readonly presentationId: TerminalPresentationId;
	releaseRequested: boolean;
}

export interface DesktopTerminalProjectionControlService {
	isPublicationTargetCurrent?(
		presentationId: TerminalPresentationId,
		lease: DesktopTerminalProjectionLease,
	): boolean;
	open(
		presentationId: TerminalPresentationId,
		geometry: TerminalGeometry,
	): Promise<DesktopTerminalProjectionLease>;
}

export class TerminalProjectionControlBroker {
	readonly #openTasks = new Set<Promise<void>>();
	readonly #port: MessagePortMain;
	readonly #requests = new Map<TerminalProjectionRequestId, ProjectionRequest>();
	readonly #service: DesktopTerminalProjectionControlService;
	#disposePromise: Promise<void> | undefined;
	#disposed = false;
	#started = false;

	constructor(port: MessagePortMain, service: DesktopTerminalProjectionControlService) {
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
				if (
					existing.presentationId !== command.presentationId ||
					!geometriesEqual(existing.geometry, command.geometry)
				) {
					void this.#startRetire(true).catch(() => undefined);
				}
				return;
			}
			const request: ProjectionRequest = {
				disposalPromise: undefined,
				geometry: command.geometry,
				lease: null,
				presentationId: command.presentationId,
				releaseRequested: false,
			};
			this.#requests.set(command.requestId, request);
			const task = this.#open(command.requestId, command.presentationId, command.geometry, request);
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
		presentationId: TerminalPresentationId,
		geometry: TerminalGeometry,
		request: ProjectionRequest,
	): Promise<void> {
		try {
			const lease = await this.#service.open(presentationId, geometry);
			request.lease = lease;
			if (this.#disposed || request.releaseRequested) {
				try {
					await this.#disposeRequest(requestId, request);
				} catch {
					return;
				}
				return;
			}
			if (this.#service.isPublicationTargetCurrent?.(presentationId, lease) === false) {
				try {
					await this.#disposeUnpublishedRequest(request);
				} catch {
					throw projectionControlFailure('terminal-projection-cleanup-failed', true);
				}
				throw projectionControlFailure('terminal-projection-retired', false);
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
			if (request.lease === null && this.#requests.get(requestId) === request) {
				this.#requests.delete(requestId);
			}
		}
	}

	async #disposeUnpublishedRequest(request: ProjectionRequest): Promise<void> {
		if (request.disposalPromise !== undefined) return request.disposalPromise;
		const lease = request.lease;
		if (lease === null) return;
		const operation = lease
			.dispose()
			.then(() => {
				request.lease = null;
			})
			.catch((error: unknown) => {
				request.disposalPromise = undefined;
				throw error;
			});
		request.disposalPromise = operation;
		return operation;
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
					if (!failed) firstFailure = error;
					failed = true;
				}
			}),
		);
		for (const [requestId, request] of this.#requests) {
			try {
				await this.#disposeRequest(requestId, request);
			} catch (error: unknown) {
				if (!failed) firstFailure = error;
				failed = true;
			}
		}
		if (failed) throw firstFailure;
		this.#requests.clear();
	}
}

function geometriesEqual(left: TerminalGeometry, right: TerminalGeometry): boolean {
	return (
		left.cellPixelHeight === right.cellPixelHeight &&
		left.cellPixelWidth === right.cellPixelWidth &&
		left.columns === right.columns &&
		left.rows === right.rows
	);
}

function projectionControlFailure(code: string, retryable: boolean): Error {
	return new Error(code, { cause: { code, retryable } });
}
