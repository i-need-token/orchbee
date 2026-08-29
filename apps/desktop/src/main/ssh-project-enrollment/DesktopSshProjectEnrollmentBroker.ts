import {
	DesktopSshProjectEnrollmentMainEventSchema,
	DesktopSshProjectEnrollmentRendererCommandSchema,
	type DesktopSshProjectConnectionResult,
	type DesktopSshProjectConnectionStatus,
	type DesktopSshProjectEnrollmentInput,
	type DesktopSshProjectEnrollmentRequestId,
	type DesktopSshProjectEnrollmentResult,
	type DesktopSshProjectInteractionResult,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';

import type {
	DesktopSshProjectEnrollmentInteractionRequest,
	DesktopSshProjectEnrollmentOperation,
} from './DesktopSshProjectEnrollmentRuntime';

export interface DesktopSshProjectEnrollmentBrokerRuntime {
	connectProject(
		projectId: string,
		operation: Pick<DesktopSshProjectEnrollmentOperation, 'interact'>,
		signal: AbortSignal,
	): Promise<DesktopSshProjectConnectionResult>;
	disconnectProject(
		projectId: string,
		signal: AbortSignal,
	): Promise<DesktopSshProjectConnectionResult>;
	enroll(
		input: DesktopSshProjectEnrollmentInput,
		operation: DesktopSshProjectEnrollmentOperation,
		signal: AbortSignal,
	): Promise<DesktopSshProjectEnrollmentResult>;
	statuses(): readonly DesktopSshProjectConnectionStatus[];
	subscribeStatuses(
		listener: (statuses: readonly DesktopSshProjectConnectionStatus[]) => void,
	): () => void;
}

type ActiveRequest =
	| {
			readonly abortController: AbortController;
			readonly input: DesktopSshProjectEnrollmentInput;
			readonly kind: 'enrollment';
			readonly requestId: DesktopSshProjectEnrollmentRequestId;
	  }
	| {
			readonly abortController: AbortController;
			readonly kind: 'connect' | 'disconnect';
			readonly projectId: string;
			readonly requestId: DesktopSshProjectEnrollmentRequestId;
	  };

interface PendingInteraction {
	readonly generation: number;
	readonly handle: string;
	readonly requestId: DesktopSshProjectEnrollmentRequestId;
	resolve(result: DesktopSshProjectInteractionResult): void;
}

export class DesktopSshProjectEnrollmentBroker {
	#active: ActiveRequest | undefined;
	#disposed = false;
	#pendingInteraction: PendingInteraction | undefined;
	readonly #port: MessagePortMain;
	readonly #runtime: DesktopSshProjectEnrollmentBrokerRuntime;
	#started = false;
	#unsubscribeStatuses: (() => void) | undefined;

	constructor(port: MessagePortMain, runtime: DesktopSshProjectEnrollmentBrokerRuntime) {
		this.#port = port;
		this.#runtime = runtime;
	}

	start(): void {
		if (this.#started || this.#disposed) return;
		this.#started = true;
		this.#port.on('message', this.#handleMessage);
		this.#port.on('close', this.#handleClose);
		this.#port.start();
		this.#unsubscribeStatuses = this.#runtime.subscribeStatuses((statuses) =>
			this.#postStatuses(null, statuses),
		);
	}

	dispose(): void {
		this.#retire(true);
	}

	readonly #handleClose = (): void => this.#retire(false);

	readonly #handleMessage = (event: MessageEvent): void => {
		if (event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = DesktopSshProjectEnrollmentRendererCommandSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const command = parsed.data;
		switch (command.type) {
			case 'desktop-ssh-project-enrollment-cancel-requested':
				if (this.#active?.requestId === command.requestId) this.#active.abortController.abort();
				return;
			case 'desktop-ssh-project-interaction-resolved':
				this.#resolveInteraction(command);
				return;
			case 'desktop-ssh-project-status-requested':
				this.#postStatuses(command.requestId, this.#runtime.statuses());
				return;
			case 'desktop-ssh-project-enrollment-requested':
				this.#requestEnrollment(command.requestId, command.input);
				return;
			case 'desktop-ssh-project-connect-requested':
				this.#requestConnection(command.requestId, command.projectId, 'connect');
				return;
			case 'desktop-ssh-project-disconnect-requested':
				this.#requestConnection(command.requestId, command.projectId, 'disconnect');
				return;
		}
	};

	#requestEnrollment(
		requestId: DesktopSshProjectEnrollmentRequestId,
		input: DesktopSshProjectEnrollmentInput,
	): void {
		const active = this.#active;
		if (active !== undefined) {
			if (
				active.kind === 'enrollment' &&
				active.requestId === requestId &&
				JSON.stringify(active.input) === JSON.stringify(input)
			) {
				return;
			}
			if (active.requestId === requestId) {
				this.#retire(true);
				return;
			}
			this.#postEnrollmentResult(requestId, { kind: 'failed', reason: 'busy' });
			return;
		}
		const request: ActiveRequest = {
			abortController: new AbortController(),
			input,
			kind: 'enrollment',
			requestId,
		};
		this.#active = request;
		void this.#executeEnrollment(request).finally(() => this.#complete(request));
	}

	#requestConnection(
		requestId: DesktopSshProjectEnrollmentRequestId,
		projectId: string,
		kind: 'connect' | 'disconnect',
	): void {
		const active = this.#active;
		if (active !== undefined) {
			if (
				active.kind === kind &&
				active.requestId === requestId &&
				active.projectId === projectId
			) {
				return;
			}
			if (active.requestId === requestId) {
				this.#retire(true);
				return;
			}
			this.#postConnectionResult(requestId, { kind: 'failed', reason: 'busy' });
			return;
		}
		const request: ActiveRequest = {
			abortController: new AbortController(),
			kind,
			projectId,
			requestId,
		};
		this.#active = request;
		void this.#executeConnection(request).finally(() => this.#complete(request));
	}

	async #executeEnrollment(
		request: Extract<ActiveRequest, { readonly kind: 'enrollment' }>,
	): Promise<void> {
		const result = await this.#runtime
			.enroll(
				request.input,
				{
					interact: (interaction) => this.#requestInteraction(request, interaction),
					progress: (stage) => {
						this.#post({
							requestId: request.requestId,
							stage,
							type: 'desktop-ssh-project-enrollment-progressed',
						});
					},
				},
				request.abortController.signal,
			)
			.catch(
				(): DesktopSshProjectEnrollmentResult => ({
					kind: 'indeterminate',
					reason: 'control-unavailable',
				}),
			);
		this.#postEnrollmentResult(request.requestId, result);
	}

	async #executeConnection(
		request: Extract<ActiveRequest, { readonly kind: 'connect' | 'disconnect' }>,
	): Promise<void> {
		const result = await (
			request.kind === 'connect'
				? this.#runtime.connectProject(
						request.projectId,
						{ interact: (interaction) => this.#requestInteraction(request, interaction) },
						request.abortController.signal,
					)
				: this.#runtime.disconnectProject(request.projectId, request.abortController.signal)
		).catch(
			(): DesktopSshProjectConnectionResult => ({
				kind: 'indeterminate',
				reason: 'control-unavailable',
			}),
		);
		this.#postConnectionResult(request.requestId, result);
	}

	#requestInteraction(
		request: ActiveRequest,
		interaction: DesktopSshProjectEnrollmentInteractionRequest,
	): Promise<DesktopSshProjectInteractionResult> {
		if (
			this.#disposed ||
			this.#active !== request ||
			request.abortController.signal.aborted ||
			this.#pendingInteraction !== undefined
		) {
			return Promise.resolve({ kind: 'cancelled' });
		}
		return new Promise((resolve) => {
			const pending = {
				generation: interaction.generation,
				handle: interaction.handle,
				requestId: request.requestId,
				resolve,
			};
			this.#pendingInteraction = pending;
			const abort = (): void => {
				if (this.#pendingInteraction !== pending) return;
				this.#pendingInteraction = undefined;
				resolve({ kind: 'cancelled' });
			};
			request.abortController.signal.addEventListener('abort', abort, { once: true });
			this.#post({
				generation: interaction.generation,
				handle: interaction.handle,
				kind: interaction.kind,
				prompt: interaction.prompt,
				requestId: request.requestId,
				type: 'desktop-ssh-project-interaction-requested',
			});
		});
	}

	#resolveInteraction(
		command: Extract<
			ReturnType<typeof DesktopSshProjectEnrollmentRendererCommandSchema.parse>,
			{ readonly type: 'desktop-ssh-project-interaction-resolved' }
		>,
	): void {
		const pending = this.#pendingInteraction;
		if (
			pending === undefined ||
			pending.requestId !== command.requestId ||
			pending.generation !== command.generation ||
			pending.handle !== command.handle
		) {
			this.#retire(true);
			return;
		}
		this.#pendingInteraction = undefined;
		pending.resolve(command.result);
	}

	#complete(request: ActiveRequest): void {
		if (this.#active !== request) return;
		this.#active = undefined;
		const pending = this.#pendingInteraction;
		if (pending?.requestId === request.requestId) {
			this.#pendingInteraction = undefined;
			pending.resolve({ kind: 'cancelled' });
		}
	}

	#postEnrollmentResult(
		requestId: DesktopSshProjectEnrollmentRequestId,
		result: DesktopSshProjectEnrollmentResult,
	): void {
		this.#post({
			requestId,
			result,
			type: 'desktop-ssh-project-enrollment-settled',
		});
	}

	#postConnectionResult(
		requestId: DesktopSshProjectEnrollmentRequestId,
		result: DesktopSshProjectConnectionResult,
	): void {
		this.#post({
			requestId,
			result,
			type: 'desktop-ssh-project-connection-settled',
		});
	}

	#postStatuses(
		requestId: DesktopSshProjectEnrollmentRequestId | null,
		statuses: readonly DesktopSshProjectConnectionStatus[],
	): void {
		this.#post({
			requestId,
			statuses,
			type: 'desktop-ssh-project-status-published',
		});
	}

	#post(event: Parameters<typeof DesktopSshProjectEnrollmentMainEventSchema.parse>[0]): void {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(DesktopSshProjectEnrollmentMainEventSchema.parse(event));
		} catch {
			this.#retire(true);
		}
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#active?.abortController.abort();
		this.#active = undefined;
		const pending = this.#pendingInteraction;
		this.#pendingInteraction = undefined;
		pending?.resolve({ kind: 'cancelled' });
		this.#unsubscribeStatuses?.();
		this.#unsubscribeStatuses = undefined;
		this.#port.off('message', this.#handleMessage);
		this.#port.off('close', this.#handleClose);
		if (closePort) this.#port.close();
	}
}
