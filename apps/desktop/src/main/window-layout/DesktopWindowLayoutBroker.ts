import {
	DesktopWindowLayoutMainEventSchema,
	DesktopWindowLayoutRendererCommandSchema,
	type DesktopWindowLayoutDestination,
	type DesktopWindowLayoutDragSessionId,
	type DesktopWindowLayoutOperationResult,
	type DesktopWindowLayoutRequestId,
	type DesktopWindowLayoutSnapshot,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';

export interface DesktopWindowLayoutBrokerController {
	cancelDrag(
		sessionId: DesktopWindowLayoutDragSessionId,
	): Promise<DesktopWindowLayoutOperationResult>;
	completeDrag(
		sessionId: DesktopWindowLayoutDragSessionId,
		point: Readonly<{ x: number; y: number }>,
	): Promise<DesktopWindowLayoutOperationResult>;
	getSnapshot(): DesktopWindowLayoutSnapshot;
	moveGroup(
		terminalContextId: string,
		groupId: string,
		destination: DesktopWindowLayoutDestination,
	): Promise<DesktopWindowLayoutOperationResult>;
	moveTabs(
		terminalContextId: string,
		tabIds: readonly string[],
		destination: DesktopWindowLayoutDestination,
	): Promise<DesktopWindowLayoutOperationResult>;
	setActiveTerminalContext(terminalContextId: string): void;
	startDrag(
		sessionId: DesktopWindowLayoutDragSessionId,
		terminalContextId: string,
		tabIds: readonly string[],
	): void;
	subscribe(listener: () => void): () => void;
	updateDrag(
		sessionId: DesktopWindowLayoutDragSessionId,
		point: Readonly<{ x: number; y: number }>,
	): DesktopWindowLayoutDestination | null;
}

export class DesktopWindowLayoutBroker {
	#activeDragSessionId: DesktopWindowLayoutDragSessionId | undefined;
	#activeRequestId: DesktopWindowLayoutRequestId | undefined;
	readonly #controller: DesktopWindowLayoutBrokerController;
	#disposed = false;
	readonly #port: MessagePortMain;
	#started = false;
	#unsubscribe: (() => void) | undefined;

	public constructor(port: MessagePortMain, controller: DesktopWindowLayoutBrokerController) {
		this.#controller = controller;
		this.#port = port;
	}

	public start(): void {
		if (this.#started || this.#disposed) return;
		this.#started = true;
		this.#port.on('message', this.#handleMessage);
		this.#port.on('close', this.#handleClose);
		this.#unsubscribe = this.#controller.subscribe(this.#publishSnapshot);
		this.#port.start();
		this.#publishSnapshot();
	}

	public dispose(): void {
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
		const parsed = DesktopWindowLayoutRendererCommandSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const command = parsed.data;
		switch (command.type) {
			case 'desktop-window-layout-active-context-changed':
				this.#controller.setActiveTerminalContext(command.terminalContextId);
				break;
			case 'desktop-window-layout-move-group-requested':
				this.#executeOperation(command.requestId, () =>
					this.#controller.moveGroup(
						command.terminalContextId,
						command.groupId,
						command.destination,
					),
				);
				break;
			case 'desktop-window-layout-move-tabs-requested':
				this.#executeOperation(command.requestId, () =>
					this.#controller.moveTabs(command.terminalContextId, command.tabIds, command.destination),
				);
				break;
			case 'desktop-window-layout-drag-started':
				if (this.#activeDragSessionId !== undefined) {
					this.#retire(true);
					return;
				}
				this.#activeDragSessionId = command.sessionId;
				this.#controller.startDrag(command.sessionId, command.terminalContextId, command.tabIds);
				break;
			case 'desktop-window-layout-drag-updated':
				if (!this.#matchesActiveDrag(command.sessionId)) return;
				this.#post({
					sessionId: command.sessionId,
					target: this.#controller.updateDrag(command.sessionId, command.point),
					type: 'desktop-window-layout-drag-target-changed',
				});
				break;
			case 'desktop-window-layout-drag-cancelled':
				if (!this.#matchesActiveDrag(command.sessionId)) return;
				this.#settleDrag(command.sessionId, this.#controller.cancelDrag(command.sessionId));
				break;
			case 'desktop-window-layout-drag-completed':
				if (!this.#matchesActiveDrag(command.sessionId)) return;
				this.#settleDrag(
					command.sessionId,
					this.#controller.completeDrag(command.sessionId, command.point),
				);
				break;
		}
	};

	#matchesActiveDrag(sessionId: DesktopWindowLayoutDragSessionId): boolean {
		if (this.#activeDragSessionId === sessionId) return true;
		this.#retire(true);
		return false;
	}

	#executeOperation(
		requestId: DesktopWindowLayoutRequestId,
		operation: () => Promise<DesktopWindowLayoutOperationResult>,
	): void {
		if (this.#activeRequestId !== undefined) {
			this.#post({
				requestId,
				result: { code: 'busy', kind: 'failed' },
				type: 'desktop-window-layout-operation-responded',
			});
			return;
		}
		this.#activeRequestId = requestId;
		void operation()
			.catch(() => ({ code: 'control-unavailable', kind: 'failed' }) as const)
			.then((result) => {
				if (this.#activeRequestId !== requestId || this.#disposed) return;
				this.#activeRequestId = undefined;
				this.#post({
					requestId,
					result,
					type: 'desktop-window-layout-operation-responded',
				});
			});
	}

	#settleDrag(
		sessionId: DesktopWindowLayoutDragSessionId,
		settlement: Promise<DesktopWindowLayoutOperationResult>,
	): void {
		void settlement
			.catch(() => ({ code: 'control-unavailable', kind: 'failed' }) as const)
			.then((result) => {
				if (this.#activeDragSessionId !== sessionId || this.#disposed) return;
				this.#activeDragSessionId = undefined;
				this.#post({
					result,
					sessionId,
					type: 'desktop-window-layout-drag-settled',
				});
			});
	}

	readonly #publishSnapshot = (): void => {
		this.#post({
			snapshot: this.#controller.getSnapshot(),
			type: 'desktop-window-layout-snapshot-published',
		});
	};

	#post(message: unknown): void {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(DesktopWindowLayoutMainEventSchema.parse(message));
		} catch {
			this.#retire(true);
		}
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		const activeDragSessionId = this.#activeDragSessionId;
		this.#activeDragSessionId = undefined;
		this.#activeRequestId = undefined;
		this.#unsubscribe?.();
		this.#unsubscribe = undefined;
		this.#port.off('message', this.#handleMessage);
		this.#port.off('close', this.#handleClose);
		if (closePort) this.#port.close();
		if (activeDragSessionId !== undefined) {
			void this.#controller.cancelDrag(activeDragSessionId).catch(() => undefined);
		}
	}
}
