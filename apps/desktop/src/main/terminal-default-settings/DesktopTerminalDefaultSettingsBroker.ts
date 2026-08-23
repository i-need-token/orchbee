import {
	DesktopTerminalDefaultSettingsMainEventSchema,
	DesktopTerminalDefaultSettingsRendererCommandSchema,
	type DesktopTerminalDefaultSettingsRequestId,
	type DesktopTerminalDefaultSettingsSnapshot,
} from '@orchbee/desktop-client-protocol';
import type { TerminalConfiguration } from '@orchbee/terminal-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';

import type { DesktopTerminalDefaultSettingsMutationResult } from './DesktopTerminalDefaultSettingsController';

export interface DesktopTerminalDefaultSettingsBrokerController {
	getSnapshot(): DesktopTerminalDefaultSettingsSnapshot;
	update(
		configuration: TerminalConfiguration,
	): Promise<DesktopTerminalDefaultSettingsMutationResult>;
}

export class DesktopTerminalDefaultSettingsBroker {
	#activeRequestId: DesktopTerminalDefaultSettingsRequestId | undefined;
	readonly #controller: DesktopTerminalDefaultSettingsBrokerController;
	#disposed = false;
	readonly #port: MessagePortMain;
	#started = false;

	constructor(port: MessagePortMain, controller: DesktopTerminalDefaultSettingsBrokerController) {
		this.#controller = controller;
		this.#port = port;
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

	readonly #handleClose = (): void => {
		this.#retire(false);
	};

	readonly #handleMessage = (event: MessageEvent): void => {
		if (event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = DesktopTerminalDefaultSettingsRendererCommandSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const command = parsed.data;
		if (this.#activeRequestId !== undefined) {
			if (this.#activeRequestId !== command.requestId) {
				this.#post(command.requestId, {
					code: 'busy',
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
		command: ReturnType<typeof DesktopTerminalDefaultSettingsRendererCommandSchema.parse>,
	): Promise<void> {
		try {
			const result =
				command.type === 'desktop-terminal-default-settings-snapshot-requested'
					? { kind: 'completed' as const, snapshot: this.#controller.getSnapshot() }
					: await this.#controller.update(command.configuration);
			this.#post(command.requestId, result);
		} catch {
			this.#post(command.requestId, {
				code: 'control-unavailable',
				kind: 'failed',
				snapshot: this.#controller.getSnapshot(),
			});
		}
	}

	#post(
		requestId: DesktopTerminalDefaultSettingsRequestId,
		result: DesktopTerminalDefaultSettingsMutationResult,
	): void {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(
				DesktopTerminalDefaultSettingsMainEventSchema.parse({
					requestId,
					result,
					type: 'desktop-terminal-default-settings-responded',
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
		this.#port.off('message', this.#handleMessage);
		this.#port.off('close', this.#handleClose);
		if (closePort) this.#port.close();
	}
}
