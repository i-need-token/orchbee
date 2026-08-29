import {
	DesktopRemoteTerminalCreationMainEventSchema,
	DesktopRemoteTerminalCreationRendererCommandSchema,
	type DesktopRemoteTerminalCreationRequestId,
	type DesktopRemoteTerminalCreationResult,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';

export interface DesktopRemoteTerminalCreationBrokerRuntime {
	create(
		projectId: string,
		workspaceId: string,
		signal: AbortSignal,
	): Promise<DesktopRemoteTerminalCreationResult>;
}

interface ActiveRequest {
	readonly abortController: AbortController;
	readonly projectId: string;
	readonly requestId: DesktopRemoteTerminalCreationRequestId;
	readonly workspaceId: string;
}

export class DesktopRemoteTerminalCreationBroker {
	#active: ActiveRequest | undefined;
	#disposed = false;
	readonly #port: MessagePortMain;
	readonly #runtime: DesktopRemoteTerminalCreationBrokerRuntime;
	#started = false;

	constructor(port: MessagePortMain, runtime: DesktopRemoteTerminalCreationBrokerRuntime) {
		this.#port = port;
		this.#runtime = runtime;
	}

	start(): void {
		if (this.#started || this.#disposed) return;
		this.#started = true;
		this.#port.on('message', this.#handleMessage);
		this.#port.on('close', this.#handleClose);
		this.#port.start();
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
		const parsed = DesktopRemoteTerminalCreationRendererCommandSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const command = parsed.data;
		if (command.type === 'desktop-remote-terminal-creation-cancel-requested') {
			if (this.#active?.requestId === command.requestId) {
				this.#active.abortController.abort();
			}
			return;
		}

		const active = this.#active;
		if (active !== undefined) {
			if (active.requestId === command.requestId) {
				if (active.projectId !== command.projectId || active.workspaceId !== command.workspaceId) {
					this.#retire(true);
				}
				return;
			}
			this.#post(command.requestId, { kind: 'failed', reason: 'busy' });
			return;
		}

		const request: ActiveRequest = {
			abortController: new AbortController(),
			projectId: command.projectId,
			requestId: command.requestId,
			workspaceId: command.workspaceId,
		};
		this.#active = request;
		void this.#execute(request).finally(() => {
			if (this.#active === request) this.#active = undefined;
		});
	};

	async #execute(request: ActiveRequest): Promise<void> {
		const result = await this.#runtime
			.create(request.projectId, request.workspaceId, request.abortController.signal)
			.catch(
				(): DesktopRemoteTerminalCreationResult => ({
					kind: 'failed',
					reason: 'workbench-unavailable',
				}),
			);
		this.#post(request.requestId, result);
	}

	#post(
		requestId: DesktopRemoteTerminalCreationRequestId,
		result: DesktopRemoteTerminalCreationResult,
	): void {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(
				DesktopRemoteTerminalCreationMainEventSchema.parse({
					requestId,
					result,
					type: 'desktop-remote-terminal-creation-settled',
				}),
			);
		} catch {
			this.#retire(true);
		}
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#active?.abortController.abort();
		this.#active = undefined;
		this.#port.off('message', this.#handleMessage);
		this.#port.off('close', this.#handleClose);
		if (closePort) this.#port.close();
	}
}
