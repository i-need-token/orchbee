import {
	decodeControlJson,
	MessageKind,
	TerminalExitedSchema,
	TerminalFailedSchema,
	TerminalMetadataChangedSchema,
	type DecodedProtocolEnvelope,
	type HostEpoch,
	type TerminalExited,
	type TerminalFailed,
	type TerminalIdentity,
	type TerminalMetadataChanged,
} from '@orchbee/terminal-protocol';
import type { WorkbenchTerminalMetadata } from '@orchbee/workbench-client-protocol';
import type {
	TerminalPresentationEvent,
	TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';

import { EnvironmentIdSchema, type EnvironmentId } from '../environment';
import type { DesktopSshEnvironmentTerminalRuntimeHandle } from './desktop-ssh-environment-terminal-runtime';

const MAX_PENDING_TERMINAL_OBSERVATIONS = 1_024;
const RETRY_DELAY_MILLISECONDS = 25;

type TerminalOutcomeEvent = Extract<
	TerminalPresentationEvent,
	{ readonly type: 'terminal-exited' | 'terminal-failed' }
>;
export type DesktopSshEnvironmentTerminalExitEvent = Extract<
	TerminalOutcomeEvent,
	{ readonly type: 'terminal-exited' }
>;
type TerminalFailureEvent = Extract<TerminalOutcomeEvent, { readonly type: 'terminal-failed' }>;

interface TerminalExitPendingEntry {
	readonly event: TerminalExited;
	readonly reason: string;
}

interface TerminalExitRejectionEntry {
	readonly event: TerminalExited;
	readonly reason: 'pending-journal-full' | 'write-failed';
}

export interface DesktopSshEnvironmentTerminalExitSpillPort {
	admit(
		event: TerminalExited,
		reason: 'host-admission',
	): { readonly kind: 'admitted' } | { readonly kind: 'full' };
	list(): readonly TerminalExitPendingEntry[];
	listRejected(): readonly TerminalExitRejectionEntry[];
	load(publicationEvidence: {
		hasExact(event: TerminalExited): boolean;
	}): Promise<
		| { readonly kind: 'ready' }
		| { readonly kind: 'unavailable'; readonly reason: 'invalid' | 'read-failed' }
	>;
	reject(
		event: TerminalExited,
		reason: 'pending-journal-full' | 'write-failed',
	): { readonly kind: 'recorded' } | { readonly kind: 'full' };
	remove(event: TerminalExited): void;
}

export interface DesktopSshEnvironmentTerminalExitPublicationPort {
	has(event: TerminalExited): boolean;
	hasExact(event: TerminalExited): boolean;
	list(): readonly { readonly event: TerminalExited }[];
	load(): Promise<
		| { readonly kind: 'ready' }
		| { readonly kind: 'unavailable'; readonly reason: 'invalid' | 'read-failed' }
	>;
	markPublished(event: TerminalExited): boolean;
}

export interface DesktopSshEnvironmentTerminalObservationPublicationPort {
	commitExternalEvent(event: TerminalOutcomeEvent): boolean;
	hasExternalEvent(event: TerminalOutcomeEvent): boolean;
	isCurrentPresentationTarget(
		environmentId: EnvironmentId,
		presentationId: TerminalPresentationId,
	): boolean;
	publishTerminalEvent(event: TerminalPresentationEvent): boolean;
	publishTerminalMetadata(metadata: WorkbenchTerminalMetadata): boolean;
	recordExternalEvent(event: TerminalOutcomeEvent): boolean;
	reserveExternalEvent(event: TerminalOutcomeEvent): boolean;
}

export interface DesktopSshEnvironmentTerminalObservationOwnerOptions {
	readonly createExitEvent: (
		presentationId: TerminalPresentationId,
		event: TerminalExited,
	) => DesktopSshEnvironmentTerminalExitEvent;
	readonly createFailureEvent: (
		presentationId: TerminalPresentationId,
		event: TerminalFailed,
	) => TerminalFailureEvent;
	readonly environmentId: EnvironmentId;
	readonly publication: DesktopSshEnvironmentTerminalObservationPublicationPort;
	readonly publicationLedger: DesktopSshEnvironmentTerminalExitPublicationPort;
	readonly spillStore: DesktopSshEnvironmentTerminalExitSpillPort;
	readonly trace?: ((reason: string) => void) | undefined;
}

export type DesktopSshEnvironmentTerminalObservationInitialization =
	| { readonly kind: 'ready' }
	| {
			readonly kind: 'unavailable';
			readonly reason: 'invalid' | 'read-failed' | 'write-failed';
	  };

export interface DesktopSshEnvironmentTerminalObservationHandle {
	acceptEnvelope(envelope: DecodedProtocolEnvelope, expectedHostEpoch: HostEpoch): void;
	activate(): Promise<void>;
	attachRuntime(runtime: DesktopSshEnvironmentTerminalRuntimeHandle): void;
	commitReconciledExit(
		event: DesktopSshEnvironmentTerminalExitEvent,
		publish: (event: TerminalPresentationEvent) => unknown,
	): boolean;
	dispose(): Promise<void>;
	initialize(): Promise<DesktopSshEnvironmentTerminalObservationInitialization>;
	retryPending(): Promise<void>;
	sealInbound(): void;
}

type ExitAttemptResult = 'completed' | 'retryable' | 'waiting';

export class DesktopSshEnvironmentTerminalObservationOwner implements DesktopSshEnvironmentTerminalObservationHandle {
	#accepting = true;
	#active = false;
	readonly #completedExitRevisions = new Map<string, TerminalExited['finalRevision']>();
	readonly #createExitEvent: DesktopSshEnvironmentTerminalObservationOwnerOptions['createExitEvent'];
	readonly #createFailureEvent: DesktopSshEnvironmentTerminalObservationOwnerOptions['createFailureEvent'];
	#disposed = false;
	readonly #environmentId: EnvironmentId;
	readonly #failureRevisions = new Map<string, TerminalFailed['finalRevision']>();
	#initialized = false;
	readonly #metadataRevisions = new Map<string, TerminalMetadataChanged['revision']>();
	readonly #observedIdentities = new Set<string>();
	readonly #pendingFailures = new Map<string, TerminalFailed>();
	readonly #pendingMetadata = new Map<string, TerminalMetadataChanged>();
	readonly #publication: DesktopSshEnvironmentTerminalObservationPublicationPort;
	readonly #publicationLedger: DesktopSshEnvironmentTerminalExitPublicationPort;
	#retryOperation: Promise<void> | undefined;
	#retryRequested = false;
	#retryTimer: ReturnType<typeof setTimeout> | undefined;
	#runtime: DesktopSshEnvironmentTerminalRuntimeHandle | undefined;
	readonly #spillStore: DesktopSshEnvironmentTerminalExitSpillPort;
	readonly #trace: (reason: string) => void;

	constructor(options: DesktopSshEnvironmentTerminalObservationOwnerOptions) {
		this.#createExitEvent = options.createExitEvent;
		this.#createFailureEvent = options.createFailureEvent;
		this.#environmentId = EnvironmentIdSchema.parse(options.environmentId);
		this.#publication = options.publication;
		this.#publicationLedger = options.publicationLedger;
		this.#spillStore = options.spillStore;
		this.#trace = options.trace ?? (() => undefined);
	}

	async initialize(): Promise<DesktopSshEnvironmentTerminalObservationInitialization> {
		if (this.#disposed) return { kind: 'unavailable', reason: 'read-failed' };
		if (this.#initialized) return { kind: 'ready' };
		const ledger = await this.#publicationLedger.load();
		if (ledger.kind !== 'ready') return ledger;
		const spill = await this.#spillStore.load(this.#publicationLedger);
		if (spill.kind !== 'ready') return spill;
		try {
			this.#promoteRejectedExits();
		} catch {
			return { kind: 'unavailable', reason: 'write-failed' };
		}
		this.#initialized = true;
		return { kind: 'ready' };
	}

	attachRuntime(runtime: DesktopSshEnvironmentTerminalRuntimeHandle): void {
		if (this.#disposed || this.#runtime !== undefined) {
			throw new Error('SSH Environment terminal observation runtime is already settled.');
		}
		this.#runtime = runtime;
		runtime.setIdentityObservedListener?.((identity) => {
			const key = identityKey(identity);
			if (this.#observedIdentities.has(key)) return;
			if (this.#observedIdentities.size >= MAX_PENDING_TERMINAL_OBSERVATIONS) {
				const oldestKey = this.#observedIdentities.values().next().value;
				if (oldestKey !== undefined) this.#observedIdentities.delete(oldestKey);
			}
			this.#observedIdentities.add(key);
			this.#requestRetry();
		});
	}

	activate(): Promise<void> {
		if (!this.#initialized || this.#runtime === undefined || this.#disposed) {
			return Promise.reject(
				new Error('SSH Environment terminal observation owner cannot be activated.'),
			);
		}
		this.#active = true;
		return this.retryPending();
	}

	acceptEnvelope(envelope: DecodedProtocolEnvelope, expectedHostEpoch: HostEpoch): void {
		if (!this.#accepting || this.#disposed) return;
		if (envelope.hostEpoch !== expectedHostEpoch) {
			throw new Error('SSH Environment Terminal Host envelope crossed its Host epoch fence.');
		}
		if (envelope.kind === MessageKind.terminalExited) {
			const decoded = decodeControlJson(envelope.payload, TerminalExitedSchema);
			if (!decoded.ok) throw new Error(decoded.detail);
			this.#requireExpectedIdentityEpoch(decoded.value.identity, expectedHostEpoch);
			this.#admitExit(decoded.value);
			this.#requestRetry();
			return;
		}
		if (envelope.kind === MessageKind.terminalMetadataChanged) {
			const decoded = decodeControlJson(envelope.payload, TerminalMetadataChangedSchema);
			if (!decoded.ok) throw new Error(decoded.detail);
			this.#requireExpectedIdentityEpoch(decoded.value.identity, expectedHostEpoch);
			this.#rememberMetadata(decoded.value);
			this.#requestRetry();
			return;
		}
		if (envelope.kind !== MessageKind.terminalFailed) return;
		const decoded = decodeControlJson(envelope.payload, TerminalFailedSchema);
		if (!decoded.ok) throw new Error(decoded.detail);
		this.#requireExpectedIdentityEpoch(decoded.value.identity, expectedHostEpoch);
		this.#rememberFailure(decoded.value);
		this.#requestRetry();
	}

	sealInbound(): void {
		this.#accepting = false;
	}

	commitReconciledExit(
		event: DesktopSshEnvironmentTerminalExitEvent,
		publish: (event: TerminalPresentationEvent) => unknown,
	): boolean {
		const runtime = this.#runtime;
		if (!this.#active || runtime === undefined || this.#disposed) return false;
		const terminalExit = runtime.getDurableTerminalExit(event.presentationId);
		if (terminalExit === null) return false;
		const result = this.#publishExit(runtime, terminalExit, event.presentationId, publish);
		if (result !== 'completed') {
			if (result === 'retryable') this.#scheduleRetry();
			this.#requestRetry();
			return false;
		}
		return true;
	}

	retryPending(): Promise<void> {
		if (!this.#active || this.#disposed) return Promise.resolve();
		this.#retryRequested = true;
		const existing = this.#retryOperation;
		if (existing !== undefined) return existing;
		const operation = Promise.resolve().then(async () => {
			while (this.#retryRequested && this.#active && !this.#disposed) {
				this.#retryRequested = false;
				const retryable = await this.#processPending();
				if (retryable) this.#scheduleRetry();
			}
		});
		this.#retryOperation = operation;
		const settle = () => {
			if (this.#retryOperation === operation) this.#retryOperation = undefined;
			if (this.#retryRequested && this.#active && !this.#disposed) {
				this.#requestRetry();
			}
		};
		void operation.then(settle, settle);
		return operation;
	}

	async dispose(): Promise<void> {
		if (this.#disposed) return;
		this.sealInbound();
		if (this.#retryTimer !== undefined) {
			clearTimeout(this.#retryTimer);
			this.#retryTimer = undefined;
		}
		if (this.#active) await this.retryPending();
		await this.#retryOperation;
		this.#disposed = true;
		this.#active = false;
		this.#runtime?.setIdentityObservedListener?.(() => undefined);
	}

	#admitExit(event: TerminalExited): void {
		if (!this.#initialized) {
			throw new Error('SSH Environment terminal exit journal is not ready.');
		}
		try {
			const admission = this.#spillStore.admit(event, 'host-admission');
			if (admission.kind === 'admitted') return;
			const rejection = this.#spillStore.reject(event, 'pending-journal-full');
			if (rejection.kind === 'recorded') {
				this.#trace('remote-terminal-exit-rejected-pending-journal-full');
				return;
			}
			throw new Error('SSH Environment terminal exit rejection journal is full.');
		} catch (error: unknown) {
			try {
				const rejection = this.#spillStore.reject(event, 'write-failed');
				if (rejection.kind === 'recorded') {
					this.#trace('remote-terminal-exit-rejected-write-failed');
					return;
				}
			} catch (rejectionError: unknown) {
				throw new AggregateError(
					[error, rejectionError],
					'SSH Environment terminal exit journal failed closed.',
				);
			}
			throw error;
		}
	}

	async #processPending(): Promise<boolean> {
		const runtime = this.#runtime;
		if (runtime === undefined) return false;
		let retryable = this.#publishPendingMetadata(runtime);
		if (await this.#publishPendingFailures(runtime)) retryable = true;
		let candidates: readonly TerminalExited[];
		try {
			candidates = this.#exitCandidates();
		} catch {
			this.#trace('remote-terminal-exit-recovery-read-failed');
			return true;
		}
		for (const event of candidates) {
			const result = await this.#attemptExit(runtime, event);
			if (result === 'retryable') retryable = true;
		}
		return retryable;
	}

	#publishPendingMetadata(runtime: DesktopSshEnvironmentTerminalRuntimeHandle): boolean {
		let retryable = false;
		for (const [key, event] of this.#pendingMetadata) {
			const presentationId = runtime.findPresentationByIdentity(event.identity);
			if (presentationId === null || !this.#isCurrentPresentationTarget(presentationId)) {
				continue;
			}
			try {
				if (
					!this.#publication.publishTerminalMetadata({
						currentDirectory: event.currentDirectory,
						identity: event.identity,
						presentationId,
						revision: event.revision,
						title: event.title,
					})
				) {
					retryable = true;
					continue;
				}
			} catch {
				this.#trace('remote-terminal-metadata-publication-failed');
				retryable = true;
				continue;
			}
			if (this.#pendingMetadata.get(key) === event) this.#pendingMetadata.delete(key);
		}
		return retryable;
	}

	async #publishPendingFailures(
		runtime: DesktopSshEnvironmentTerminalRuntimeHandle,
	): Promise<boolean> {
		let retryable = false;
		for (const [key, event] of this.#pendingFailures) {
			let presentationId: TerminalPresentationId | null;
			try {
				presentationId = await runtime.recordFailure(event);
			} catch {
				this.#trace('remote-terminal-failure-association-failed');
				retryable = true;
				continue;
			}
			if (presentationId === null || !this.#isCurrentPresentationTarget(presentationId)) {
				continue;
			}
			const documentEvent = this.#createFailureEvent(presentationId, event);
			let recorded = false;
			try {
				if (this.#publication.hasExternalEvent(documentEvent)) {
					if (this.#pendingFailures.get(key) === event) this.#pendingFailures.delete(key);
					continue;
				}
				if (!this.#publication.recordExternalEvent(documentEvent)) {
					continue;
				}
				recorded = true;
				this.#publication.publishTerminalEvent(documentEvent);
			} catch {
				this.#trace('remote-terminal-failure-publication-failed');
				if (!recorded) {
					retryable = true;
					continue;
				}
			}
			if (this.#pendingFailures.get(key) === event) this.#pendingFailures.delete(key);
		}
		return retryable;
	}

	#exitCandidates(): readonly TerminalExited[] {
		const candidates = new Map<string, TerminalExited>();
		for (const { event } of [...this.#spillStore.list(), ...this.#publicationLedger.list()]) {
			const key = identityKey(event.identity);
			const completedRevision = this.#completedExitRevisions.get(key);
			if (
				completedRevision !== undefined &&
				BigInt(completedRevision) >= BigInt(event.finalRevision)
			) {
				continue;
			}
			const existing = candidates.get(key);
			if (existing === undefined || BigInt(event.finalRevision) > BigInt(existing.finalRevision)) {
				candidates.set(key, event);
			}
		}
		return [...candidates.values()];
	}

	async #attemptExit(
		runtime: DesktopSshEnvironmentTerminalRuntimeHandle,
		event: TerminalExited,
	): Promise<ExitAttemptResult> {
		let outcome: Awaited<
			ReturnType<DesktopSshEnvironmentTerminalRuntimeHandle['recordExitOutcome']>
		>;
		try {
			outcome = await runtime.recordExitOutcome(event);
		} catch {
			this.#trace('remote-terminal-exit-persistence-failed');
			return 'retryable';
		}
		if (outcome.kind === 'retryable') {
			return outcome.reason === 'association' ? 'waiting' : 'retryable';
		}
		if (outcome.kind === 'unattributed') return 'waiting';
		let presentationId: TerminalPresentationId | null = null;
		if (
			outcome.kind === 'committed' ||
			outcome.kind === 'durable' ||
			outcome.kind === 'published'
		) {
			presentationId = outcome.presentationId;
		} else if (this.#publicationLedger.has(event)) {
			presentationId = runtime.findPresentationByIdentity(event.identity);
		}
		if (presentationId === null) return 'waiting';
		return this.#publishExit(runtime, event, presentationId, (documentEvent) =>
			this.#publication.publishTerminalEvent(documentEvent),
		);
	}

	#publishExit(
		runtime: DesktopSshEnvironmentTerminalRuntimeHandle,
		event: TerminalExited,
		presentationId: TerminalPresentationId,
		publish: (event: TerminalPresentationEvent) => unknown,
	): ExitAttemptResult {
		if (!this.#isCurrentPresentationTarget(presentationId)) return 'waiting';
		const documentEvent = this.#createExitEvent(presentationId, event);
		let alreadyRecorded: boolean;
		let ledgerCommitted: boolean;
		try {
			alreadyRecorded = this.#publication.hasExternalEvent(documentEvent);
			if (!alreadyRecorded && !this.#publication.reserveExternalEvent(documentEvent)) {
				return 'waiting';
			}
			ledgerCommitted = this.#publicationLedger.has(event);
		} catch {
			this.#trace('remote-terminal-exit-publication-reservation-failed');
			return 'retryable';
		}
		if (!ledgerCommitted) {
			let runtimeCommitted: boolean;
			try {
				runtimeCommitted = runtime.commitExitPublication?.(event) ?? true;
			} catch {
				this.#trace('remote-terminal-exit-runtime-publication-commit-failed');
				return 'retryable';
			}
			if (!runtimeCommitted) return 'retryable';
			try {
				if (!this.#publicationLedger.markPublished(event)) return 'retryable';
			} catch {
				this.#trace('remote-terminal-exit-publication-ledger-failed');
				return 'retryable';
			}
		}
		if (!alreadyRecorded) {
			try {
				if (!this.#publication.commitExternalEvent(documentEvent)) return 'waiting';
			} catch {
				this.#trace('remote-terminal-exit-publication-commit-failed');
				return 'retryable';
			}
		}
		try {
			if (publish(documentEvent) === false) return 'retryable';
		} catch {
			this.#trace('remote-terminal-exit-client-publication-failed');
			return 'retryable';
		}
		try {
			this.#spillStore.remove(event);
		} catch {
			this.#trace('remote-terminal-exit-journal-compaction-failed');
			return 'retryable';
		}
		runtime.releasePendingExit?.(event);
		this.#completedExitRevisions.set(identityKey(event.identity), event.finalRevision);
		try {
			if (this.#promoteRejectedExits() > 0) this.#retryRequested = true;
		} catch {
			this.#trace('remote-terminal-exit-rejection-promotion-failed');
			return 'retryable';
		}
		return 'completed';
	}

	#promoteRejectedExits(): number {
		let promoted = 0;
		for (const { event } of this.#spillStore.listRejected()) {
			if (this.#publicationLedger.has(event)) continue;
			const admission = this.#spillStore.admit(event, 'host-admission');
			if (admission.kind === 'full') break;
			promoted += 1;
		}
		return promoted;
	}

	#rememberMetadata(event: TerminalMetadataChanged): void {
		const key = identityKey(event.identity);
		const existingRevision = this.#metadataRevisions.get(key);
		if (existingRevision !== undefined && BigInt(existingRevision) >= BigInt(event.revision)) {
			return;
		}
		this.#evictOldestObservationIfFull(this.#metadataRevisions, this.#pendingMetadata, key);
		this.#metadataRevisions.delete(key);
		this.#metadataRevisions.set(key, event.revision);
		this.#pendingMetadata.delete(key);
		this.#pendingMetadata.set(key, event);
	}

	#rememberFailure(event: TerminalFailed): void {
		const key = identityKey(event.identity);
		const existingRevision = this.#failureRevisions.get(key);
		if (existingRevision !== undefined && BigInt(existingRevision) >= BigInt(event.finalRevision)) {
			return;
		}
		this.#evictOldestObservationIfFull(this.#failureRevisions, this.#pendingFailures, key);
		this.#failureRevisions.delete(key);
		this.#failureRevisions.set(key, event.finalRevision);
		this.#pendingFailures.delete(key);
		this.#pendingFailures.set(key, event);
	}

	#evictOldestObservationIfFull<Revision, Value>(
		revisions: Map<string, Revision>,
		pending: Map<string, Value>,
		incomingKey: string,
	): void {
		if (revisions.has(incomingKey) || revisions.size < MAX_PENDING_TERMINAL_OBSERVATIONS) return;
		const oldestKey = revisions.keys().next().value;
		if (oldestKey === undefined) return;
		revisions.delete(oldestKey);
		pending.delete(oldestKey);
	}

	#isCurrentPresentationTarget(presentationId: TerminalPresentationId): boolean {
		try {
			return this.#publication.isCurrentPresentationTarget(this.#environmentId, presentationId);
		} catch {
			return false;
		}
	}

	#requireExpectedIdentityEpoch(identity: TerminalIdentity, expectedHostEpoch: HostEpoch): void {
		if (identity.hostEpoch !== expectedHostEpoch) {
			throw new Error('SSH Environment terminal observation crossed its identity epoch fence.');
		}
	}

	#requestRetry(): void {
		void this.retryPending().catch(() => {
			this.#trace('remote-terminal-observation-retry-failed');
		});
	}

	#scheduleRetry(): void {
		if (!this.#accepting || !this.#active || this.#disposed || this.#retryTimer !== undefined) {
			return;
		}
		const timer = setTimeout(() => {
			this.#retryTimer = undefined;
			this.#requestRetry();
		}, RETRY_DELAY_MILLISECONDS);
		timer.unref?.();
		this.#retryTimer = timer;
	}
}

function identityKey(identity: TerminalIdentity): string {
	return `${identity.hostEpoch}:${identity.creationId}:${identity.terminalId}`;
}
