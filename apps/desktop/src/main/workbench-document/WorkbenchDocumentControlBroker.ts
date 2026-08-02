import type { MessageEvent, MessagePortMain } from 'electron';

import {
	WorkbenchDocumentControlCommandSchema,
	WorkbenchDocumentControlEventSchema,
	type WorkbenchDocumentControlCommand,
	type WorkbenchDocumentControlEvent,
	type WorkbenchDocumentRequestId,
} from '../../shared';
import type { WorkbenchDocumentStore } from './decideWorkbenchDocumentCommit';

export class WorkbenchDocumentControlBroker {
	readonly #acceptedRequests = new Map<WorkbenchDocumentRequestId, string>();
	readonly #port: MessagePortMain;
	readonly #store: WorkbenchDocumentStore;
	#disposed = false;
	#started = false;

	constructor(port: MessagePortMain, store: WorkbenchDocumentStore) {
		this.#port = port;
		this.#store = store;
	}

	start(): void {
		if (this.#disposed || this.#started) return;
		this.#started = true;
		this.#port.on('message', this.#handleRendererMessage);
		this.#port.on('close', this.#handlePortClose);
		this.#port.start();
	}

	dispose(): void {
		this.#retire(true);
	}

	readonly #handlePortClose = (): void => {
		this.#retire(false);
	};

	readonly #handleRendererMessage = (event: MessageEvent): void => {
		if (this.#disposed) return;
		if (event.ports.length !== 0) {
			this.#retire(true);
			return;
		}
		const parsed = WorkbenchDocumentControlCommandSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		const fingerprint = JSON.stringify(parsed.data);
		const existingFingerprint = this.#acceptedRequests.get(parsed.data.requestId);
		if (existingFingerprint !== undefined) {
			if (existingFingerprint !== fingerprint) this.#retire(true);
			return;
		}
		this.#acceptedRequests.set(parsed.data.requestId, fingerprint);
		void this.#execute(parsed.data);
	};

	async #execute(command: WorkbenchDocumentControlCommand): Promise<void> {
		let event: WorkbenchDocumentControlEvent;
		if (command.type === 'workbench-document-load-requested') {
			const result = await this.#store
				.load()
				.catch(() => ({ kind: 'unavailable', reason: 'read-failed' }) as const);
			event = {
				requestId: command.requestId,
				result,
				type: 'workbench-document-load-completed',
			};
		} else {
			const result = await this.#store
				.commit(command.expectedRevision, command.document)
				.catch(() => ({ kind: 'unavailable', reason: 'write-failed' }) as const);
			event = {
				requestId: command.requestId,
				result,
				type: 'workbench-document-commit-completed',
			};
		}
		this.#post(event);
	}

	#post(event: WorkbenchDocumentControlEvent): void {
		if (this.#disposed) return;
		const parsed = WorkbenchDocumentControlEventSchema.safeParse(event);
		if (!parsed.success) {
			this.#retire(true);
			return;
		}
		try {
			this.#port.postMessage(parsed.data);
		} catch {
			this.#retire(true);
		}
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		if (this.#started) {
			this.#port.off('message', this.#handleRendererMessage);
			this.#port.off('close', this.#handlePortClose);
		}
		this.#acceptedRequests.clear();
		if (closePort) this.#port.close();
	}
}
