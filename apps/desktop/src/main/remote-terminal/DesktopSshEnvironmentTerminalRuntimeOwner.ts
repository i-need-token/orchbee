import type { TerminalExited, TerminalFailed, TerminalGeometry } from '@orchbee/terminal-protocol';
import type {
	TerminalPresentationCommand,
	TerminalPresentationEvent,
	TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';

import type { DesktopTerminalExecutionResult } from '../client-session';
import { EnvironmentIdSchema, type EnvironmentId } from '../environment';
import type { HostConnection } from '../host';
import type { SshTerminalHostConnectionOutcome } from '../ssh-environment';
import type {
	DesktopTerminalProjectionLease,
	DesktopTerminalProjectionOpenOptions,
} from '../terminal-projection-control';
import type {
	DesktopSshEnvironmentTerminalExitEvent,
	DesktopSshEnvironmentTerminalObservationHandle,
} from './DesktopSshEnvironmentTerminalObservationOwner';
import type { DesktopSshEnvironmentTerminalRuntimeHandle } from './desktop-ssh-environment-terminal-runtime';

interface DesktopSshEnvironmentTerminalRuntimeConnectionPort {
	openTerminalHost(environmentId: EnvironmentId): Promise<SshTerminalHostConnectionOutcome>;
}

export interface DesktopSshEnvironmentTerminalRuntimeOwnerOptions {
	readonly connections: DesktopSshEnvironmentTerminalRuntimeConnectionPort;
	readonly createObservation: (
		environmentId: EnvironmentId,
	) => DesktopSshEnvironmentTerminalObservationHandle;
	readonly createRuntime: (
		environmentId: EnvironmentId,
		connection: HostConnection,
	) => DesktopSshEnvironmentTerminalRuntimeHandle;
}

export type DesktopSshEnvironmentTerminalRuntimePrepareResult =
	| { readonly activation: 'existing' | 'started'; readonly kind: 'ready' }
	| {
			readonly kind: 'failed';
			readonly reason: 'busy' | 'compatibility-unsupported' | 'unavailable';
	  };

interface RuntimeRecord {
	accepting: boolean;
	available: boolean;
	readonly connection: HostConnection;
	readonly environmentId: EnvironmentId;
	removeClosedListener: () => void;
	removeEnvelopeListener: () => void;
	readonly observation: DesktopSshEnvironmentTerminalObservationHandle;
	readonly runtime: DesktopSshEnvironmentTerminalRuntimeHandle;
}

export class DesktopSshEnvironmentTerminalRuntimeOwner {
	readonly #connections: DesktopSshEnvironmentTerminalRuntimeConnectionPort;
	readonly #createObservation: DesktopSshEnvironmentTerminalRuntimeOwnerOptions['createObservation'];
	readonly #createRuntime: DesktopSshEnvironmentTerminalRuntimeOwnerOptions['createRuntime'];
	#disposed = false;
	readonly #operationTails = new Map<EnvironmentId, Promise<void>>();
	readonly #records = new Map<EnvironmentId, RuntimeRecord>();

	constructor(options: DesktopSshEnvironmentTerminalRuntimeOwnerOptions) {
		this.#connections = options.connections;
		this.#createObservation = options.createObservation;
		this.#createRuntime = options.createRuntime;
	}

	prepare(
		environmentId: EnvironmentId,
	): Promise<DesktopSshEnvironmentTerminalRuntimePrepareResult> {
		const canonicalId = EnvironmentIdSchema.parse(environmentId);
		return this.#enqueue(canonicalId, async () => {
			if (this.#disposed) return { kind: 'failed', reason: 'unavailable' };
			if (this.#records.get(canonicalId)?.available === true) {
				return { activation: 'existing', kind: 'ready' };
			}
			let observation: DesktopSshEnvironmentTerminalObservationHandle;
			try {
				observation = this.#createObservation(canonicalId);
			} catch {
				return { kind: 'failed', reason: 'unavailable' };
			}
			try {
				const initialized = await observation.initialize();
				if (initialized.kind !== 'ready') {
					await disposeObservationDuringPreparation(observation);
					return { kind: 'failed', reason: 'unavailable' };
				}
			} catch {
				await disposeObservationDuringPreparation(observation);
				return { kind: 'failed', reason: 'unavailable' };
			}
			let opened: SshTerminalHostConnectionOutcome;
			try {
				opened = await this.#connections.openTerminalHost(canonicalId);
			} catch {
				await disposeObservationDuringPreparation(observation);
				return { kind: 'failed', reason: 'unavailable' };
			}
			if (opened.kind === 'failed') {
				await disposeObservationDuringPreparation(observation);
				return opened;
			}
			if (this.#disposed) {
				await closeConnectionAndDisposeObservation(opened.connection, observation);
				return { kind: 'failed', reason: 'unavailable' };
			}
			let runtime: DesktopSshEnvironmentTerminalRuntimeHandle;
			try {
				runtime = this.#createRuntime(canonicalId, opened.connection);
			} catch {
				await closeConnectionAndDisposeObservation(opened.connection, observation);
				return { kind: 'failed', reason: 'unavailable' };
			}
			let connectionClosed = false;
			const record: RuntimeRecord = {
				accepting: true,
				available: false,
				connection: opened.connection,
				environmentId: canonicalId,
				removeClosedListener: () => undefined,
				removeEnvelopeListener: () => undefined,
				observation,
				runtime,
			};
			this.#records.set(canonicalId, record);
			try {
				observation.attachRuntime(runtime);
			} catch {
				this.#records.delete(canonicalId);
				await disposeRecordDuringPreparation(record, true);
				return { kind: 'failed', reason: 'unavailable' };
			}
			try {
				record.removeClosedListener = opened.connection.onClosed(() => {
					connectionClosed = true;
					record.accepting = false;
					record.available = false;
					record.observation.sealInbound();
					void this.#retireRecord(record).catch(() => undefined);
				});
				const expectedHostEpoch = opened.connection.hostEpoch;
				record.removeEnvelopeListener = opened.connection.onEnvelope((envelope) => {
					if (!record.accepting || this.#records.get(canonicalId) !== record || this.#disposed) {
						return;
					}
					record.observation.acceptEnvelope(envelope, expectedHostEpoch);
				});
				const initialized = await runtime.initialize();
				if (initialized.kind !== 'ready') {
					if (this.#records.get(canonicalId) === record) this.#records.delete(canonicalId);
					await disposeRecordDuringPreparation(record, !connectionClosed);
					return { kind: 'failed', reason: 'unavailable' };
				}
				await observation.activate();
				if (connectionClosed || this.#disposed) {
					if (this.#records.get(canonicalId) === record) this.#records.delete(canonicalId);
					await disposeRecord(record, !connectionClosed);
					return { kind: 'failed', reason: 'unavailable' };
				}
			} catch {
				if (this.#records.get(canonicalId) === record) this.#records.delete(canonicalId);
				await disposeRecordDuringPreparation(record, !connectionClosed);
				return { kind: 'failed', reason: 'unavailable' };
			}
			record.available = true;
			return { activation: 'started', kind: 'ready' };
		});
	}

	async execute(
		environmentId: EnvironmentId,
		command: TerminalPresentationCommand,
	): Promise<DesktopTerminalExecutionResult> {
		const record = this.#records.get(EnvironmentIdSchema.parse(environmentId));
		if (record === undefined || !record.available || this.#disposed) return { kind: 'deferred' };
		try {
			const event = await record.runtime.execute(command);
			if ('failure' in event && event.failure.retryable) return { kind: 'deferred' };
			return { event, kind: 'settled' };
		} catch {
			return { kind: 'deferred' };
		}
	}

	getDurableTerminalExit(
		environmentId: EnvironmentId,
		presentationId: TerminalPresentationId,
	): TerminalExited | null {
		const record = this.#records.get(EnvironmentIdSchema.parse(environmentId));
		return record?.available === true
			? record.runtime.getDurableTerminalExit(presentationId)
			: null;
	}

	getTerminalFailure(
		environmentId: EnvironmentId,
		presentationId: TerminalPresentationId,
	): TerminalFailed | null {
		const record = this.#records.get(EnvironmentIdSchema.parse(environmentId));
		return record?.available === true ? record.runtime.getTerminalFailure(presentationId) : null;
	}

	commitReconciledExit(
		environmentId: EnvironmentId,
		event: DesktopSshEnvironmentTerminalExitEvent,
		publish: (event: TerminalPresentationEvent) => unknown,
	): boolean {
		const record = this.#records.get(EnvironmentIdSchema.parse(environmentId));
		return record?.available === true
			? record.observation.commitReconciledExit(event, publish)
			: false;
	}

	retryPending(environmentId: EnvironmentId): Promise<void> {
		const record = this.#records.get(EnvironmentIdSchema.parse(environmentId));
		return record?.available === true ? record.observation.retryPending() : Promise.resolve();
	}

	async open(
		environmentId: EnvironmentId,
		presentationId: TerminalPresentationId,
		geometry: TerminalGeometry,
		options: DesktopTerminalProjectionOpenOptions = { brokerEffectTarget: 'desktop' },
	): Promise<DesktopTerminalProjectionLease> {
		const canonicalId = EnvironmentIdSchema.parse(environmentId);
		const record = this.#records.get(canonicalId);
		if (record === undefined || !record.available || this.#disposed) {
			throw projectionFailure('terminal-projection-unavailable', true);
		}
		const lease = await record.runtime.open(presentationId, geometry, {
			...options,
			allowArchivedFallback: false,
		});
		if (!this.#disposed && record.available && this.#records.get(canonicalId) === record) {
			return lease;
		}
		try {
			await lease.dispose();
		} catch {
			throw projectionFailure('terminal-projection-cleanup-failed', true);
		}
		throw projectionFailure('terminal-projection-unavailable', true);
	}

	retire(environmentId: EnvironmentId): Promise<void> {
		const canonicalId = EnvironmentIdSchema.parse(environmentId);
		return this.#enqueue(canonicalId, async () => {
			const record = this.#records.get(canonicalId);
			if (record === undefined) return;
			record.accepting = false;
			record.available = false;
			record.observation.sealInbound();
			this.#records.delete(canonicalId);
			await disposeRecord(record, true);
		});
	}

	async dispose(): Promise<void> {
		if (this.#disposed) return;
		this.#disposed = true;
		await Promise.all(this.#operationTails.values());
		const records = [...this.#records.values()];
		for (const record of records) {
			record.accepting = false;
			record.available = false;
			record.observation.sealInbound();
		}
		this.#records.clear();
		const settlements = await Promise.allSettled(
			records.map(async (record) => disposeRecord(record, true)),
		);
		const failures = settlements.flatMap((settlement) =>
			settlement.status === 'rejected' ? [settlement.reason] : [],
		);
		if (failures.length > 0) {
			throw new AggregateError(
				failures,
				'SSH Environment terminal runtime cleanup did not converge.',
			);
		}
	}

	#retireRecord(record: RuntimeRecord): Promise<void> {
		return this.#enqueue(record.environmentId, async () => {
			if (this.#records.get(record.environmentId) !== record) return;
			this.#records.delete(record.environmentId);
			await disposeRecord(record, false);
		});
	}

	#enqueue<Value>(environmentId: EnvironmentId, operation: () => Promise<Value>): Promise<Value> {
		const previous = this.#operationTails.get(environmentId) ?? Promise.resolve();
		const result = previous.then(operation);
		const tail = result.then(
			() => undefined,
			() => undefined,
		);
		this.#operationTails.set(environmentId, tail);
		void tail.then(() => {
			if (this.#operationTails.get(environmentId) === tail) {
				this.#operationTails.delete(environmentId);
			}
		});
		return result;
	}
}

async function closeConnectionAndDisposeObservation(
	connection: HostConnection,
	observation: DesktopSshEnvironmentTerminalObservationHandle,
): Promise<void> {
	try {
		connection.close();
	} catch {
		// Preparation still has to release the observation owner.
	}
	await disposeObservationDuringPreparation(observation);
}

async function disposeObservationDuringPreparation(
	observation: DesktopSshEnvironmentTerminalObservationHandle,
): Promise<void> {
	try {
		await observation.dispose();
	} catch {
		// Preparation has no live record through which to surface cleanup diagnostics.
	}
}

async function disposeRecordDuringPreparation(
	record: RuntimeRecord,
	closeConnection: boolean,
): Promise<void> {
	try {
		await disposeRecord(record, closeConnection);
	} catch {
		// A failed preparation remains unavailable even when compensation also fails.
	}
}

async function disposeRecord(record: RuntimeRecord, closeConnection: boolean): Promise<void> {
	record.accepting = false;
	record.available = false;
	record.removeClosedListener();
	record.removeClosedListener = () => undefined;
	record.removeEnvelopeListener();
	record.removeEnvelopeListener = () => undefined;
	record.observation.sealInbound();
	let observationFailure: unknown;
	try {
		await record.observation.dispose();
	} catch (error: unknown) {
		observationFailure = error;
	}
	let runtimeFailure: unknown;
	try {
		await record.runtime.dispose();
	} catch (error: unknown) {
		runtimeFailure = error;
	} finally {
		if (closeConnection) record.connection.close();
	}
	if (observationFailure !== undefined && runtimeFailure !== undefined) {
		throw new AggregateError(
			[observationFailure, runtimeFailure],
			'SSH Environment terminal runtime and observation cleanup failed.',
		);
	}
	if (observationFailure !== undefined) throw observationFailure;
	if (runtimeFailure !== undefined) throw runtimeFailure;
}

function projectionFailure(code: string, retryable: boolean): Error {
	return new Error(code, { cause: { code, retryable } });
}
