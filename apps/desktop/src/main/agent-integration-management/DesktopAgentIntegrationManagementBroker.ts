import {
	DesktopAgentIntegrationManagementMainEventSchema,
	DesktopAgentIntegrationManagementRendererCommandSchema,
	type DesktopAgentIntegrationId,
	type DesktopAgentIntegrationManagementRequestId,
	type DesktopAgentIntegrationManagementSnapshot,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';

import type { DesktopAgentIntegrationManagementOperationResult } from './DesktopAgentIntegrationManagementController';

export interface DesktopAgentIntegrationManagementBrokerController {
	disable(
		integrationId: DesktopAgentIntegrationId,
	): Promise<DesktopAgentIntegrationManagementOperationResult>;
	enable(
		integrationId: DesktopAgentIntegrationId,
	): Promise<DesktopAgentIntegrationManagementOperationResult>;
	getSnapshot(): DesktopAgentIntegrationManagementSnapshot;
	refresh(): Promise<DesktopAgentIntegrationManagementOperationResult>;
	subscribeLaunches(
		listener: (integrationId: DesktopAgentIntegrationId, revision: number) => void,
	): () => void;
}

export class DesktopAgentIntegrationManagementBroker {
	#activeRequestId: DesktopAgentIntegrationManagementRequestId | undefined;
	readonly #controller: DesktopAgentIntegrationManagementBrokerController;
	#disposed = false;
	readonly #port: MessagePortMain;
	#started = false;
	#unsubscribeLaunches: (() => void) | undefined;

	constructor(
		port: MessagePortMain,
		controller: DesktopAgentIntegrationManagementBrokerController,
	) {
		this.#controller = controller;
		this.#port = port;
	}

	start(): void {
		if (this.#started || this.#disposed) return;
		this.#started = true;
		this.#port.on('message', this.#handleMessage);
		this.#port.on('close', this.#handleClose);
		this.#unsubscribeLaunches = this.#controller.subscribeLaunches(this.#handleLaunch);
		this.#port.start();
	}

	dispose(): void {
		this.#retire(true);
	}

	readonly #handleClose = (): void => this.#retire(false);

	readonly #handleLaunch = (integrationId: DesktopAgentIntegrationId, revision: number): void => {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(
				DesktopAgentIntegrationManagementMainEventSchema.parse({
					integrationId,
					revision,
					type: 'desktop-agent-integration-launch-observed',
				}),
			);
		} catch {
			this.#retire(true);
		}
	};

	readonly #handleMessage = (event: MessageEvent): void => {
		if (event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = DesktopAgentIntegrationManagementRendererCommandSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const command = parsed.data;
		if (this.#activeRequestId !== undefined) {
			if (this.#activeRequestId !== command.requestId) {
				this.#post(command.requestId, {
					code: 'busy',
					integrationId: 'integrationId' in command ? command.integrationId : null,
					kind: 'failed',
					snapshot: this.#controller.getSnapshot(),
				});
			}
			return;
		}
		this.#activeRequestId = command.requestId;
		void this.#execute(command).finally(() => {
			if (this.#activeRequestId === command.requestId) this.#activeRequestId = undefined;
		});
	};

	async #execute(
		command: ReturnType<typeof DesktopAgentIntegrationManagementRendererCommandSchema.parse>,
	): Promise<void> {
		try {
			let result: DesktopAgentIntegrationManagementOperationResult;
			switch (command.type) {
				case 'desktop-agent-integration-inventory-requested':
					result = { kind: 'completed', snapshot: this.#controller.getSnapshot() };
					break;
				case 'desktop-agent-integration-refresh-requested':
					result = await this.#controller.refresh();
					break;
				case 'desktop-agent-integration-enable-requested':
					result = await this.#controller.enable(command.integrationId);
					break;
				case 'desktop-agent-integration-disable-requested':
					result = await this.#controller.disable(command.integrationId);
					break;
			}
			this.#post(command.requestId, result);
		} catch {
			this.#post(command.requestId, {
				code: 'control-unavailable',
				integrationId: 'integrationId' in command ? command.integrationId : null,
				kind: 'failed',
				snapshot: this.#controller.getSnapshot(),
			});
		}
	}

	#post(
		requestId: DesktopAgentIntegrationManagementRequestId,
		result: DesktopAgentIntegrationManagementOperationResult,
	): void {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(
				DesktopAgentIntegrationManagementMainEventSchema.parse({
					requestId,
					result,
					type: 'desktop-agent-integration-responded',
				}),
			);
		} catch {
			this.#retire(true);
		}
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#activeRequestId = undefined;
		this.#unsubscribeLaunches?.();
		this.#unsubscribeLaunches = undefined;
		this.#port.off('message', this.#handleMessage);
		this.#port.off('close', this.#handleClose);
		if (closePort) this.#port.close();
	}
}
