import type {
	WorkbenchTopologyDocument,
	WorkbenchTopologyRevision,
} from '@orchbee/workbench/topology';

import {
	WorkbenchTopologyControlCommandSchema,
	WorkbenchTopologyControlEventSchema,
	WorkbenchTopologyRequestIdSchema,
	type WorkbenchTopologyCommitResult,
	type WorkbenchTopologyLoadResult,
	type WorkbenchTopologyRequestId,
} from '../../shared';

interface ControlUnavailable {
	readonly kind: 'unavailable';
	readonly reason: 'control-unavailable';
}

export type DesktopWorkbenchTopologyLoadResult = WorkbenchTopologyLoadResult | ControlUnavailable;
export type DesktopWorkbenchTopologyCommitResult =
	| WorkbenchTopologyCommitResult
	| ControlUnavailable;

type PendingRequest =
	| {
			readonly kind: 'load';
			readonly resolve: (result: DesktopWorkbenchTopologyLoadResult) => void;
	  }
	| {
			readonly kind: 'commit';
			readonly resolve: (result: DesktopWorkbenchTopologyCommitResult) => void;
	  };

export class DesktopWorkbenchTopologyClient {
	readonly #createRequestId: () => WorkbenchTopologyRequestId;
	readonly #pending = new Map<WorkbenchTopologyRequestId, PendingRequest>();
	readonly #port: MessagePort;
	#disposed = false;

	constructor(
		port: MessagePort,
		createRequestId: () => WorkbenchTopologyRequestId = () =>
			WorkbenchTopologyRequestIdSchema.parse(`desktop:topology:${crypto.randomUUID()}`),
	) {
		this.#port = port;
		this.#createRequestId = createRequestId;
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	load(): Promise<DesktopWorkbenchTopologyLoadResult> {
		return new Promise((resolve) => {
			const requestId = this.#begin({ kind: 'load', resolve });
			if (requestId === null) return;
			this.#post({ requestId, type: 'workbench-topology-load-requested' });
		});
	}

	commit(
		expectedRevision: WorkbenchTopologyRevision | null,
		document: WorkbenchTopologyDocument,
	): Promise<DesktopWorkbenchTopologyCommitResult> {
		return new Promise((resolve) => {
			const requestId = this.#begin({ kind: 'commit', resolve });
			if (requestId === null) return;
			this.#post({
				document,
				expectedRevision,
				requestId,
				type: 'workbench-topology-commit-requested',
			});
		});
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		for (const pending of this.#pending.values()) pending.resolve(controlUnavailable());
		this.#pending.clear();
		this.#port.close();
	}

	#begin(pending: PendingRequest): WorkbenchTopologyRequestId | null {
		if (this.#disposed) {
			pending.resolve(controlUnavailable());
			return null;
		}
		let requestId: WorkbenchTopologyRequestId;
		try {
			requestId = WorkbenchTopologyRequestIdSchema.parse(this.#createRequestId());
		} catch {
			pending.resolve(controlUnavailable());
			this.dispose();
			return null;
		}
		if (this.#pending.has(requestId)) {
			pending.resolve(controlUnavailable());
			this.dispose();
			return null;
		}
		this.#pending.set(requestId, pending);
		return requestId;
	}

	#post(command: unknown): void {
		const parsed = WorkbenchTopologyControlCommandSchema.safeParse(command);
		if (!parsed.success) {
			this.dispose();
			return;
		}
		try {
			this.#port.postMessage(parsed.data);
		} catch {
			this.dispose();
		}
	}

	readonly #handleMessage = (message: MessageEvent<unknown>): void => {
		if (this.#disposed) return;
		if (message.ports.length !== 0) {
			for (const port of message.ports) port.close();
			this.dispose();
			return;
		}
		const event = WorkbenchTopologyControlEventSchema.safeParse(message.data);
		if (!event.success) {
			this.dispose();
			return;
		}
		const pending = this.#pending.get(event.data.requestId);
		if (
			pending === undefined ||
			(pending.kind === 'load') !== (event.data.type === 'workbench-topology-load-completed')
		) {
			this.dispose();
			return;
		}
		this.#pending.delete(event.data.requestId);
		if (pending.kind === 'load' && event.data.type === 'workbench-topology-load-completed') {
			pending.resolve(event.data.result);
			return;
		}
		if (pending.kind === 'commit' && event.data.type === 'workbench-topology-commit-completed') {
			pending.resolve(event.data.result);
		}
	};

	readonly #handleMessageError = (): void => {
		this.dispose();
	};
}

function controlUnavailable(): ControlUnavailable {
	return { kind: 'unavailable', reason: 'control-unavailable' };
}
