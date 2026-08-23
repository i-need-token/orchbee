import type { WorkbenchProjectWorkspaceRuntime } from '@orchbee/workbench';
import {
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
	type WorkbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';

import type {
	WorkbenchClientSessionBootstrap,
	WorkbenchClientSessionEvent,
	WorkbenchDocumentProposalResult,
} from '@orchbee/workbench-client-protocol';

type CoordinatorFailureReason =
	| 'application-quiescing'
	| 'control-unavailable'
	| 'exit-transition'
	| 'invalid'
	| 'projection-mismatch'
	| 'read-failed'
	| 'revision-sequence'
	| 'write-failed';

type CoordinatorStartResult =
	| { readonly kind: 'ready'; readonly revision: WorkbenchSharedDocumentRevision }
	| { readonly kind: 'unavailable'; readonly reason: CoordinatorFailureReason };

export type WorkbenchDocumentFlushResult =
	| { readonly kind: 'flushed'; readonly revision: WorkbenchSharedDocumentRevision }
	| { readonly kind: 'unavailable'; readonly reason: CoordinatorFailureReason };

interface WorkbenchClientSessionPort {
	readonly bootstrap: WorkbenchClientSessionBootstrap;
	propose(
		expectedRevision: WorkbenchSharedDocumentRevision | null,
		document: WorkbenchSharedDocument,
	): Promise<WorkbenchDocumentProposalResult>;
	subscribe(
		listener: (
			publication: Exclude<
				WorkbenchClientSessionEvent,
				{ readonly type: 'shared-document-proposal-completed' }
			>,
		) => void,
	): () => void;
}

type DocumentRuntime = Pick<
	WorkbenchProjectWorkspaceRuntime,
	| 'applySharedDocument'
	| 'applyTerminalPresentationEvent'
	| 'getSharedDocument'
	| 'subscribeDocument'
>;

export class WorkbenchDocumentCoordinator {
	#committedDocument: WorkbenchSharedDocument | null = null;
	#documentPublicationGeneration = 0;
	#disposed = false;
	#failedReason: CoordinatorFailureReason | null = null;
	readonly #flushWaiters = new Set<(result: WorkbenchDocumentFlushResult) => void>();
	#pumping = false;
	#scheduled = false;
	readonly #session: WorkbenchClientSessionPort;
	#started = false;
	readonly #documentRuntime: DocumentRuntime;
	#unsubscribeDocument: (() => void) | undefined;
	#unsubscribeSession: (() => void) | undefined;

	constructor(documentRuntime: DocumentRuntime, session: WorkbenchClientSessionPort) {
		this.#documentRuntime = documentRuntime;
		this.#session = session;
	}

	get failureReason(): CoordinatorFailureReason | null {
		return this.#failedReason;
	}

	get committedRevision(): WorkbenchSharedDocumentRevision | null {
		return this.#committedDocument?.revision ?? null;
	}

	async start(): Promise<CoordinatorStartResult> {
		if (this.#disposed || this.#started) {
			return { kind: 'unavailable', reason: 'control-unavailable' };
		}
		this.#started = true;
		const bootstrap = this.#session.bootstrap;
		this.#committedDocument =
			bootstrap.documentState.kind === 'available' ? bootstrap.documentState.document : null;
		const expected = this.#committedDocument ?? bootstrap.initializationDocument;
		const projected = this.#documentRuntime.getSharedDocument(expected.revision);
		if (documentFingerprint(projected) !== documentFingerprint(expected)) {
			return this.#fail('projection-mismatch');
		}
		this.#unsubscribeSession = this.#session.subscribe(this.#applyPublication);
		this.#unsubscribeDocument = this.#documentRuntime.subscribeDocument(this.#schedule);
		for (const event of bootstrap.terminalEvents) {
			const result = this.#documentRuntime.applyTerminalPresentationEvent(event);
			if (result.kind === 'rejected') return this.#fail('projection-mismatch');
		}
		await this.#pump();
		if (this.#failedReason !== null) return { kind: 'unavailable', reason: this.#failedReason };
		const revision = this.#committedDocument?.revision;
		return revision === undefined ? this.#fail('control-unavailable') : { kind: 'ready', revision };
	}

	flush(): Promise<WorkbenchDocumentFlushResult> {
		if (this.#disposed) {
			return Promise.resolve({ kind: 'unavailable', reason: 'control-unavailable' });
		}
		if (this.#failedReason !== null) {
			return Promise.resolve({ kind: 'unavailable', reason: this.#failedReason });
		}
		if (!this.#started) {
			return Promise.resolve({ kind: 'unavailable', reason: 'control-unavailable' });
		}
		const result = new Promise<WorkbenchDocumentFlushResult>((resolve) => {
			this.#flushWaiters.add(resolve);
		});
		this.#schedule();
		this.#settleFlushWaiters();
		return result;
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#unsubscribeDocument?.();
		this.#unsubscribeDocument = undefined;
		this.#unsubscribeSession?.();
		this.#unsubscribeSession = undefined;
		this.#settleFlushWaiters();
	}

	readonly #applyPublication = (
		publication: Exclude<
			WorkbenchClientSessionEvent,
			{ readonly type: 'shared-document-proposal-completed' }
		>,
	): void => {
		if (this.#disposed || this.#failedReason !== null) return;
		switch (publication.type) {
			case 'shared-document-published':
				this.#committedDocument = publication.document;
				this.#documentPublicationGeneration += 1;
				this.#documentRuntime.applySharedDocument(publication.document);
				this.#schedule();
				break;
			case 'terminal-presentation-event-published': {
				const result = this.#documentRuntime.applyTerminalPresentationEvent(publication.event);
				if (result.kind === 'rejected') this.#fail('projection-mismatch');
				break;
			}
			case 'terminal-metadata-published':
				// Terminal-global metadata is transient presentation state owned by the
				// page composition, not by shared-document convergence.
				break;
			case 'client-session-coordinator-unavailable':
				this.#fail(publication.reason);
				break;
		}
	};

	readonly #schedule = (): void => {
		if (this.#disposed || this.#failedReason !== null || !this.#started || this.#scheduled) {
			return;
		}
		this.#scheduled = true;
		queueMicrotask(() => {
			this.#scheduled = false;
			void this.#pump();
		});
	};

	async #pump(): Promise<void> {
		if (this.#disposed || this.#failedReason !== null || this.#pumping) return;
		this.#pumping = true;
		try {
			while (!this.#disposed && this.#failedReason === null) {
				const committed = this.#committedDocument;
				const candidate = this.#documentRuntime.getSharedDocument(
					nextDocumentRevision(committed?.revision ?? null),
				);
				if (
					committed !== null &&
					documentFingerprint(candidate) === documentFingerprint(committed)
				) {
					break;
				}
				const publicationGeneration = this.#documentPublicationGeneration;
				const result = await this.#session.propose(committed?.revision ?? null, candidate);
				if (this.#disposed) return;
				switch (result.kind) {
					case 'committed':
					case 'converged':
					case 'stale': {
						const resultWasPublishedDuringProposal =
							this.#documentPublicationGeneration !== publicationGeneration &&
							this.#committedDocument !== null &&
							sameSharedDocument(this.#committedDocument, result.document);
						this.#committedDocument = result.document;
						if (!resultWasPublishedDuringProposal) {
							this.#documentRuntime.applySharedDocument(result.document);
						}
						continue;
					}
					case 'rejected':
					case 'unavailable':
						this.#fail(result.reason);
						return;
				}
			}
		} finally {
			this.#pumping = false;
			if (this.#needsPump()) this.#schedule();
			else this.#settleFlushWaiters();
		}
	}

	#needsPump(): boolean {
		if (this.#disposed || this.#failedReason !== null || !this.#started) return false;
		if (this.#committedDocument === null) return true;
		const projected = this.#documentRuntime.getSharedDocument(this.#committedDocument.revision);
		return documentFingerprint(projected) !== documentFingerprint(this.#committedDocument);
	}

	#fail(
		reason: CoordinatorFailureReason,
	): Extract<CoordinatorStartResult, { readonly kind: 'unavailable' }> {
		this.#failedReason = reason;
		this.#unsubscribeDocument?.();
		this.#unsubscribeDocument = undefined;
		this.#settleFlushWaiters();
		return { kind: 'unavailable', reason };
	}

	#settleFlushWaiters(): void {
		if (this.#flushWaiters.size === 0) return;
		let result: WorkbenchDocumentFlushResult | null = null;
		if (this.#disposed) {
			result = { kind: 'unavailable', reason: 'control-unavailable' };
		} else if (this.#failedReason !== null) {
			result = { kind: 'unavailable', reason: this.#failedReason };
		} else if (
			this.#started &&
			this.#committedDocument !== null &&
			!this.#pumping &&
			!this.#scheduled &&
			!this.#needsPump()
		) {
			result = { kind: 'flushed', revision: this.#committedDocument.revision };
		}
		if (result === null) return;
		const waiters = [...this.#flushWaiters];
		this.#flushWaiters.clear();
		for (const resolve of waiters) resolve(result);
	}
}

function nextDocumentRevision(
	revision: WorkbenchSharedDocumentRevision | null,
): WorkbenchSharedDocumentRevision {
	return revision === null
		? workbenchSharedDocumentRevision('0')
		: workbenchSharedDocumentRevision((BigInt(revision) + 1n).toString());
}

function documentFingerprint(document: WorkbenchSharedDocument): string {
	return JSON.stringify([document.version, document.projectWorkspace, document.terminalContexts]);
}

function sameSharedDocument(
	left: WorkbenchSharedDocument,
	right: WorkbenchSharedDocument,
): boolean {
	return (
		left.revision === right.revision && documentFingerprint(left) === documentFingerprint(right)
	);
}
