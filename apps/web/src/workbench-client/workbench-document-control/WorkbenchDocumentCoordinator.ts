import type { WorkbenchProjectWorkspaceRuntime } from '@orchbee/workbench';

import type {
	WorkbenchClientSessionBootstrap,
	WorkbenchClientSessionEvent,
	WorkbenchDocumentProposalResult,
	WorkbenchSharedDocument,
	WorkbenchSharedDocumentRevision,
} from '@orchbee/workbench-client-protocol';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench-client-protocol';

import { runApplicationCleanup } from '../../runApplicationCleanup';

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
	#activeProposalDocument: WorkbenchSharedDocument | null = null;
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

	get committedDocument(): WorkbenchSharedDocument | null {
		return this.#committedDocument;
	}

	async start(): Promise<CoordinatorStartResult> {
		if (this.#disposed || this.#started) {
			return { kind: 'unavailable', reason: 'control-unavailable' };
		}
		this.#started = true;
		try {
			const bootstrap = this.#session.bootstrap;
			this.#committedDocument =
				bootstrap.documentState.kind === 'available' ? bootstrap.documentState.document : null;
			const expected = this.#committedDocument ?? bootstrap.initializationDocument;
			const projected = this.#documentRuntime.getSharedDocument(expected.revision);
			if (this.#disposed || this.#failedReason !== null) return this.#startUnavailable();
			if (documentFingerprint(projected) !== documentFingerprint(expected)) {
				return this.#fail('projection-mismatch');
			}
			const unsubscribeSession = this.#session.subscribe(this.#applyPublication);
			if (this.#disposed || this.#failedReason !== null) {
				unsubscribeSession();
				return this.#startUnavailable();
			}
			this.#unsubscribeSession = unsubscribeSession;
			const unsubscribeDocument = this.#documentRuntime.subscribeDocument(this.#schedule);
			if (this.#disposed || this.#failedReason !== null) {
				unsubscribeDocument();
				return this.#startUnavailable();
			}
			this.#unsubscribeDocument = unsubscribeDocument;
			for (const event of bootstrap.terminalEvents) {
				const result = this.#documentRuntime.applyTerminalPresentationEvent(event);
				if (this.#disposed || this.#failedReason !== null) return this.#startUnavailable();
				if (result.kind === 'rejected') return this.#fail('projection-mismatch');
			}
		} catch (error) {
			try {
				this.dispose();
			} catch (cleanupError) {
				throw new AggregateError([error, cleanupError], 'Workbench document initialization failed');
			}
			throw error;
		}
		await this.#pump();
		if (this.#disposed || this.#failedReason !== null) return this.#startUnavailable();
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
		this.#settleFlushWaiters();
		this.#releaseSubscriptions();
	}

	readonly #applyPublication = (
		publication: Exclude<
			WorkbenchClientSessionEvent,
			{ readonly type: 'shared-document-proposal-completed' }
		>,
	): void => {
		if (this.#disposed || this.#failedReason !== null) return;
		switch (publication.type) {
			case 'shared-document-published': {
				const acknowledgesActiveProposal =
					this.#activeProposalDocument !== null &&
					sameSharedDocument(publication.document, this.#activeProposalDocument);
				this.#committedDocument = publication.document;
				this.#documentPublicationGeneration += 1;
				if (!acknowledgesActiveProposal) {
					this.#documentRuntime.applySharedDocument(publication.document);
				}
				this.#schedule();
				break;
			}
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
				this.#activeProposalDocument = candidate;
				try {
					let result: WorkbenchDocumentProposalResult;
					try {
						result = await this.#session.propose(committed?.revision ?? null, candidate);
					} catch {
						// An exception carries no evidence that the proposal was not committed.
						// Retire admission instead of scheduling the same document again.
						this.#fail('control-unavailable');
						return;
					}
					if (this.#disposed) return;
					switch (result.kind) {
						case 'committed':
						case 'converged':
						case 'stale': {
							const resultWasPublishedDuringProposal =
								this.#documentPublicationGeneration !== publicationGeneration &&
								this.#committedDocument !== null &&
								sameSharedDocument(this.#committedDocument, result.document);
							const acknowledgesCandidate =
								result.kind !== 'stale' && sameSharedDocument(result.document, candidate);
							this.#committedDocument = result.document;
							if (!resultWasPublishedDuringProposal && !acknowledgesCandidate) {
								this.#documentRuntime.applySharedDocument(result.document);
							}
							continue;
						}
						case 'rejected':
						case 'unavailable':
							this.#fail(result.reason);
							return;
					}
				} finally {
					this.#activeProposalDocument = null;
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
		if (this.#disposed) return this.#startUnavailable();
		this.#failedReason ??= reason;
		this.#settleFlushWaiters();
		this.#releaseSubscriptions();
		return { kind: 'unavailable', reason: this.#failedReason };
	}

	#startUnavailable(): Extract<CoordinatorStartResult, { readonly kind: 'unavailable' }> {
		return {
			kind: 'unavailable',
			reason: this.#disposed
				? 'control-unavailable'
				: (this.#failedReason ?? 'control-unavailable'),
		};
	}

	#releaseSubscriptions(): void {
		const unsubscribeDocument = this.#unsubscribeDocument;
		const unsubscribeSession = this.#unsubscribeSession;
		this.#unsubscribeDocument = undefined;
		this.#unsubscribeSession = undefined;
		runApplicationCleanup(
			[() => unsubscribeDocument?.(), () => unsubscribeSession?.()],
			'Workbench document cleanup failed',
		);
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
