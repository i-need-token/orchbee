import {
	DesktopBrowserAccessFailureCodeSchema,
	DesktopBrowserAccessMainEventSchema,
	DesktopBrowserAccessRendererCommandSchema,
	type DesktopBrowserAccessFailureCode,
	type DesktopBrowserAccessMode,
	type DesktopBrowserAccessRequestId,
	type DesktopBrowserAccessState,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';

export interface DesktopBrowserAccessBrokerRuntime {
	copyAccessLink(origin: string): Promise<DesktopBrowserAccessState>;
	getSnapshot(): DesktopBrowserAccessState;
	openLocalBrowser(): Promise<DesktopBrowserAccessState>;
	start(mode: DesktopBrowserAccessMode): Promise<DesktopBrowserAccessState>;
	stop(): Promise<DesktopBrowserAccessState>;
}

export class DesktopBrowserAccessBroker {
	readonly #activeRequestIds = new Set<DesktopBrowserAccessRequestId>();
	#disposed = false;
	readonly #port: MessagePortMain;
	readonly #runtime: DesktopBrowserAccessBrokerRuntime;
	#started = false;

	constructor(port: MessagePortMain, runtime: DesktopBrowserAccessBrokerRuntime) {
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

	readonly #handleClose = (): void => {
		this.#retire(false);
	};

	readonly #handleMessage = (event: MessageEvent): void => {
		if (event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = DesktopBrowserAccessRendererCommandSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const command = parsed.data;
		if (this.#activeRequestIds.has(command.requestId)) return;
		this.#activeRequestIds.add(command.requestId);
		void this.#execute(command).finally(() => this.#activeRequestIds.delete(command.requestId));
	};

	async #execute(
		command: ReturnType<typeof DesktopBrowserAccessRendererCommandSchema.parse>,
	): Promise<void> {
		try {
			switch (command.type) {
				case 'desktop-browser-access-state-requested':
					break;
				case 'desktop-browser-access-start-requested':
					await this.#runtime.start(command.mode);
					break;
				case 'desktop-browser-access-open-requested':
					await this.#runtime.openLocalBrowser();
					break;
				case 'desktop-browser-access-copy-link-requested':
					await this.#runtime.copyAccessLink(command.origin);
					break;
				case 'desktop-browser-access-stop-requested':
					await this.#runtime.stop();
			}
			this.#postCompleted(command.requestId);
		} catch (error: unknown) {
			this.#postFailed(command.requestId, failureCode(error));
		}
	}

	#postCompleted(requestId: DesktopBrowserAccessRequestId): void {
		this.#post({
			requestId,
			state: this.#runtime.getSnapshot(),
			type: 'desktop-browser-access-completed',
		});
	}

	#postFailed(
		requestId: DesktopBrowserAccessRequestId,
		code: DesktopBrowserAccessFailureCode,
	): void {
		this.#post({
			code,
			requestId,
			state: this.#runtime.getSnapshot(),
			type: 'desktop-browser-access-failed',
		});
	}

	#post(event: Parameters<typeof DesktopBrowserAccessMainEventSchema.parse>[0]): void {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(DesktopBrowserAccessMainEventSchema.parse(event));
		} catch {
			this.#retire(true);
		}
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.off('message', this.#handleMessage);
		this.#port.off('close', this.#handleClose);
		this.#activeRequestIds.clear();
		if (closePort) this.#port.close();
	}
}

function failureCode(error: unknown): DesktopBrowserAccessFailureCode {
	const candidate =
		typeof error === 'object' && error !== null && 'code' in error ? error.code : undefined;
	const parsed = DesktopBrowserAccessFailureCodeSchema.safeParse(candidate);
	return parsed.success ? parsed.data : 'gateway-unavailable';
}
