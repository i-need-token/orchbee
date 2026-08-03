import type {
	WorkbenchSharedDocument,
	WorkbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';

import {
	DesktopClientSessionBootstrapSchema,
	DesktopClientSessionCommandSchema,
	DesktopClientSessionEventSchema,
	DesktopClientSessionRequestIdSchema,
	type DesktopClientSessionBootstrap,
	type DesktopClientSessionEvent,
	type DesktopClientSessionRequestId,
	type DesktopDocumentProposalResult,
} from '../../shared';

type DesktopClientSessionPublication = Exclude<
	DesktopClientSessionEvent,
	{ readonly type: 'shared-document-proposal-completed' }
>;

interface ControlUnavailable {
	readonly document: null;
	readonly kind: 'rejected';
	readonly reason: 'control-unavailable';
}

export class DesktopClientSessionClient {
	readonly bootstrap: DesktopClientSessionBootstrap;
	readonly #createRequestId: () => DesktopClientSessionRequestId;
	#disposed = false;
	readonly #listeners = new Set<(publication: DesktopClientSessionPublication) => void>();
	readonly #pending = new Map<
		DesktopClientSessionRequestId,
		(result: DesktopDocumentProposalResult | ControlUnavailable) => void
	>();
	readonly #port: MessagePort;
	#sequence: bigint;

	constructor(
		port: MessagePort,
		bootstrap: DesktopClientSessionBootstrap,
		createRequestId: () => DesktopClientSessionRequestId = () =>
			DesktopClientSessionRequestIdSchema.parse(`desktop:proposal:${crypto.randomUUID()}`),
	) {
		this.#port = port;
		this.bootstrap = DesktopClientSessionBootstrapSchema.parse(bootstrap);
		this.#sequence = BigInt(this.bootstrap.sequence);
		this.#createRequestId = createRequestId;
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	propose(
		expectedRevision: WorkbenchSharedDocumentRevision | null,
		document: WorkbenchSharedDocument,
	): Promise<DesktopDocumentProposalResult | ControlUnavailable> {
		if (this.#disposed) return Promise.resolve(controlUnavailable());
		let requestId: DesktopClientSessionRequestId;
		try {
			requestId = DesktopClientSessionRequestIdSchema.parse(this.#createRequestId());
		} catch {
			this.dispose();
			return Promise.resolve(controlUnavailable());
		}
		if (this.#pending.has(requestId)) {
			this.dispose();
			return Promise.resolve(controlUnavailable());
		}
		return new Promise((resolve) => {
			this.#pending.set(requestId, resolve);
			try {
				this.#port.postMessage(
					DesktopClientSessionCommandSchema.parse({
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

	subscribe(listener: (publication: DesktopClientSessionPublication) => void): () => void {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		this.#port.close();
		for (const resolve of this.#pending.values()) resolve(controlUnavailable());
		this.#pending.clear();
		this.#listeners.clear();
	}

	readonly #handleMessage = (message: MessageEvent<unknown>): void => {
		if (this.#disposed) return;
		if (message.ports.length !== 0) {
			for (const port of message.ports) port.close();
			this.dispose();
			return;
		}
		const parsed = DesktopClientSessionEventSchema.safeParse(message.data);
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

	readonly #handleMessageError = (): void => {
		this.dispose();
	};
}

function controlUnavailable(): ControlUnavailable {
	return { document: null, kind: 'rejected', reason: 'control-unavailable' };
}
