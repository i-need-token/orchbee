import {
	DesktopProjectFolderAcquisitionMainEventSchema,
	DesktopProjectFolderAcquisitionRendererCommandSchema,
	type DesktopProjectFolderAcquisitionFailureCode,
	type DesktopProjectFolderAcquisitionRequestId,
	type DesktopProjectFolderAcquisitionResult,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';

export type DesktopProjectFolderAcquisitionBrokerResult =
	| DesktopProjectFolderAcquisitionResult
	| { readonly code: DesktopProjectFolderAcquisitionFailureCode; readonly kind: 'failed' };

export interface DesktopProjectFolderAcquisitionBrokerRuntime {
	selectExistingFolder(): Promise<DesktopProjectFolderAcquisitionBrokerResult>;
}

export class DesktopProjectFolderAcquisitionBroker {
	#activeRequestId: DesktopProjectFolderAcquisitionRequestId | undefined;
	#disposed = false;
	readonly #port: MessagePortMain;
	readonly #runtime: DesktopProjectFolderAcquisitionBrokerRuntime;
	#started = false;

	constructor(port: MessagePortMain, runtime: DesktopProjectFolderAcquisitionBrokerRuntime) {
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
		const parsed = DesktopProjectFolderAcquisitionRendererCommandSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const requestId = parsed.data.requestId;
		if (this.#activeRequestId !== undefined) {
			if (this.#activeRequestId !== requestId) this.#postFailed(requestId, 'busy');
			return;
		}
		this.#activeRequestId = requestId;
		void this.#execute(requestId).finally(() => {
			if (this.#activeRequestId === requestId) this.#activeRequestId = undefined;
		});
	};

	async #execute(requestId: DesktopProjectFolderAcquisitionRequestId): Promise<void> {
		let result: DesktopProjectFolderAcquisitionBrokerResult;
		try {
			result = await this.#runtime.selectExistingFolder();
		} catch {
			result = { code: 'dialog-failed', kind: 'failed' };
		}
		if (result.kind === 'failed') this.#postFailed(requestId, result.code);
		else this.#postCompleted(requestId, result);
	}

	#postCompleted(
		requestId: DesktopProjectFolderAcquisitionRequestId,
		result: DesktopProjectFolderAcquisitionResult,
	): void {
		this.#post({
			requestId,
			result,
			type: 'desktop-project-folder-acquisition-completed',
		});
	}

	#postFailed(
		requestId: DesktopProjectFolderAcquisitionRequestId,
		code: DesktopProjectFolderAcquisitionFailureCode,
	): void {
		this.#post({
			code,
			requestId,
			type: 'desktop-project-folder-acquisition-failed',
		});
	}

	#post(event: Parameters<typeof DesktopProjectFolderAcquisitionMainEventSchema.parse>[0]): void {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(DesktopProjectFolderAcquisitionMainEventSchema.parse(event));
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
