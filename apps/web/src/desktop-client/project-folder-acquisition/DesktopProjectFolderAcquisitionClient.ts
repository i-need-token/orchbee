import {
	DesktopProjectFolderAcquisitionMainEventSchema,
	DesktopProjectFolderAcquisitionRendererCommandSchema,
	DesktopProjectFolderAcquisitionRequestIdSchema,
	type DesktopProjectFolderAcquisitionRequestId,
} from '@orchbee/desktop-client-protocol';
import type {
	WorkbenchProjectFolderAcquisition,
	WorkbenchProjectFolderAcquisitionResult,
} from '@orchbee/workbench';

import { createBrowserRandomIdentifier } from '../../browser-random-identifier';

interface PendingSelection {
	readonly requestId: DesktopProjectFolderAcquisitionRequestId;
	resolve(result: WorkbenchProjectFolderAcquisitionResult): void;
}

export class DesktopProjectFolderAcquisitionClient implements WorkbenchProjectFolderAcquisition {
	readonly #createRequestId: () => DesktopProjectFolderAcquisitionRequestId;
	#disposed = false;
	#pending: PendingSelection | undefined;
	readonly #port: MessagePort;
	#sequence = 0;

	constructor(port: MessagePort, createRequestId: () => string = defaultRequestId) {
		this.#port = port;
		this.#createRequestId = () =>
			DesktopProjectFolderAcquisitionRequestIdSchema.parse(
				`${createRequestId()}:${++this.#sequence}`,
			);
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	selectExistingFolder(): Promise<WorkbenchProjectFolderAcquisitionResult> {
		if (this.#disposed) {
			return Promise.resolve({ code: 'control-unavailable', kind: 'failed' });
		}
		if (this.#pending !== undefined) return Promise.resolve({ code: 'busy', kind: 'failed' });
		const requestId = this.#createRequestId();
		const command = DesktopProjectFolderAcquisitionRendererCommandSchema.parse({
			requestId,
			type: 'desktop-project-folder-selection-requested',
		});
		const result = new Promise<WorkbenchProjectFolderAcquisitionResult>((resolve) => {
			this.#pending = { requestId, resolve };
		});
		try {
			this.#port.postMessage(command);
		} catch {
			this.#retire(true);
		}
		return result;
	}

	dispose(): void {
		this.#retire(true);
	}

	readonly #handleMessage = (event: MessageEvent<unknown>): void => {
		if (this.#disposed || event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = DesktopProjectFolderAcquisitionMainEventSchema.safeParse(event.data);
		const pending = this.#pending;
		if (!parsed.success || pending === undefined || parsed.data.requestId !== pending.requestId) {
			this.#retire(true);
			return;
		}
		this.#pending = undefined;
		if (parsed.data.type === 'desktop-project-folder-acquisition-completed') {
			pending.resolve(parsed.data.result);
		} else {
			pending.resolve({ code: parsed.data.code, kind: 'failed' });
		}
	};

	readonly #handleMessageError = (): void => {
		this.#retire(true);
	};

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		if (closePort) this.#port.close();
		this.#pending?.resolve({ code: 'control-unavailable', kind: 'failed' });
		this.#pending = undefined;
	}
}

function defaultRequestId(): string {
	return `project-folder:${createBrowserRandomIdentifier()}`;
}
