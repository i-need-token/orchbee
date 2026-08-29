import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	CURRENT_PROTOCOL_VERSION,
	HostEpochSchema,
	MessageKind,
	PayloadEncoding,
	TerminalIdSchema,
	type DecodedProtocolEnvelope,
	type TerminalExited,
	type TerminalFailed,
	type TerminalMetadataChanged,
} from '@orchbee/terminal-protocol';
import {
	terminalPresentationId,
	type TerminalPresentationEvent,
	type TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import { EnvironmentIdSchema, type EnvironmentId } from '../../environment';
import { TerminalExitPublicationLedger, TerminalExitSpillStore } from '../../recovery';
import {
	DesktopSshEnvironmentTerminalObservationOwner,
	type DesktopSshEnvironmentTerminalExitPublicationPort,
	type DesktopSshEnvironmentTerminalExitSpillPort,
	type DesktopSshEnvironmentTerminalObservationPublicationPort,
} from '../DesktopSshEnvironmentTerminalObservationOwner';
import type { DesktopSshEnvironmentTerminalRuntimeHandle } from '../desktop-ssh-environment-terminal-runtime';

const firstEnvironmentId = EnvironmentIdSchema.parse(`environment:${'1'.repeat(32)}`);
const secondEnvironmentId = EnvironmentIdSchema.parse(`environment:${'2'.repeat(32)}`);
const hostEpoch = HostEpochSchema.parse('a'.repeat(32));
const identity = {
	creationId: CreationIdSchema.parse('b'.repeat(32)),
	hostEpoch,
	terminalId: TerminalIdSchema.parse('c'.repeat(32)),
};
const firstPresentationId = terminalPresentationId('remote-presentation-a');
const secondPresentationId = terminalPresentationId('remote-presentation-b');

describe('DesktopSshEnvironmentTerminalObservationOwner', () => {
	it('journals before listener return and preserves the durable publication order', async () => {
		const order: string[] = [];
		const recovery = recoveryState(order);
		const publication = publicationPort(firstEnvironmentId, firstPresentationId, order);
		const runtime = runtimeHandle(firstPresentationId, order);
		const event = terminalExit('7');
		const owner = observationOwner({
			environmentId: firstEnvironmentId,
			publication: publication.port,
			recovery,
		});

		await expect(owner.initialize()).resolves.toEqual({ kind: 'ready' });
		owner.attachRuntime(runtime.handle);
		owner.acceptEnvelope(envelope(MessageKind.terminalExited, event), hostEpoch);

		expect(order).toEqual(['journal-admit']);
		expect(runtime.recordExitOutcome).not.toHaveBeenCalled();
		await owner.activate();

		expect(order).toEqual([
			'journal-admit',
			'binding-outcome',
			'reconciler-reserve',
			'runtime-publication-commit',
			'publication-ledger',
			'reconciler-commit',
			'client-broadcast',
			'journal-compact',
		]);
		expect(recovery.pending).toHaveLength(0);
		expect(recovery.published).toEqual([event]);
		await owner.dispose();
	});

	it('retries a failed Client publication from durable evidence without repeating durable commits', async () => {
		const order: string[] = [];
		const recovery = recoveryState(order);
		let clientPublicationAvailable = false;
		const publication = publicationPort(
			firstEnvironmentId,
			firstPresentationId,
			order,
			() => true,
			() => clientPublicationAvailable,
		);
		const runtime = runtimeHandle(firstPresentationId, order);
		const event = terminalExit('11');
		const owner = observationOwner({
			environmentId: firstEnvironmentId,
			publication: publication.port,
			recovery,
		});

		await owner.initialize();
		owner.attachRuntime(runtime.handle);
		owner.acceptEnvelope(envelope(MessageKind.terminalExited, event), hostEpoch);
		await owner.activate();

		expect(recovery.pending).toEqual([event]);
		expect(recovery.published).toEqual([event]);
		expect(publication.events).toHaveLength(0);
		expect(order.filter((entry) => entry === 'runtime-publication-commit')).toHaveLength(1);
		expect(order.filter((entry) => entry === 'publication-ledger')).toHaveLength(1);

		clientPublicationAvailable = true;
		await owner.retryPending();

		expect(publication.events).toContainEqual(
			expect.objectContaining({
				presentationId: firstPresentationId,
				type: 'terminal-exited',
			}),
		);
		expect(recovery.pending).toHaveLength(0);
		expect(order.filter((entry) => entry === 'runtime-publication-commit')).toHaveLength(1);
		expect(order.filter((entry) => entry === 'publication-ledger')).toHaveLength(1);
		expect(order.at(-1)).toBe('journal-compact');
		await owner.dispose();
	});

	it('publishes monotonic metadata and an associated failure only for the exact target', async () => {
		const recovery = recoveryState();
		let targetCurrent = false;
		const publication = publicationPort(
			firstEnvironmentId,
			firstPresentationId,
			[],
			() => targetCurrent,
		);
		const runtime = runtimeHandle(firstPresentationId);
		const owner = observationOwner({
			environmentId: firstEnvironmentId,
			publication: publication.port,
			recovery,
		});
		await owner.initialize();
		owner.attachRuntime(runtime.handle);
		await owner.activate();

		owner.acceptEnvelope(
			envelope(MessageKind.terminalMetadataChanged, terminalMetadata('2', 'newest')),
			hostEpoch,
		);
		await owner.retryPending();
		expect(publication.metadata).toHaveLength(0);

		owner.acceptEnvelope(
			envelope(MessageKind.terminalMetadataChanged, terminalMetadata('1', 'stale')),
			hostEpoch,
		);
		targetCurrent = true;
		await owner.retryPending();
		expect(publication.metadata).toEqual([
			expect.objectContaining({ revision: '2', title: 'newest' }),
		]);

		const failure = terminalFailure('3');
		owner.acceptEnvelope(envelope(MessageKind.terminalFailed, failure), hostEpoch);
		await owner.retryPending();
		expect(runtime.recordFailure).toHaveBeenCalledWith(failure);
		expect(publication.events).toContainEqual(
			expect.objectContaining({ presentationId: firstPresentationId, type: 'terminal-failed' }),
		);
		owner.acceptEnvelope(envelope(MessageKind.terminalFailed, failure), hostEpoch);
		await owner.retryPending();
		expect(runtime.recordFailure).toHaveBeenCalledOnce();
		expect(publication.events.filter((event) => event.type === 'terminal-failed')).toHaveLength(1);

		expect(() =>
			owner.acceptEnvelope(
				{
					...envelope(MessageKind.terminalMetadataChanged, terminalMetadata('4', 'foreign')),
					hostEpoch: HostEpochSchema.parse('d'.repeat(32)),
				},
				hostEpoch,
			),
		).toThrow('Host epoch fence');
		const failureCalls = runtime.recordFailure.mock.calls.length;
		owner.sealInbound();
		owner.acceptEnvelope(envelope(MessageKind.terminalFailed, terminalFailure('5')), hostEpoch);
		await owner.retryPending();
		expect(runtime.recordFailure).toHaveBeenCalledTimes(failureCalls);
		await owner.dispose();
	});

	it('retains a retargeted exit across owner restart and isolates the same raw identity per Environment', async () => {
		const firstRecovery = recoveryState();
		let firstTargetCurrent = false;
		const firstPublication = publicationPort(
			firstEnvironmentId,
			firstPresentationId,
			[],
			() => firstTargetCurrent,
		);
		const firstRuntime = runtimeHandle(firstPresentationId);
		const event = terminalExit('8');
		const firstOwner = observationOwner({
			environmentId: firstEnvironmentId,
			publication: firstPublication.port,
			recovery: firstRecovery,
		});
		await firstOwner.initialize();
		firstOwner.attachRuntime(firstRuntime.handle);
		await firstOwner.activate();
		firstOwner.acceptEnvelope(envelope(MessageKind.terminalExited, event), hostEpoch);
		await firstOwner.retryPending();
		expect(firstRecovery.pending).toHaveLength(1);
		expect(firstRecovery.published).toHaveLength(0);
		expect(firstPublication.events).toHaveLength(0);
		await firstOwner.dispose();

		firstTargetCurrent = true;
		const recoveredRuntime = runtimeHandle(firstPresentationId);
		const recoveredOwner = observationOwner({
			environmentId: firstEnvironmentId,
			publication: firstPublication.port,
			recovery: firstRecovery,
		});
		await recoveredOwner.initialize();
		recoveredOwner.attachRuntime(recoveredRuntime.handle);
		expect(recoveredRuntime.recordExitOutcome).not.toHaveBeenCalled();
		await recoveredOwner.activate();
		expect(firstRecovery.pending).toHaveLength(0);
		expect(firstPublication.events).toContainEqual(
			expect.objectContaining({ presentationId: firstPresentationId, type: 'terminal-exited' }),
		);

		const secondRecovery = recoveryState();
		const secondPublication = publicationPort(secondEnvironmentId, secondPresentationId);
		const secondRuntime = runtimeHandle(secondPresentationId);
		const secondOwner = observationOwner({
			environmentId: secondEnvironmentId,
			publication: secondPublication.port,
			recovery: secondRecovery,
		});
		await secondOwner.initialize();
		secondOwner.attachRuntime(secondRuntime.handle);
		await secondOwner.activate();
		secondOwner.acceptEnvelope(envelope(MessageKind.terminalExited, event), hostEpoch);
		await secondOwner.retryPending();
		expect(secondRecovery.published).toEqual([event]);
		expect(secondPublication.events).toContainEqual(
			expect.objectContaining({ presentationId: secondPresentationId, type: 'terminal-exited' }),
		);

		await Promise.all([recoveredOwner.dispose(), secondOwner.dispose()]);
	});

	it('recovers a full-journal rejection only on the next explicit owner activation', async () => {
		const recovery = recoveryState();
		recovery.maximumPending = 0;
		const publication = publicationPort(firstEnvironmentId, firstPresentationId);
		const firstRuntime = runtimeHandle(firstPresentationId);
		const event = terminalExit('9');
		const firstOwner = observationOwner({
			environmentId: firstEnvironmentId,
			publication: publication.port,
			recovery,
		});
		await firstOwner.initialize();
		firstOwner.attachRuntime(firstRuntime.handle);
		await firstOwner.activate();
		firstOwner.acceptEnvelope(envelope(MessageKind.terminalExited, event), hostEpoch);
		await firstOwner.retryPending();
		expect(recovery.pending).toHaveLength(0);
		expect(recovery.rejected).toEqual([event]);
		await firstOwner.dispose();

		recovery.maximumPending = 1;
		const recoveredRuntime = runtimeHandle(firstPresentationId);
		const recoveredOwner = observationOwner({
			environmentId: firstEnvironmentId,
			publication: publication.port,
			recovery,
		});
		await recoveredOwner.initialize();
		expect(recovery.pending).toEqual([event]);
		expect(recovery.rejected).toHaveLength(0);
		recoveredOwner.attachRuntime(recoveredRuntime.handle);
		expect(publication.events).toHaveLength(0);
		await recoveredOwner.activate();
		expect(publication.events).toContainEqual(expect.objectContaining({ type: 'terminal-exited' }));
		await recoveredOwner.dispose();
	});

	it('reloads an Environment exit from real durable stores on the next explicit activation', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-remote-terminal-outcome-'));
		try {
			let targetCurrent = false;
			const publication = publicationPort(
				firstEnvironmentId,
				firstPresentationId,
				[],
				() => targetCurrent,
			);
			const event = terminalExit('10');
			const firstOwner = new DesktopSshEnvironmentTerminalObservationOwner({
				createExitEvent: (presentationId, exited) => ({
					exitCode: exited.exit.kind === 'exit-code' ? exited.exit.code : null,
					presentationId,
					signal: null,
					type: 'terminal-exited',
				}),
				createFailureEvent: (presentationId, failed) => ({
					failure: {
						code: `terminal-runtime-${failed.failure.kind}-${failed.failure.stage}`,
						retryable: false,
					},
					presentationId,
					type: 'terminal-failed',
				}),
				environmentId: firstEnvironmentId,
				publication: publication.port,
				publicationLedger: new TerminalExitPublicationLedger(
					path.join(directory, 'terminal-exit-publications-v1.json'),
				),
				spillStore: new TerminalExitSpillStore(path.join(directory, 'terminal-exits-v1.json')),
			});
			await firstOwner.initialize();
			firstOwner.attachRuntime(runtimeHandle(firstPresentationId).handle);
			await firstOwner.activate();
			firstOwner.acceptEnvelope(envelope(MessageKind.terminalExited, event), hostEpoch);
			await firstOwner.retryPending();
			expect(publication.events).toHaveLength(0);
			await firstOwner.dispose();

			targetCurrent = true;
			const recoveredRuntime = runtimeHandle(firstPresentationId);
			const recoveredOwner = new DesktopSshEnvironmentTerminalObservationOwner({
				createExitEvent: (presentationId, exited) => ({
					exitCode: exited.exit.kind === 'exit-code' ? exited.exit.code : null,
					presentationId,
					signal: null,
					type: 'terminal-exited',
				}),
				createFailureEvent: (presentationId, failed) => ({
					failure: {
						code: `terminal-runtime-${failed.failure.kind}-${failed.failure.stage}`,
						retryable: false,
					},
					presentationId,
					type: 'terminal-failed',
				}),
				environmentId: firstEnvironmentId,
				publication: publication.port,
				publicationLedger: new TerminalExitPublicationLedger(
					path.join(directory, 'terminal-exit-publications-v1.json'),
				),
				spillStore: new TerminalExitSpillStore(path.join(directory, 'terminal-exits-v1.json')),
			});
			await recoveredOwner.initialize();
			recoveredOwner.attachRuntime(recoveredRuntime.handle);
			expect(recoveredRuntime.recordExitOutcome).not.toHaveBeenCalled();
			await recoveredOwner.activate();
			expect(recoveredRuntime.recordExitOutcome).toHaveBeenCalledWith(event);
			expect(publication.events).toContainEqual(
				expect.objectContaining({
					presentationId: firstPresentationId,
					type: 'terminal-exited',
				}),
			);
			await recoveredOwner.dispose();
		} finally {
			await rm(directory, { force: true, recursive: true });
		}
	});
});

function observationOwner(options: {
	readonly environmentId: EnvironmentId;
	readonly publication: DesktopSshEnvironmentTerminalObservationPublicationPort;
	readonly recovery: ReturnType<typeof recoveryState>;
}): DesktopSshEnvironmentTerminalObservationOwner {
	return new DesktopSshEnvironmentTerminalObservationOwner({
		createExitEvent: (presentationId, event) => ({
			exitCode: event.exit.kind === 'exit-code' ? event.exit.code : null,
			presentationId,
			signal: null,
			type: 'terminal-exited',
		}),
		createFailureEvent: (presentationId, event) => ({
			failure: {
				code: `terminal-runtime-${event.failure.kind}-${event.failure.stage}`,
				retryable: false,
			},
			presentationId,
			type: 'terminal-failed',
		}),
		environmentId: options.environmentId,
		publication: options.publication,
		publicationLedger: options.recovery.ledger,
		spillStore: options.recovery.spill,
	});
}

function publicationPort(
	environmentId: EnvironmentId,
	presentationId: TerminalPresentationId,
	order: string[] = [],
	isTargetCurrent: () => boolean = () => true,
	isEventPublicationAvailable: () => boolean = () => true,
): {
	readonly events: TerminalPresentationEvent[];
	readonly metadata: unknown[];
	readonly port: DesktopSshEnvironmentTerminalObservationPublicationPort;
} {
	const events: TerminalPresentationEvent[] = [];
	const metadata: unknown[] = [];
	const external = new Map<string, TerminalPresentationEvent>();
	const reservations = new Map<string, TerminalPresentationEvent>();
	const key = (event: TerminalPresentationEvent) => JSON.stringify(event);
	return {
		events,
		metadata,
		port: {
			commitExternalEvent: (event) => {
				order.push('reconciler-commit');
				if (!reservations.has(key(event))) return false;
				external.set(key(event), event);
				reservations.delete(key(event));
				return true;
			},
			hasExternalEvent: (event) => external.has(key(event)),
			isCurrentPresentationTarget: (candidateEnvironmentId, candidatePresentationId) =>
				candidateEnvironmentId === environmentId &&
				candidatePresentationId === presentationId &&
				isTargetCurrent(),
			publishTerminalEvent: (event) => {
				order.push('client-broadcast');
				if (!isEventPublicationAvailable()) return false;
				events.push(event);
				return true;
			},
			publishTerminalMetadata: (value) => {
				metadata.push(value);
				return true;
			},
			recordExternalEvent: (event) => {
				if (external.has(key(event))) return false;
				external.set(key(event), event);
				return true;
			},
			reserveExternalEvent: (event) => {
				order.push('reconciler-reserve');
				reservations.set(key(event), event);
				return true;
			},
		},
	};
}

function recoveryState(order: string[] = []): {
	readonly ledger: DesktopSshEnvironmentTerminalExitPublicationPort;
	maximumPending: number;
	readonly pending: TerminalExited[];
	readonly published: TerminalExited[];
	readonly rejected: TerminalExited[];
	readonly spill: DesktopSshEnvironmentTerminalExitSpillPort;
} {
	const pending: TerminalExited[] = [];
	const rejected: TerminalExited[] = [];
	const published: TerminalExited[] = [];
	const state = {
		maximumPending: 4_096,
	};
	const find = (entries: readonly TerminalExited[], event: TerminalExited) =>
		entries.findIndex((candidate) => identityKey(candidate) === identityKey(event));
	const spill: DesktopSshEnvironmentTerminalExitSpillPort = {
		admit: (event) => {
			const existing = find(pending, event);
			if (existing === -1 && pending.length >= state.maximumPending) return { kind: 'full' };
			order.push('journal-admit');
			if (existing === -1) pending.push(event);
			else pending[existing] = event;
			const rejectedIndex = find(rejected, event);
			if (rejectedIndex !== -1) rejected.splice(rejectedIndex, 1);
			return { kind: 'admitted' };
		},
		list: () => pending.map((event) => ({ event, reason: 'host-admission' })),
		listRejected: () =>
			rejected.map((event) => ({ event, reason: 'pending-journal-full' as const })),
		load: async () => ({ kind: 'ready' }),
		reject: (event) => {
			if (find(rejected, event) === -1) rejected.push(event);
			return { kind: 'recorded' };
		},
		remove: (event) => {
			order.push('journal-compact');
			const index = find(pending, event);
			if (index !== -1) pending.splice(index, 1);
		},
	};
	const ledger: DesktopSshEnvironmentTerminalExitPublicationPort = {
		has: (event) => {
			const existing = published[find(published, event)];
			return (
				existing !== undefined && BigInt(existing.finalRevision) >= BigInt(event.finalRevision)
			);
		},
		hasExact: (event) => {
			const existing = published[find(published, event)];
			return (
				existing !== undefined && BigInt(existing.finalRevision) === BigInt(event.finalRevision)
			);
		},
		list: () => published.map((event) => ({ event })),
		load: async () => ({ kind: 'ready' }),
		markPublished: (event) => {
			order.push('publication-ledger');
			const index = find(published, event);
			if (index === -1) published.push(event);
			else published[index] = event;
			return true;
		},
	};
	return {
		ledger,
		get maximumPending() {
			return state.maximumPending;
		},
		set maximumPending(value: number) {
			state.maximumPending = value;
		},
		pending,
		published,
		rejected,
		spill,
	};
}

function runtimeHandle(
	presentationId: TerminalPresentationId,
	order: string[] = [],
): {
	readonly handle: DesktopSshEnvironmentTerminalRuntimeHandle;
	readonly recordExitOutcome: ReturnType<
		typeof vi.fn<DesktopSshEnvironmentTerminalRuntimeHandle['recordExitOutcome']>
	>;
	readonly recordFailure: ReturnType<
		typeof vi.fn<DesktopSshEnvironmentTerminalRuntimeHandle['recordFailure']>
	>;
} {
	let identityObserved: ((value: typeof identity) => void) | undefined;
	const recordExitOutcome = vi.fn<DesktopSshEnvironmentTerminalRuntimeHandle['recordExitOutcome']>(
		async (event) => {
			order.push('binding-outcome');
			identityObserved?.(event.identity);
			return { finalRevision: event.finalRevision, kind: 'durable', presentationId };
		},
	);
	const recordFailure = vi.fn<DesktopSshEnvironmentTerminalRuntimeHandle['recordFailure']>(
		async () => presentationId,
	);
	return {
		handle: {
			commitExitPublication: () => {
				order.push('runtime-publication-commit');
				return true;
			},
			dispose: async () => undefined,
			execute: async (command) => ({
				operationId: command.operationId,
				presentationId: command.presentationId,
				type: 'terminal-provision-completed',
			}),
			findPresentationByIdentity: () => presentationId,
			getDurableTerminalExit: () => null,
			getTerminalFailure: () => null,
			initialize: async () => ({ kind: 'ready' }),
			open: async () => {
				throw new Error('Projection is outside this observation test.');
			},
			recordExitOutcome,
			recordFailure,
			releasePendingExit: () => undefined,
			setIdentityObservedListener: (listener) => {
				identityObserved = listener;
			},
		},
		recordExitOutcome,
		recordFailure,
	};
}

function terminalExit(finalRevision: string): TerminalExited {
	return {
		exit: { code: 7, kind: 'exit-code' },
		finalRevision: CanonicalUint64TextSchema.parse(finalRevision),
		identity,
	};
}

function terminalFailure(finalRevision: string): TerminalFailed {
	return {
		failure: { kind: 'pty-read', stage: 'read-output' },
		finalRevision: CanonicalUint64TextSchema.parse(finalRevision),
		identity,
		process: { kind: 'observed', result: { code: 1, kind: 'exit-code' } },
	};
}

function terminalMetadata(revision: string, title: string): TerminalMetadataChanged {
	return {
		currentDirectory: '/srv/orchbee',
		identity,
		revision: CanonicalUint64TextSchema.parse(revision),
		title,
	};
}

function envelope(kind: number, value: unknown): DecodedProtocolEnvelope {
	return {
		correlationId: null,
		event: true,
		hostEpoch,
		ignoredOptionalFlags: 0,
		kind,
		knownMessage: true,
		optionalMessage: false,
		payload: new TextEncoder().encode(JSON.stringify(value)),
		payloadEncoding: PayloadEncoding.controlJson,
		protocol: CURRENT_PROTOCOL_VERSION,
		response: false,
	};
}

function identityKey(event: TerminalExited): string {
	return `${event.identity.hostEpoch}:${event.identity.creationId}:${event.identity.terminalId}`;
}
