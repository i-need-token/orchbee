import {
	DesktopSshProjectEnrollmentMainEventSchema,
	DesktopSshProjectEnrollmentRendererCommandSchema,
	type DesktopSshProjectConnectionResult,
	type DesktopSshProjectConnectionStatus,
	type DesktopSshEnvironmentAddResult,
	type DesktopSshEnvironmentCatalog,
	type DesktopSshEnvironmentConnectionResult,
	type DesktopSshProjectDirectorySelectionResult,
	type DesktopSshProjectDirectorySnapshot,
	type DesktopSshProjectEnrollmentInput,
	type DesktopSshProjectEnrollmentRequestId,
	type DesktopSshProjectEnrollmentResult,
	type DesktopSshProjectFolderAttachmentResult,
	type DesktopSshProjectInteractionResult,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';

import type {
	DesktopSshProjectEnrollmentInteractionRequest,
	DesktopSshProjectEnrollmentOperation,
} from './DesktopSshProjectEnrollmentRuntime';

export interface DesktopSshProjectEnrollmentBrokerRuntime {
	addEnvironment(
		input: DesktopSshProjectEnrollmentInput,
		signal: AbortSignal,
	): Promise<DesktopSshEnvironmentAddResult>;
	attachFolder(
		projectId: string,
		input: DesktopSshProjectEnrollmentInput,
		operation: DesktopSshProjectEnrollmentOperation,
		signal: AbortSignal,
	): Promise<DesktopSshProjectFolderAttachmentResult>;
	connectProject(
		projectId: string,
		operation: Pick<DesktopSshProjectEnrollmentOperation, 'interact'>,
		signal: AbortSignal,
	): Promise<DesktopSshProjectConnectionResult>;
	disconnectProject(
		projectId: string,
		signal: AbortSignal,
	): Promise<DesktopSshProjectConnectionResult>;
	connectEnvironment(
		environmentId: string,
		operation: Pick<DesktopSshProjectEnrollmentOperation, 'interact'>,
		signal: AbortSignal,
	): Promise<DesktopSshEnvironmentConnectionResult>;
	disconnectEnvironment(
		environmentId: string,
		signal: AbortSignal,
	): Promise<DesktopSshEnvironmentConnectionResult>;
	environmentCatalog(refreshDiscovery: boolean): Promise<DesktopSshEnvironmentCatalog>;
	enroll(
		input: DesktopSshProjectEnrollmentInput,
		operation: DesktopSshProjectEnrollmentOperation,
		signal: AbortSignal,
	): Promise<DesktopSshProjectEnrollmentResult>;
	statuses(): readonly DesktopSshProjectConnectionStatus[];
	subscribeStatuses(
		listener: (statuses: readonly DesktopSshProjectConnectionStatus[]) => void,
	): () => void;
	subscribeEnvironmentCatalog(listener: () => void): () => void;
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
			readonly input: DesktopSshProjectEnrollmentInput;
			readonly kind: 'attachment';
			readonly projectId: string;
			readonly requestId: DesktopSshProjectEnrollmentRequestId;
	  }
	| {
			readonly abortController: AbortController;
			readonly kind: 'connect' | 'disconnect';
			readonly projectId: string;
			readonly requestId: DesktopSshProjectEnrollmentRequestId;
	  }
	| {
			readonly abortController: AbortController;
			readonly input: DesktopSshProjectEnrollmentInput;
			readonly kind: 'environment-add';
			readonly requestId: DesktopSshProjectEnrollmentRequestId;
	  }
	| {
			readonly abortController: AbortController;
			readonly environmentId: string;
			readonly kind: 'environment-connect' | 'environment-disconnect';
			readonly requestId: DesktopSshProjectEnrollmentRequestId;
	  };

interface PendingPromptInteraction {
	readonly generation: number;
	readonly handle: string;
	readonly kind: 'prompt';
	readonly requestId: DesktopSshProjectEnrollmentRequestId;
	resolve(result: DesktopSshProjectInteractionResult): void;
}

interface PendingDirectoryInteraction {
	readonly kind: 'directory';
	readonly requestId: DesktopSshProjectEnrollmentRequestId;
	readonly revision: number;
	resolve(result: DesktopSshProjectDirectorySelectionResult): void;
}

type PendingInteraction = PendingDirectoryInteraction | PendingPromptInteraction;

export class DesktopSshProjectEnrollmentBroker {
	#active: ActiveRequest | undefined;
	#catalogPublicationGeneration = 0;
	#directoryRevision = 0;
	#disposed = false;
	#pendingInteraction: PendingInteraction | undefined;
	readonly #port: MessagePortMain;
	readonly #runtime: DesktopSshProjectEnrollmentBrokerRuntime;
	#started = false;
	#unsubscribeStatuses: (() => void) | undefined;
	#unsubscribeEnvironmentCatalog: (() => void) | undefined;

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
		this.#unsubscribeEnvironmentCatalog = this.#runtime.subscribeEnvironmentCatalog(() =>
			this.#publishEnvironmentCatalog(null, false),
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
			case 'desktop-ssh-project-directory-selection-resolved':
				this.#resolveDirectorySelection(command);
				return;
			case 'desktop-ssh-project-status-requested':
				this.#postStatuses(command.requestId, this.#runtime.statuses());
				return;
			case 'desktop-ssh-environment-catalog-requested':
				this.#publishEnvironmentCatalog(command.requestId, command.refreshDiscovery);
				return;
			case 'desktop-ssh-environment-add-requested':
				this.#requestEnvironmentAdd(command.requestId, command.input);
				return;
			case 'desktop-ssh-environment-connect-requested':
				this.#requestEnvironmentConnection(
					command.requestId,
					command.environmentId,
					'environment-connect',
				);
				return;
			case 'desktop-ssh-environment-disconnect-requested':
				this.#requestEnvironmentConnection(
					command.requestId,
					command.environmentId,
					'environment-disconnect',
				);
				return;
			case 'desktop-ssh-project-enrollment-requested':
				this.#requestEnrollment(command.requestId, command.input);
				return;
			case 'desktop-ssh-project-folder-attachment-requested':
				this.#requestFolderAttachment(command.requestId, command.projectId, command.input);
				return;
			case 'desktop-ssh-project-connect-requested':
				this.#requestConnection(command.requestId, command.projectId, 'connect');
				return;
			case 'desktop-ssh-project-disconnect-requested':
				this.#requestConnection(command.requestId, command.projectId, 'disconnect');
				return;
		}
	};

	#requestEnvironmentAdd(
		requestId: DesktopSshProjectEnrollmentRequestId,
		input: DesktopSshProjectEnrollmentInput,
	): void {
		const active = this.#active;
		if (active !== undefined) {
			if (
				active.kind === 'environment-add' &&
				active.requestId === requestId &&
				JSON.stringify(active.input) === JSON.stringify(input)
			) {
				return;
			}
			if (active.requestId === requestId) {
				this.#retire(true);
				return;
			}
			this.#postEnvironmentAddResult(requestId, { kind: 'failed', reason: 'busy' });
			return;
		}
		const request: ActiveRequest = {
			abortController: new AbortController(),
			input,
			kind: 'environment-add',
			requestId,
		};
		this.#active = request;
		void this.#executeEnvironmentAdd(request).finally(() => this.#complete(request));
	}

	#requestEnvironmentConnection(
		requestId: DesktopSshProjectEnrollmentRequestId,
		environmentId: string,
		kind: 'environment-connect' | 'environment-disconnect',
	): void {
		const active = this.#active;
		if (active !== undefined) {
			if (
				active.kind === kind &&
				active.requestId === requestId &&
				active.environmentId === environmentId
			) {
				return;
			}
			if (active.requestId === requestId) {
				this.#retire(true);
				return;
			}
			this.#postEnvironmentConnectionResult(requestId, { kind: 'failed', reason: 'busy' });
			return;
		}
		const request: ActiveRequest = {
			abortController: new AbortController(),
			environmentId,
			kind,
			requestId,
		};
		this.#active = request;
		void this.#executeEnvironmentConnection(request).finally(() => this.#complete(request));
	}

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

	#requestFolderAttachment(
		requestId: DesktopSshProjectEnrollmentRequestId,
		projectId: string,
		input: DesktopSshProjectEnrollmentInput,
	): void {
		const active = this.#active;
		if (active !== undefined) {
			if (
				active.kind === 'attachment' &&
				active.requestId === requestId &&
				active.projectId === projectId &&
				JSON.stringify(active.input) === JSON.stringify(input)
			) {
				return;
			}
			if (active.requestId === requestId) {
				this.#retire(true);
				return;
			}
			this.#postFolderAttachmentResult(requestId, { kind: 'failed', reason: 'busy' });
			return;
		}
		const request: ActiveRequest = {
			abortController: new AbortController(),
			input,
			kind: 'attachment',
			projectId,
			requestId,
		};
		this.#active = request;
		void this.#executeFolderAttachment(request).finally(() => this.#complete(request));
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
					selectDirectory: (snapshot) => this.#requestDirectorySelection(request, snapshot),
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

	async #executeFolderAttachment(
		request: Extract<ActiveRequest, { readonly kind: 'attachment' }>,
	): Promise<void> {
		const result = await this.#runtime
			.attachFolder(
				request.projectId,
				request.input,
				{
					interact: (interaction) => this.#requestInteraction(request, interaction),
					progress: (stage) => {
						this.#post({
							requestId: request.requestId,
							stage,
							type: 'desktop-ssh-project-folder-attachment-progressed',
						});
					},
					selectDirectory: (snapshot) => this.#requestDirectorySelection(request, snapshot),
				},
				request.abortController.signal,
			)
			.catch(
				(): DesktopSshProjectFolderAttachmentResult => ({
					kind: 'indeterminate',
					reason: 'control-unavailable',
				}),
			);
		this.#postFolderAttachmentResult(request.requestId, result);
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

	async #executeEnvironmentAdd(
		request: Extract<ActiveRequest, { readonly kind: 'environment-add' }>,
	): Promise<void> {
		const result = await this.#runtime
			.addEnvironment(request.input, request.abortController.signal)
			.catch(
				(): DesktopSshEnvironmentAddResult => ({
					kind: 'indeterminate',
					reason: 'control-unavailable',
				}),
			);
		this.#postEnvironmentAddResult(request.requestId, result);
	}

	async #executeEnvironmentConnection(
		request: Extract<
			ActiveRequest,
			{ readonly kind: 'environment-connect' | 'environment-disconnect' }
		>,
	): Promise<void> {
		const result = await (
			request.kind === 'environment-connect'
				? this.#runtime.connectEnvironment(
						request.environmentId,
						{ interact: (interaction) => this.#requestInteraction(request, interaction) },
						request.abortController.signal,
					)
				: this.#runtime.disconnectEnvironment(request.environmentId, request.abortController.signal)
		).catch(
			(): DesktopSshEnvironmentConnectionResult => ({
				kind: 'indeterminate',
				reason: 'control-unavailable',
			}),
		);
		this.#postEnvironmentConnectionResult(request.requestId, result);
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
			let abort = (): void => undefined;
			const settle = (result: DesktopSshProjectInteractionResult): void => {
				request.abortController.signal.removeEventListener('abort', abort);
				resolve(result);
			};
			const pending = {
				generation: interaction.generation,
				handle: interaction.handle,
				kind: 'prompt' as const,
				requestId: request.requestId,
				resolve: settle,
			};
			this.#pendingInteraction = pending;
			abort = (): void => {
				if (this.#pendingInteraction !== pending) return;
				this.#pendingInteraction = undefined;
				settle({ kind: 'cancelled' });
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

	#requestDirectorySelection(
		request: Extract<ActiveRequest, { readonly kind: 'attachment' | 'enrollment' }>,
		snapshot: DesktopSshProjectDirectorySnapshot,
	): Promise<DesktopSshProjectDirectorySelectionResult> {
		if (
			this.#disposed ||
			this.#active !== request ||
			request.abortController.signal.aborted ||
			this.#pendingInteraction !== undefined ||
			this.#directoryRevision >= Number.MAX_SAFE_INTEGER
		) {
			return Promise.resolve({ kind: 'cancelled' });
		}
		return new Promise((resolve) => {
			let abort = (): void => undefined;
			const settle = (result: DesktopSshProjectDirectorySelectionResult): void => {
				request.abortController.signal.removeEventListener('abort', abort);
				resolve(result);
			};
			const pending: PendingDirectoryInteraction = {
				kind: 'directory',
				requestId: request.requestId,
				resolve: settle,
				revision: ++this.#directoryRevision,
			};
			this.#pendingInteraction = pending;
			abort = (): void => {
				if (this.#pendingInteraction !== pending) return;
				this.#pendingInteraction = undefined;
				settle({ kind: 'cancelled' });
			};
			request.abortController.signal.addEventListener('abort', abort, { once: true });
			this.#post({
				requestId: request.requestId,
				revision: pending.revision,
				snapshot,
				type: 'desktop-ssh-project-directory-selection-requested',
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
			pending.kind !== 'prompt' ||
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

	#resolveDirectorySelection(
		command: Extract<
			ReturnType<typeof DesktopSshProjectEnrollmentRendererCommandSchema.parse>,
			{ readonly type: 'desktop-ssh-project-directory-selection-resolved' }
		>,
	): void {
		const pending = this.#pendingInteraction;
		if (
			pending === undefined ||
			pending.kind !== 'directory' ||
			pending.requestId !== command.requestId ||
			pending.revision !== command.revision
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
			this.#cancelPending(pending);
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

	#postFolderAttachmentResult(
		requestId: DesktopSshProjectEnrollmentRequestId,
		result: DesktopSshProjectFolderAttachmentResult,
	): void {
		this.#post({
			requestId,
			result,
			type: 'desktop-ssh-project-folder-attachment-settled',
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

	#postEnvironmentAddResult(
		requestId: DesktopSshProjectEnrollmentRequestId,
		result: DesktopSshEnvironmentAddResult,
	): void {
		this.#post({ requestId, result, type: 'desktop-ssh-environment-add-settled' });
	}

	#postEnvironmentConnectionResult(
		requestId: DesktopSshProjectEnrollmentRequestId,
		result: DesktopSshEnvironmentConnectionResult,
	): void {
		this.#post({ requestId, result, type: 'desktop-ssh-environment-connection-settled' });
	}

	#publishEnvironmentCatalog(
		requestId: DesktopSshProjectEnrollmentRequestId | null,
		refreshDiscovery: boolean,
	): void {
		const generation = ++this.#catalogPublicationGeneration;
		void this.#runtime
			.environmentCatalog(refreshDiscovery)
			.catch(
				(): DesktopSshEnvironmentCatalog => ({
					discoveryIssue: 'read-failed',
					managed: [],
					registryIssue: 'unavailable',
					suggestions: [],
				}),
			)
			.then((catalog) => {
				if (generation !== this.#catalogPublicationGeneration || this.#disposed) return;
				this.#post({
					catalog,
					requestId,
					type: 'desktop-ssh-environment-catalog-published',
				});
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
		if (pending !== undefined) this.#cancelPending(pending);
		this.#unsubscribeStatuses?.();
		this.#unsubscribeStatuses = undefined;
		this.#unsubscribeEnvironmentCatalog?.();
		this.#unsubscribeEnvironmentCatalog = undefined;
		this.#port.off('message', this.#handleMessage);
		this.#port.off('close', this.#handleClose);
		if (closePort) this.#port.close();
	}

	#cancelPending(pending: PendingInteraction): void {
		pending.resolve({ kind: 'cancelled' });
	}
}
