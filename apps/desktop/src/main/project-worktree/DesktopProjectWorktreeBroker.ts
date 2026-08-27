import {
	DesktopProjectWorktreeMainEventSchema,
	DesktopProjectWorktreeRendererCommandSchema,
	type DesktopProjectWorktreeRequestId,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';

import type { DesktopProjectWorktreeRuntime } from './DesktopProjectWorktreeRuntime';

type DesktopProjectWorktreeBrokerRuntime = Pick<
	DesktopProjectWorktreeRuntime,
	'adopt' | 'create' | 'discover'
>;

export class DesktopProjectWorktreeBroker {
	#activeRequestId: DesktopProjectWorktreeRequestId | undefined;
	#disposed = false;
	readonly #port: MessagePortMain;
	readonly #runtime: DesktopProjectWorktreeBrokerRuntime;
	#started = false;

	constructor(port: MessagePortMain, runtime: DesktopProjectWorktreeBrokerRuntime) {
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
		const parsed = DesktopProjectWorktreeRendererCommandSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const command = parsed.data;
		let operation: 'adoption' | 'creation' | 'discovery' = 'creation';
		if (command.type === 'desktop-project-worktree-discovery-requested') {
			operation = 'discovery';
		} else if (command.type === 'desktop-project-worktree-adoption-requested') {
			operation = 'adoption';
		}
		if (this.#activeRequestId !== undefined) {
			if (this.#activeRequestId !== command.requestId) {
				this.#post({
					code: 'busy',
					operation,
					requestId: command.requestId,
					type: 'desktop-project-worktree-failed',
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
		command: ReturnType<typeof DesktopProjectWorktreeRendererCommandSchema.parse>,
	): Promise<void> {
		if (command.type === 'desktop-project-worktree-discovery-requested') {
			const result = await this.#runtime
				.discover(command.projectId, command.preferredDirectoryId)
				.catch(() => ({ code: 'discovery-failed' as const, kind: 'failed' as const }));
			this.#post(
				result.kind === 'completed'
					? {
							discovery: result.value,
							requestId: command.requestId,
							type: 'desktop-project-worktree-discovery-completed',
						}
					: {
							code: result.code,
							operation: 'discovery',
							requestId: command.requestId,
							type: 'desktop-project-worktree-failed',
						},
			);
			return;
		}
		if (command.type === 'desktop-project-worktree-adoption-requested') {
			const result = await this.#runtime
				.adopt(command)
				.catch(() => ({ code: 'document-conflict' as const, kind: 'failed' as const }));
			this.#post(
				result.kind === 'completed'
					? {
							requestId: command.requestId,
							result: result.value,
							type: 'desktop-project-worktree-adoption-completed',
						}
					: {
							code: result.code,
							operation: 'adoption',
							requestId: command.requestId,
							type: 'desktop-project-worktree-failed',
						},
			);
			return;
		}
		const result = await this.#runtime
			.create(command)
			.catch(() => ({ code: 'creation-failed' as const, kind: 'failed' as const }));
		this.#post(
			result.kind === 'completed'
				? {
						requestId: command.requestId,
						result: result.value,
						type: 'desktop-project-worktree-creation-completed',
					}
				: {
						code: result.code,
						operation: 'creation',
						requestId: command.requestId,
						type: 'desktop-project-worktree-failed',
					},
		);
	}

	#post(event: Parameters<typeof DesktopProjectWorktreeMainEventSchema.parse>[0]): void {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(DesktopProjectWorktreeMainEventSchema.parse(event));
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
