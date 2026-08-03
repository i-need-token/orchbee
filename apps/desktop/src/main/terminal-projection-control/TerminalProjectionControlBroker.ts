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
	lease: DesktopTerminalProjectionLease | null;
	readonly presentationId: TerminalPresentationId;
	released: boolean;
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
		return (this.#disposePromise ??= this.#retire(true));
	}

	readonly #handlePortClose = (): void => {
		this.#disposePromise ??= this.#retire(false);
	};

	readonly #handleRendererMessage = (event: MessageEvent): void => {
		if (this.#disposed) return;
		const parsed = TerminalProjectionControlCommandSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#disposePromise ??= this.#retire(true);
			return;
		}
		const command = parsed.data;
		const existing = this.#requests.get(command.requestId);
		if (command.type === 'terminal-projection-open-requested') {
			if (existing !== undefined) {
				if (existing.presentationId !== command.presentationId) {
					this.#disposePromise ??= this.#retire(true);
				}
				return;
			}
			const request: ProjectionRequest = {
				lease: null,
				presentationId: command.presentationId,
				released: false,
			};
			this.#requests.set(command.requestId, request);
			const task = this.#open(command.requestId, command.presentationId, request);
			this.#openTasks.add(task);
			void task.finally(() => this.#openTasks.delete(task));
			return;
		}
		if (existing === undefined || existing.presentationId !== command.presentationId) {
			this.#disposePromise ??= this.#retire(true);
			return;
		}
		if (existing.released) return;
		existing.released = true;
		const lease = existing.lease;
		existing.lease = null;
		if (lease !== null) {
			this.#requests.delete(command.requestId);
			void lease.dispose();
		}
	};

	async #open(
		requestId: TerminalProjectionRequestId,
		presentationId: Parameters<DesktopTerminalProjectionService['open']>[0],
		request: ProjectionRequest,
	): Promise<void> {
		try {
			const lease = await this.#service.open(presentationId);
			if (this.#disposed || request.released) {
				await lease.dispose();
				this.#requests.delete(requestId);
				return;
			}
			request.lease = lease;
			const event = TerminalProjectionControlEventSchema.parse({
				bootstrap: lease.bootstrap,
				presentationId,
				requestId,
				type: 'terminal-projection-opened',
			});
			try {
				this.#port.postMessage(event, [lease.rendererPort]);
			} catch {
				request.lease = null;
				await lease.dispose();
				this.#disposePromise ??= this.#retire(true);
			}
		} catch (error) {
			if (this.#disposed || request.released) return;
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
			this.#disposePromise ??= this.#retire(true);
			return;
		}
		try {
			this.#port.postMessage(parsed.data);
		} catch {
			this.#disposePromise ??= this.#retire(true);
		}
	}

	async #retire(closePort: boolean): Promise<void> {
		if (this.#disposed) return;
		this.#disposed = true;
		if (this.#started) {
			this.#port.off('message', this.#handleRendererMessage);
			this.#port.off('close', this.#handlePortClose);
		}
		if (closePort) this.#port.close();
		const leases = [...this.#requests.values()].flatMap((request) => {
			request.released = true;
			const lease = request.lease;
			request.lease = null;
			return lease === null ? [] : [lease];
		});
		await Promise.all(leases.map(async (lease) => lease.dispose()));
		await Promise.all(this.#openTasks);
		this.#requests.clear();
	}
}
