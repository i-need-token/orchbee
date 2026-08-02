import type {
	WorkbenchSharedDocument,
	WorkbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';

import {
	WorkbenchDocumentControlCommandSchema,
	WorkbenchDocumentControlEventSchema,
	WorkbenchDocumentRequestIdSchema,
	type WorkbenchDocumentCommitResult,
	type WorkbenchDocumentLoadResult,
	type WorkbenchDocumentRequestId,
} from '../../shared';

interface ControlUnavailable {
	readonly kind: 'unavailable';
	readonly reason: 'control-unavailable';
}

export type DesktopWorkbenchDocumentLoadResult = WorkbenchDocumentLoadResult | ControlUnavailable;
export type DesktopWorkbenchDocumentCommitResult =
	| WorkbenchDocumentCommitResult
	| ControlUnavailable;

type PendingRequest =
	| {
			readonly kind: 'load';
			readonly resolve: (result: DesktopWorkbenchDocumentLoadResult) => void;
	  }
	| {
			readonly kind: 'commit';
			readonly resolve: (result: DesktopWorkbenchDocumentCommitResult) => void;
	  };

export class DesktopWorkbenchDocumentClient {
	readonly #createRequestId: () => WorkbenchDocumentRequestId;
	readonly #pending = new Map<WorkbenchDocumentRequestId, PendingRequest>();
	readonly #port: MessagePort;
	#disposed = false;

	constructor(
		port: MessagePort,
		createRequestId: () => WorkbenchDocumentRequestId = () =>
			WorkbenchDocumentRequestIdSchema.parse(`desktop:document:${crypto.randomUUID()}`),
	) {
		this.#port = port;
		this.#createRequestId = createRequestId;
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	load(): Promise<DesktopWorkbenchDocumentLoadResult> {
		return new Promise((resolve) => {
			const requestId = this.#begin({ kind: 'load', resolve });
			if (requestId === null) return;
			this.#post({ requestId, type: 'workbench-document-load-requested' });
		});
	}

	commit(
		expectedRevision: WorkbenchSharedDocumentRevision | null,
		document: WorkbenchSharedDocument,
	): Promise<DesktopWorkbenchDocumentCommitResult> {
		return new Promise((resolve) => {
			const requestId = this.#begin({ kind: 'commit', resolve });
			if (requestId === null) return;
			this.#post({
				document,
				expectedRevision,
				requestId,
				type: 'workbench-document-commit-requested',
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

	#begin(pending: PendingRequest): WorkbenchDocumentRequestId | null {
		if (this.#disposed) {
			pending.resolve(controlUnavailable());
			return null;
		}
		let requestId: WorkbenchDocumentRequestId;
		try {
			requestId = WorkbenchDocumentRequestIdSchema.parse(this.#createRequestId());
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
		const parsed = WorkbenchDocumentControlCommandSchema.safeParse(command);
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
		const event = WorkbenchDocumentControlEventSchema.safeParse(message.data);
		if (!event.success) {
			this.dispose();
			return;
		}
		const pending = this.#pending.get(event.data.requestId);
		if (
			pending === undefined ||
			(pending.kind === 'load') !== (event.data.type === 'workbench-document-load-completed')
		) {
			this.dispose();
			return;
		}
		this.#pending.delete(event.data.requestId);
		if (pending.kind === 'load' && event.data.type === 'workbench-document-load-completed') {
			pending.resolve(event.data.result);
			return;
		}
		if (pending.kind === 'commit' && event.data.type === 'workbench-document-commit-completed') {
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
