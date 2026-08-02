import type { MessageEvent, MessagePortMain } from 'electron';

import {
	WorkbenchTopologyControlCommandSchema,
	WorkbenchTopologyControlEventSchema,
	type WorkbenchTopologyControlCommand,
	type WorkbenchTopologyControlEvent,
	type WorkbenchTopologyRequestId,
} from '../../shared';
import type { WorkbenchTopologyStore } from './decideWorkbenchTopologyCommit';

export class WorkbenchTopologyControlBroker {
	readonly #acceptedRequests = new Map<WorkbenchTopologyRequestId, string>();
	readonly #port: MessagePortMain;
	readonly #store: WorkbenchTopologyStore;
	#disposed = false;
	#started = false;

	constructor(port: MessagePortMain, store: WorkbenchTopologyStore) {
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
		const parsed = WorkbenchTopologyControlCommandSchema.safeParse(event.data);
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

	async #execute(command: WorkbenchTopologyControlCommand): Promise<void> {
		let event: WorkbenchTopologyControlEvent;
		if (command.type === 'workbench-topology-load-requested') {
			const result = await this.#store
				.load()
				.catch(() => ({ kind: 'unavailable', reason: 'read-failed' }) as const);
			event = {
				requestId: command.requestId,
				result,
				type: 'workbench-topology-load-completed',
			};
		} else {
			const result = await this.#store
				.commit(command.expectedRevision, command.document)
				.catch(() => ({ kind: 'unavailable', reason: 'write-failed' }) as const);
			event = {
				requestId: command.requestId,
				result,
				type: 'workbench-topology-commit-completed',
			};
		}
		this.#post(event);
	}

	#post(event: WorkbenchTopologyControlEvent): void {
		if (this.#disposed) return;
		const parsed = WorkbenchTopologyControlEventSchema.safeParse(event);
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
