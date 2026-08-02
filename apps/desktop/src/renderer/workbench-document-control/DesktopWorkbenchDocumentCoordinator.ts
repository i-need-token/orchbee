import type {
	TerminalPresentationCommand,
	WorkbenchProjectWorkspaceRuntime,
} from '@orchbee/workbench';
import {
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
	type WorkbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';

import type { WorkbenchDocumentCommitResult, WorkbenchDocumentLoadResult } from '../../shared';

type InitialDocumentLoadResult = Extract<
	WorkbenchDocumentLoadResult,
	{ readonly kind: 'available' | 'missing' }
>;

type CoordinatorFailureReason =
	| 'conflict'
	| 'control-unavailable'
	| 'invalid'
	| 'projection-mismatch'
	| 'read-failed'
	| 'revision-sequence'
	| 'write-failed';

type CoordinatorStartResult =
	| { readonly kind: 'ready'; readonly revision: WorkbenchSharedDocumentRevision }
	| { readonly kind: 'unavailable'; readonly reason: CoordinatorFailureReason };

export type DesktopWorkbenchDocumentFlushResult =
	| { readonly kind: 'flushed'; readonly revision: WorkbenchSharedDocumentRevision }
	| { readonly kind: 'unavailable'; readonly reason: CoordinatorFailureReason };

interface WorkbenchDocumentCommitPort {
	commit(
		expectedRevision: WorkbenchSharedDocumentRevision | null,
		document: WorkbenchSharedDocument,
	): Promise<
		| WorkbenchDocumentCommitResult
		| { readonly kind: 'unavailable'; readonly reason: 'control-unavailable' }
	>;
}

interface TerminalCommandPort {
	dispatch(commands: readonly TerminalPresentationCommand[]): void;
}

type DocumentRuntime = Pick<
	WorkbenchProjectWorkspaceRuntime,
	'getSharedDocument' | 'subscribeDocument'
>;

export class DesktopWorkbenchDocumentCoordinator {
	readonly #commandPort: TerminalCommandPort;
	#committedDocument: WorkbenchSharedDocument | undefined;
	#disposed = false;
	#failedReason: CoordinatorFailureReason | null = null;
	readonly #flushWaiters = new Set<(result: DesktopWorkbenchDocumentFlushResult) => void>();
	readonly #pendingCommands: TerminalPresentationCommand[] = [];
	#pumping = false;
	#scheduled = false;
	#started = false;
	readonly #documentPort: WorkbenchDocumentCommitPort;
	readonly #documentRuntime: DocumentRuntime;
	#unsubscribeDocument: (() => void) | undefined;

	constructor(
		documentRuntime: DocumentRuntime,
		documentPort: WorkbenchDocumentCommitPort,
		commandPort: TerminalCommandPort,
	) {
		this.#documentRuntime = documentRuntime;
		this.#documentPort = documentPort;
		this.#commandPort = commandPort;
	}

	get failureReason(): CoordinatorFailureReason | null {
		return this.#failedReason;
	}

	get committedRevision(): WorkbenchSharedDocumentRevision | null {
		return this.#committedDocument?.revision ?? null;
	}

	async start(
		loaded: InitialDocumentLoadResult,
		initialCommands: readonly TerminalPresentationCommand[] = [],
	): Promise<CoordinatorStartResult> {
		if (this.#disposed || this.#started) {
			return { kind: 'unavailable', reason: 'control-unavailable' };
		}
		this.#started = true;
		this.#pendingCommands.push(...initialCommands);
		this.#unsubscribeDocument = this.#documentRuntime.subscribeDocument(this.#schedule);
		if (loaded.kind === 'available') {
			const projected = this.#documentRuntime.getSharedDocument(loaded.document.revision);
			if (documentFingerprint(projected) !== documentFingerprint(loaded.document)) {
				return this.#fail('projection-mismatch');
			}
			this.#committedDocument = loaded.document;
		} else {
			const initial = this.#documentRuntime.getSharedDocument(workbenchSharedDocumentRevision('0'));
			const committed = await this.#documentPort.commit(null, initial);
			if (this.#disposed) return { kind: 'unavailable', reason: 'control-unavailable' };
			if (committed.kind !== 'committed') return this.#fail(reasonForCommit(committed));
			if (committed.revision !== initial.revision) return this.#fail('revision-sequence');
			this.#committedDocument = initial;
		}
		this.#schedule();
		return { kind: 'ready', revision: this.#committedDocument.revision };
	}

	dispatchCommands(commands: readonly TerminalPresentationCommand[]): void {
		if (this.#disposed || this.#failedReason !== null || commands.length === 0) return;
		this.#pendingCommands.push(...commands);
		this.#schedule();
	}

	flush(): Promise<DesktopWorkbenchDocumentFlushResult> {
		if (this.#disposed) {
			return Promise.resolve({ kind: 'unavailable', reason: 'control-unavailable' });
		}
		if (this.#failedReason !== null) {
			return Promise.resolve({ kind: 'unavailable', reason: this.#failedReason });
		}
		if (!this.#started) {
			return Promise.resolve({ kind: 'unavailable', reason: 'control-unavailable' });
		}
		const result = new Promise<DesktopWorkbenchDocumentFlushResult>((resolve) => {
			this.#flushWaiters.add(resolve);
		});
		this.#schedule();
		this.#settleFlushWaiters();
		return result;
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#pendingCommands.length = 0;
		this.#unsubscribeDocument?.();
		this.#unsubscribeDocument = undefined;
		this.#settleFlushWaiters();
	}

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
		if (
			this.#disposed ||
			this.#failedReason !== null ||
			this.#pumping ||
			this.#committedDocument === undefined
		) {
			return;
		}
		this.#pumping = true;
		try {
			while (!this.#disposed && this.#failedReason === null) {
				const committed = this.#committedDocument;
				const commandCount = this.#pendingCommands.length;
				const candidate = this.#documentRuntime.getSharedDocument(
					nextDocumentRevision(committed.revision),
				);
				if (documentFingerprint(candidate) === documentFingerprint(committed)) {
					if (commandCount === 0) break;
					this.#dispatchPreparedCommands(commandCount);
					continue;
				}
				const result = await this.#documentPort.commit(committed.revision, candidate);
				if (this.#disposed) return;
				if (result.kind !== 'committed') {
					this.#fail(reasonForCommit(result));
					return;
				}
				if (result.revision !== candidate.revision) {
					this.#fail('revision-sequence');
					return;
				}
				this.#committedDocument = candidate;
				this.#dispatchPreparedCommands(commandCount);
			}
		} finally {
			this.#pumping = false;
			if (this.#needsPump()) this.#schedule();
			else this.#settleFlushWaiters();
		}
	}

	#dispatchPreparedCommands(commandCount: number): void {
		if (commandCount === 0) return;
		const commands = this.#pendingCommands.splice(0, commandCount);
		this.#commandPort.dispatch(commands);
	}

	#needsPump(): boolean {
		if (this.#disposed || this.#failedReason !== null || this.#committedDocument === undefined) {
			return false;
		}
		if (this.#pendingCommands.length > 0) return true;
		const projected = this.#documentRuntime.getSharedDocument(this.#committedDocument.revision);
		return documentFingerprint(projected) !== documentFingerprint(this.#committedDocument);
	}

	#fail(
		reason: CoordinatorFailureReason,
	): Extract<CoordinatorStartResult, { kind: 'unavailable' }> {
		this.#failedReason = reason;
		this.#unsubscribeDocument?.();
		this.#unsubscribeDocument = undefined;
		this.#settleFlushWaiters();
		return { kind: 'unavailable', reason };
	}

	#settleFlushWaiters(): void {
		if (this.#flushWaiters.size === 0) return;
		let result: DesktopWorkbenchDocumentFlushResult | null = null;
		if (this.#disposed) {
			result = { kind: 'unavailable', reason: 'control-unavailable' };
		} else if (this.#failedReason !== null) {
			result = { kind: 'unavailable', reason: this.#failedReason };
		} else if (
			this.#started &&
			this.#committedDocument !== undefined &&
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
	revision: WorkbenchSharedDocumentRevision,
): WorkbenchSharedDocumentRevision {
	return workbenchSharedDocumentRevision((BigInt(revision) + 1n).toString());
}

function documentFingerprint(document: WorkbenchSharedDocument): string {
	return JSON.stringify([document.version, document.projectWorkspace, document.terminalContexts]);
}

function reasonForCommit(
	result: Exclude<
		Awaited<ReturnType<WorkbenchDocumentCommitPort['commit']>>,
		{ readonly kind: 'committed' }
	>,
): CoordinatorFailureReason {
	switch (result.kind) {
		case 'conflict':
			return 'conflict';
		case 'rejected':
			return 'revision-sequence';
		case 'unavailable':
			return result.reason;
	}
}
