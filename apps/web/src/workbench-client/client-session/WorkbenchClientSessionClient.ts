import type {
	WorkbenchSharedDocument,
	WorkbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import {
	WorkbenchClientSessionBootstrapSchema,
	WorkbenchClientSessionCommandSchema,
	WorkbenchClientSessionEventSchema,
	WorkbenchClientSessionRequestIdSchema,
	type WorkbenchClientSessionBootstrap,
	type WorkbenchClientSessionEvent,
	type WorkbenchClientSessionRequestId,
	type WorkbenchDocumentProposalResult,
} from '@orchbee/workbench-client-protocol';

import { createBrowserRandomIdentifier } from '../../browser-random-identifier';
import { runApplicationCleanup } from '../../runApplicationCleanup';

export type WorkbenchClientSessionPublication = Exclude<
	WorkbenchClientSessionEvent,
	{ readonly type: 'shared-document-proposal-completed' }
>;

interface ControlUnavailable {
	readonly document: null;
	readonly kind: 'rejected';
	readonly reason: 'control-unavailable';
}

export interface WorkbenchClientSessionTransport {
	close(): void;
	send(command: ReturnType<typeof WorkbenchClientSessionCommandSchema.parse>): void;
	subscribe(listener: (value: unknown) => void, unavailable: () => void): () => void;
}

export class WorkbenchClientSessionClient {
	readonly bootstrap: WorkbenchClientSessionBootstrap;
	readonly #createRequestId: () => WorkbenchClientSessionRequestId;
	#disposed = false;
	readonly #listeners = new Set<(publication: WorkbenchClientSessionPublication) => void>();
	readonly #pending = new Map<
		WorkbenchClientSessionRequestId,
		(result: WorkbenchDocumentProposalResult | ControlUnavailable) => void
	>();
	#sequence: bigint;
	readonly #transport: WorkbenchClientSessionTransport;
	#unsubscribeTransport: (() => void) | undefined;

	constructor(
		transport: WorkbenchClientSessionTransport,
		bootstrap: WorkbenchClientSessionBootstrap,
		createRequestId: () => WorkbenchClientSessionRequestId = () =>
			WorkbenchClientSessionRequestIdSchema.parse(
				`workbench:proposal:${createBrowserRandomIdentifier()}`,
			),
	) {
		this.#transport = transport;
		this.bootstrap = WorkbenchClientSessionBootstrapSchema.parse(bootstrap);
		this.#sequence = BigInt(this.bootstrap.sequence);
		this.#createRequestId = createRequestId;
		try {
			const unsubscribe = transport.subscribe(this.#handleMessage, this.#handleUnavailable);
			if (this.#disposed) unsubscribe();
			else this.#unsubscribeTransport = unsubscribe;
		} catch (error) {
			try {
				this.dispose();
			} catch (cleanupError) {
				throw new AggregateError(
					[error, cleanupError],
					'Workbench client session initialization failed',
				);
			}
			throw error;
		}
	}

	propose(
		expectedRevision: WorkbenchSharedDocumentRevision | null,
		document: WorkbenchSharedDocument,
	): Promise<WorkbenchDocumentProposalResult | ControlUnavailable> {
		if (this.#disposed) return Promise.resolve(controlUnavailable());
		let requestId: WorkbenchClientSessionRequestId;
		try {
			requestId = WorkbenchClientSessionRequestIdSchema.parse(this.#createRequestId());
		} catch {
			this.dispose();
			return Promise.resolve(controlUnavailable());
		}
		if (this.#disposed) return Promise.resolve(controlUnavailable());
		if (this.#pending.has(requestId)) {
			this.dispose();
			return Promise.resolve(controlUnavailable());
		}
		return new Promise((resolve) => {
			this.#pending.set(requestId, resolve);
			try {
				this.#transport.send(
					WorkbenchClientSessionCommandSchema.parse({
						document,
						expectedRevision,
						requestId,
						type: 'shared-document-proposed',
					}),
				);
			} catch {
				this.dispose();
			}
		});
	}

	subscribe(listener: (publication: WorkbenchClientSessionPublication) => void): () => void {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		const unsubscribe = this.#unsubscribeTransport;
		this.#unsubscribeTransport = undefined;
		for (const resolve of this.#pending.values()) resolve(controlUnavailable());
		this.#pending.clear();
		this.#listeners.clear();
		runApplicationCleanup(
			[() => unsubscribe?.(), () => this.#transport.close()],
			'Workbench client session cleanup failed',
		);
	}

	readonly #handleMessage = (value: unknown): void => {
		if (this.#disposed) return;
		const parsed = WorkbenchClientSessionEventSchema.safeParse(value);
		if (!parsed.success) {
			this.dispose();
			return;
		}
		const event = parsed.data;
		if (event.type === 'shared-document-proposal-completed') {
			const resolve = this.#pending.get(event.requestId);
			if (resolve === undefined) {
				this.dispose();
				return;
			}
			this.#pending.delete(event.requestId);
			resolve(event.result);
			return;
		}
		const sequence = BigInt(event.sequence);
		if (sequence !== this.#sequence + 1n) {
			this.dispose();
			return;
		}
		this.#sequence = sequence;
		for (const listener of this.#listeners) listener(event);
	};

	readonly #handleUnavailable = (): void => {
		this.dispose();
	};
}

function controlUnavailable(): ControlUnavailable {
	return { document: null, kind: 'rejected', reason: 'control-unavailable' };
}
