import {
	DesktopTerminalLaunchProfileMainEventSchema,
	DesktopTerminalLaunchProfileRendererCommandSchema,
	type DesktopTerminalLaunchProfileRequestId,
	type DesktopTerminalLaunchProfileSnapshot,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';

import type {
	DesktopTerminalLaunchProfileMutation,
	DesktopTerminalLaunchProfileMutationResult,
} from './DesktopTerminalLaunchProfileController';

export interface DesktopTerminalLaunchProfileBrokerController {
	getSnapshot(): DesktopTerminalLaunchProfileSnapshot;
	mutate(
		mutation: DesktopTerminalLaunchProfileMutation,
		preferencesSource?: string,
	): Promise<DesktopTerminalLaunchProfileMutationResult>;
}

export class DesktopTerminalLaunchProfileBroker {
	#activeRequestId: DesktopTerminalLaunchProfileRequestId | undefined;
	readonly #controller: DesktopTerminalLaunchProfileBrokerController;
	#disposed = false;
	readonly #port: MessagePortMain;
	#started = false;

	constructor(port: MessagePortMain, controller: DesktopTerminalLaunchProfileBrokerController) {
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
		const parsed = DesktopTerminalLaunchProfileRendererCommandSchema.safeParse(event.data);
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
		command: ReturnType<typeof DesktopTerminalLaunchProfileRendererCommandSchema.parse>,
	): Promise<void> {
		try {
			const result =
				command.type === 'desktop-terminal-launch-profile-snapshot-requested'
					? { kind: 'completed' as const, snapshot: this.#controller.getSnapshot() }
					: await this.#controller.mutate(toMutation(command), command.preferencesSource);
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
		requestId: DesktopTerminalLaunchProfileRequestId,
		result: DesktopTerminalLaunchProfileMutationResult,
	): void {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(
				DesktopTerminalLaunchProfileMainEventSchema.parse({
					requestId,
					result,
					type: 'desktop-terminal-launch-profile-responded',
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

function toMutation(
	command: Exclude<
		ReturnType<typeof DesktopTerminalLaunchProfileRendererCommandSchema.parse>,
		{ readonly type: 'desktop-terminal-launch-profile-snapshot-requested' }
	>,
): DesktopTerminalLaunchProfileMutation {
	switch (command.type) {
		case 'desktop-terminal-launch-profile-create-requested':
			return {
				kind: 'create',
				name: command.name,
				sourceProfileId: command.sourceProfileId,
			};
		case 'desktop-terminal-launch-profile-update-requested':
			return { kind: 'update', profile: command.profile };
		case 'desktop-terminal-launch-profile-default-requested':
			return { kind: 'select-default', profileId: command.profileId };
		case 'desktop-terminal-launch-profile-delete-requested':
			return { kind: 'delete', profileId: command.profileId };
		case 'desktop-terminal-launch-profile-reset-requested':
			return { kind: 'reset' };
	}
}
