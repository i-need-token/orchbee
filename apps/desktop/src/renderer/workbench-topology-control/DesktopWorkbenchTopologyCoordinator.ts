import type {
	TerminalPresentationCommand,
	WorkbenchProjectWorkspaceRuntime,
} from '@orchbee/workbench';
import {
	workbenchTopologyRevision,
	type WorkbenchTopologyDocument,
	type WorkbenchTopologyRevision,
} from '@orchbee/workbench/topology';

import type { WorkbenchTopologyCommitResult, WorkbenchTopologyLoadResult } from '../../shared';

type InitialTopologyLoadResult = Extract<
	WorkbenchTopologyLoadResult,
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
	| { readonly kind: 'ready'; readonly revision: WorkbenchTopologyRevision }
	| { readonly kind: 'unavailable'; readonly reason: CoordinatorFailureReason };

export type DesktopWorkbenchTopologyFlushResult =
	| { readonly kind: 'flushed'; readonly revision: WorkbenchTopologyRevision }
	| { readonly kind: 'unavailable'; readonly reason: CoordinatorFailureReason };

interface WorkbenchTopologyCommitPort {
	commit(
		expectedRevision: WorkbenchTopologyRevision | null,
		document: WorkbenchTopologyDocument,
	): Promise<
		| WorkbenchTopologyCommitResult
		| { readonly kind: 'unavailable'; readonly reason: 'control-unavailable' }
	>;
}

interface TerminalCommandPort {
	dispatch(commands: readonly TerminalPresentationCommand[]): void;
}

type TopologyRuntime = Pick<
	WorkbenchProjectWorkspaceRuntime,
	'getTopologyDocument' | 'subscribeTopology'
>;

export class DesktopWorkbenchTopologyCoordinator {
	readonly #commandPort: TerminalCommandPort;
	#committedDocument: WorkbenchTopologyDocument | undefined;
	#disposed = false;
	#failedReason: CoordinatorFailureReason | null = null;
	readonly #flushWaiters = new Set<(result: DesktopWorkbenchTopologyFlushResult) => void>();
	readonly #pendingCommands: TerminalPresentationCommand[] = [];
	#pumping = false;
	#scheduled = false;
	#started = false;
	readonly #topologyPort: WorkbenchTopologyCommitPort;
	readonly #topologyRuntime: TopologyRuntime;
	#unsubscribeTopology: (() => void) | undefined;

	constructor(
		topologyRuntime: TopologyRuntime,
		topologyPort: WorkbenchTopologyCommitPort,
		commandPort: TerminalCommandPort,
	) {
		this.#topologyRuntime = topologyRuntime;
		this.#topologyPort = topologyPort;
		this.#commandPort = commandPort;
	}

	get failureReason(): CoordinatorFailureReason | null {
		return this.#failedReason;
	}

	get committedRevision(): WorkbenchTopologyRevision | null {
		return this.#committedDocument?.revision ?? null;
	}

	async start(
		loaded: InitialTopologyLoadResult,
		initialCommands: readonly TerminalPresentationCommand[] = [],
	): Promise<CoordinatorStartResult> {
		if (this.#disposed || this.#started) {
			return { kind: 'unavailable', reason: 'control-unavailable' };
		}
		this.#started = true;
		this.#pendingCommands.push(...initialCommands);
		this.#unsubscribeTopology = this.#topologyRuntime.subscribeTopology(this.#schedule);
		if (loaded.kind === 'available') {
			const projected = this.#topologyRuntime.getTopologyDocument(loaded.document.revision);
			if (topologyFingerprint(projected) !== topologyFingerprint(loaded.document)) {
				return this.#fail('projection-mismatch');
			}
			this.#committedDocument = loaded.document;
		} else {
			const initial = this.#topologyRuntime.getTopologyDocument(workbenchTopologyRevision('0'));
			const committed = await this.#topologyPort.commit(null, initial);
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

	flush(): Promise<DesktopWorkbenchTopologyFlushResult> {
		if (this.#disposed) {
			return Promise.resolve({ kind: 'unavailable', reason: 'control-unavailable' });
		}
		if (this.#failedReason !== null) {
			return Promise.resolve({ kind: 'unavailable', reason: this.#failedReason });
		}
		if (!this.#started) {
			return Promise.resolve({ kind: 'unavailable', reason: 'control-unavailable' });
		}
		const result = new Promise<DesktopWorkbenchTopologyFlushResult>((resolve) => {
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
		this.#unsubscribeTopology?.();
		this.#unsubscribeTopology = undefined;
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
				const candidate = this.#topologyRuntime.getTopologyDocument(
					nextTopologyRevision(committed.revision),
				);
				if (topologyFingerprint(candidate) === topologyFingerprint(committed)) {
					if (commandCount === 0) break;
					this.#dispatchPreparedCommands(commandCount);
					continue;
				}
				const result = await this.#topologyPort.commit(committed.revision, candidate);
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
		const projected = this.#topologyRuntime.getTopologyDocument(this.#committedDocument.revision);
		return topologyFingerprint(projected) !== topologyFingerprint(this.#committedDocument);
	}

	#fail(
		reason: CoordinatorFailureReason,
	): Extract<CoordinatorStartResult, { kind: 'unavailable' }> {
		this.#failedReason = reason;
		this.#unsubscribeTopology?.();
		this.#unsubscribeTopology = undefined;
		this.#settleFlushWaiters();
		return { kind: 'unavailable', reason };
	}

	#settleFlushWaiters(): void {
		if (this.#flushWaiters.size === 0) return;
		let result: DesktopWorkbenchTopologyFlushResult | null = null;
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

function nextTopologyRevision(revision: WorkbenchTopologyRevision): WorkbenchTopologyRevision {
	return workbenchTopologyRevision((BigInt(revision) + 1n).toString());
}

function topologyFingerprint(document: WorkbenchTopologyDocument): string {
	return JSON.stringify([document.version, document.projectWorkspace, document.terminalContexts]);
}

function reasonForCommit(
	result: Exclude<
		Awaited<ReturnType<WorkbenchTopologyCommitPort['commit']>>,
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
