import {
	DesktopWindowLayoutDragSessionIdSchema,
	DesktopWindowLayoutMainEventSchema,
	DesktopWindowLayoutRendererCommandSchema,
	DesktopWindowLayoutRequestIdSchema,
	type DesktopWindowLayoutDestination,
	type DesktopWindowLayoutDragSessionId,
	type DesktopWindowLayoutOperationResult,
	type DesktopWindowLayoutRendererCommand,
	type DesktopWindowLayoutRequestId,
	type DesktopWindowLayoutSnapshot,
} from '@orchbee/desktop-client-protocol';
import type { WorkbenchTabScopeSource } from '@orchbee/workbench';

import { createBrowserRandomIdentifier } from '../../browser-random-identifier';

type CommandWithoutRequestId = DesktopWindowLayoutRendererCommand extends infer Command
	? Command extends DesktopWindowLayoutRendererCommand
		? Command extends { readonly requestId: DesktopWindowLayoutRequestId }
			? Omit<Command, 'requestId'>
			: never
		: never
	: never;

interface PendingOperation {
	readonly requestId: DesktopWindowLayoutRequestId;
	resolve(result: DesktopWindowLayoutOperationResult): void;
}

interface ActiveDrag {
	readonly sessionId: DesktopWindowLayoutDragSessionId;
	target: DesktopWindowLayoutDestination | null;
	settle: ((result: DesktopWindowLayoutOperationResult) => void) | undefined;
}

interface Deferred<Result> {
	readonly promise: Promise<Result>;
	resolve(result: Result): void;
}

type DesktopWindowLayoutClientInitialization =
	| { readonly kind: 'ready' }
	| { readonly kind: 'unavailable' };

export class DesktopWindowLayoutClient implements WorkbenchTabScopeSource {
	readonly #createIdentifier: () => string;
	#disposed = false;
	#drag: ActiveDrag | undefined;
	#initialization: Promise<DesktopWindowLayoutClientInitialization> | undefined;
	#initializeUnavailable: (() => void) | undefined;
	#initializeReady: (() => void) | undefined;
	readonly #listeners = new Set<() => void>();
	#pending: PendingOperation | undefined;
	readonly #port: MessagePort;
	#sequence = 0;
	#snapshot: DesktopWindowLayoutSnapshot | undefined;

	public constructor(port: MessagePort, createIdentifier: () => string = defaultIdentifier) {
		this.#port = port;
		this.#createIdentifier = createIdentifier;
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	public initialize(): Promise<DesktopWindowLayoutClientInitialization> {
		if (this.#snapshot !== undefined) return Promise.resolve({ kind: 'ready' });
		if (this.#disposed) return Promise.resolve({ kind: 'unavailable' });
		this.#initialization ??= new Promise<DesktopWindowLayoutClientInitialization>((resolve) => {
			this.#initializeReady = () => resolve({ kind: 'ready' });
			this.#initializeUnavailable = () => resolve({ kind: 'unavailable' });
		});
		return this.#initialization;
	}

	public readonly getSnapshot = (): DesktopWindowLayoutSnapshot => {
		if (this.#snapshot === undefined) {
			throw new Error('Desktop window layout must initialize before reading its snapshot.');
		}
		return this.#snapshot;
	};

	public readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	public setActiveTerminalContext(terminalContextId: string): void {
		this.#post({
			terminalContextId,
			type: 'desktop-window-layout-active-context-changed',
		});
	}

	public moveTabs(
		terminalContextId: string,
		tabIds: readonly string[],
		destination: DesktopWindowLayoutDestination,
	): Promise<DesktopWindowLayoutOperationResult> {
		return this.#request({
			destination,
			tabIds: [...tabIds],
			terminalContextId,
			type: 'desktop-window-layout-move-tabs-requested',
		});
	}

	public moveGroup(
		terminalContextId: string,
		groupId: string,
		destination: DesktopWindowLayoutDestination,
	): Promise<DesktopWindowLayoutOperationResult> {
		return this.#request({
			destination,
			groupId,
			terminalContextId,
			type: 'desktop-window-layout-move-group-requested',
		});
	}

	public startDrag(
		terminalContextId: string,
		tabIds: readonly string[],
	): DesktopWindowLayoutDragSessionId | null {
		if (this.#disposed || this.#snapshot === undefined || this.#drag !== undefined) return null;
		const sessionId = DesktopWindowLayoutDragSessionIdSchema.parse(
			`${this.#createIdentifier()}:drag:${++this.#sequence}`,
		);
		this.#drag = { sessionId, settle: undefined, target: null };
		if (
			!this.#post({
				sessionId,
				tabIds: [...tabIds],
				terminalContextId,
				type: 'desktop-window-layout-drag-started',
			})
		) {
			return null;
		}
		return sessionId;
	}

	public updateDrag(
		sessionId: DesktopWindowLayoutDragSessionId,
		point: Readonly<{ x: number; y: number }>,
	): void {
		if (this.#drag?.sessionId !== sessionId || this.#drag.settle !== undefined) return;
		this.#post({ point, sessionId, type: 'desktop-window-layout-drag-updated' });
	}

	public finishDrag(
		sessionId: DesktopWindowLayoutDragSessionId,
		input:
			| { readonly reason: 'cancel' }
			| { readonly point: Readonly<{ x: number; y: number }>; readonly reason: 'complete' },
	): Promise<DesktopWindowLayoutOperationResult> {
		const drag = this.#drag;
		if (drag?.sessionId !== sessionId || drag.settle !== undefined) {
			return Promise.resolve(controlUnavailable());
		}
		const settlement = createDeferred<DesktopWindowLayoutOperationResult>();
		drag.settle = settlement.resolve;
		const posted =
			input.reason === 'cancel'
				? this.#post({ sessionId, type: 'desktop-window-layout-drag-cancelled' })
				: this.#post({
						point: input.point,
						sessionId,
						type: 'desktop-window-layout-drag-completed',
					});
		if (!posted) settlement.resolve(controlUnavailable());
		return settlement.promise;
	}

	public getDragTarget(
		sessionId: DesktopWindowLayoutDragSessionId,
	): DesktopWindowLayoutDestination | null {
		return this.#drag?.sessionId === sessionId ? this.#drag.target : null;
	}

	public dispose(): void {
		this.#retire(true);
	}

	#request(command: CommandWithoutRequestId): Promise<DesktopWindowLayoutOperationResult> {
		if (this.#disposed || this.#snapshot === undefined) {
			return Promise.resolve(controlUnavailable());
		}
		if (this.#pending !== undefined) {
			return Promise.resolve({ code: 'busy', kind: 'failed' });
		}
		const requestId = DesktopWindowLayoutRequestIdSchema.parse(
			`${this.#createIdentifier()}:request:${++this.#sequence}`,
		);
		const settlement = createDeferred<DesktopWindowLayoutOperationResult>();
		this.#pending = { requestId, resolve: settlement.resolve };
		if (!this.#post({ ...command, requestId })) {
			settlement.resolve(controlUnavailable());
			this.#pending = undefined;
		}
		return settlement.promise;
	}

	readonly #handleMessage = (event: MessageEvent<unknown>): void => {
		if (this.#disposed || event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = DesktopWindowLayoutMainEventSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const message = parsed.data;
		switch (message.type) {
			case 'desktop-window-layout-snapshot-published': {
				const current = this.#snapshot;
				if (
					current !== undefined &&
					(message.snapshot.currentWindowId !== current.currentWindowId ||
						BigInt(message.snapshot.revision) <= BigInt(current.revision))
				) {
					return;
				}
				this.#snapshot = message.snapshot;
				this.#initializeReady?.();
				this.#initializeReady = undefined;
				this.#initializeUnavailable = undefined;
				this.#emit();
				break;
			}
			case 'desktop-window-layout-operation-responded': {
				const pending = this.#pending;
				if (pending === undefined || pending.requestId !== message.requestId) {
					this.#retire(true);
					return;
				}
				this.#pending = undefined;
				pending.resolve(message.result);
				break;
			}
			case 'desktop-window-layout-drag-target-changed': {
				if (this.#drag?.sessionId !== message.sessionId || this.#drag.settle !== undefined) {
					this.#retire(true);
					return;
				}
				this.#drag.target = message.target;
				this.#emit();
				break;
			}
			case 'desktop-window-layout-drag-settled': {
				const drag = this.#drag;
				if (drag?.sessionId !== message.sessionId || drag.settle === undefined) {
					this.#retire(true);
					return;
				}
				this.#drag = undefined;
				drag.settle(message.result);
				this.#emit();
				break;
			}
		}
	};

	readonly #handleMessageError = (): void => {
		this.#retire(true);
	};

	#post(command: DesktopWindowLayoutRendererCommand): boolean {
		if (this.#disposed) return false;
		try {
			this.#port.postMessage(DesktopWindowLayoutRendererCommandSchema.parse(command));
			return true;
		} catch {
			this.#retire(true);
			return false;
		}
	}

	#emit(): void {
		for (const listener of this.#listeners) listener();
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		if (closePort) this.#port.close();
		this.#initializeUnavailable?.();
		this.#initializeReady = undefined;
		this.#initializeUnavailable = undefined;
		this.#pending?.resolve(controlUnavailable());
		this.#pending = undefined;
		this.#drag?.settle?.(controlUnavailable());
		this.#drag = undefined;
		this.#listeners.clear();
	}
}

function controlUnavailable(): DesktopWindowLayoutOperationResult {
	return { code: 'control-unavailable', kind: 'failed' };
}

function createDeferred<Result>(): Deferred<Result> {
	let resolveResult: ((result: Result) => void) | undefined;
	const promise = new Promise<Result>((resolve) => {
		resolveResult = resolve;
	});
	if (resolveResult === undefined)
		throw new Error('Promise construction did not install a resolver.');
	return { promise, resolve: resolveResult };
}

function defaultIdentifier(): string {
	return `window-layout:${createBrowserRandomIdentifier()}`;
}
