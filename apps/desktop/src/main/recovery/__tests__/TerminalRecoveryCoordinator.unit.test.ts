import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	MessageKind,
	TerminalIdSchema,
	TerminalPhase,
	type TerminalExited,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { createDefaultTerminalProfile, type HostConnection } from '../../host';
import { TerminalRecoveryCoordinator } from '../TerminalRecoveryCoordinator';
import {
	TerminalRecoveryLedgerDocumentSchema,
	type TerminalRecoveryLedger,
	type TerminalRecoveryLedgerDocument,
	type TerminalRecoveryLedgerLoadResult,
} from '../recovery-binding';

const creationId = CreationIdSchema.parse('11'.repeat(16));
const firstEpoch = HostEpochSchema.parse('22'.repeat(16));
const secondEpoch = HostEpochSchema.parse('23'.repeat(16));
const terminalId = TerminalIdSchema.parse('33'.repeat(16));
const identity = { creationId, hostEpoch: firstEpoch, terminalId };
const firstRevision = CanonicalUint64TextSchema.parse('1');

class MemoryLedger implements TerminalRecoveryLedger {
	document: TerminalRecoveryLedgerDocument | undefined;
	readonly unavailable: TerminalRecoveryLedgerLoadResult | undefined;
	readonly writes: TerminalRecoveryLedgerDocument[] = [];

	constructor(
		document?: TerminalRecoveryLedgerDocument,
		unavailable?: TerminalRecoveryLedgerLoadResult,
	) {
		this.document = document;
		this.unavailable = unavailable;
	}

	load(): Promise<TerminalRecoveryLedgerLoadResult> {
		return Promise.resolve(
			this.unavailable ??
				(this.document === undefined
					? { kind: 'missing' }
					: { document: this.document, kind: 'available' }),
		);
	}

	save(document: TerminalRecoveryLedgerDocument): Promise<void> {
		this.document = document;
		this.writes.push(document);
		return Promise.resolve();
	}
}

describe('TerminalRecoveryCoordinator', () => {
	it('persists planned and requested states before provisioning one terminal', async () => {
		const ledger = new MemoryLedger();
		const host = createHost(firstEpoch, [], identity);
		const coordinator = createCoordinator(host.value, ledger);

		expect(await coordinator.reconcile()).toEqual({
			identity,
			kind: 'provisioning',
			summary: null,
		});
		expect(ledger.writes.map((write) => write.binding.state.kind)).toEqual([
			'planned',
			'create-requested',
			'retained',
		]);
		expect(host.requestKinds).toEqual([MessageKind.listTerminals, MessageKind.createTerminal]);
	});

	it('recovers a lost create response by replaying the same creation in the same epoch', async () => {
		const ledger = new MemoryLedger();
		const firstHost = createHost(firstEpoch, [], new Error('response lost'));
		const first = createCoordinator(firstHost.value, ledger);

		expect(await first.reconcile()).toEqual({
			kind: 'lost',
			reason: 'create-response-unavailable',
		});
		expect(ledger.document?.binding.state).toEqual({
			hostEpoch: firstEpoch,
			kind: 'create-requested',
		});

		const summary = createSummary();
		const secondHost = createHost(firstEpoch, [summary], identity);
		const second = createCoordinator(secondHost.value, ledger);
		expect(await second.reconcile()).toEqual({ identity, kind: 'recovered', summary });
		expect(secondHost.requestKinds).toEqual([
			MessageKind.listTerminals,
			MessageKind.createTerminal,
		]);
		expect(ledger.document?.binding.creationId).toBe(creationId);
	});

	it('marks a requested binding lost without creating across Host epochs', async () => {
		const ledger = new MemoryLedger(createDocument('create-requested'));
		const host = createHost(secondEpoch, [], identity);

		expect(await createCoordinator(host.value, ledger).reconcile()).toEqual({
			kind: 'lost',
			reason: 'host-epoch-changed',
		});
		expect(host.requestKinds).toEqual([MessageKind.listTerminals]);
		expect(ledger.document?.binding.state.kind).toBe('lost');
	});

	it('leaves an unmatched Host terminal orphaned when no ledger exists', async () => {
		const ledger = new MemoryLedger();
		const host = createHost(firstEpoch, [createSummary()], identity);

		expect(await createCoordinator(host.value, ledger).reconcile()).toEqual({
			kind: 'orphaned',
			terminalCount: 1,
		});
		expect(host.requestKinds).toEqual([MessageKind.listTerminals]);
		expect(ledger.writes).toHaveLength(0);
	});

	it('fails closed before Host inventory access when the ledger is corrupt', async () => {
		const ledger = new MemoryLedger(undefined, { kind: 'unavailable', reason: 'invalid' });
		const host = createHost(firstEpoch, [], identity);

		expect(await createCoordinator(host.value, ledger).reconcile()).toEqual({
			kind: 'ledger-unavailable',
			reason: 'invalid',
		});
		expect(host.requestKinds).toHaveLength(0);
	});

	it('validates a retained exited terminal and records its final revision', async () => {
		const ledger = new MemoryLedger(createDocument('retained'));
		const summary = createSummary({
			exit: { code: 0, kind: 'exit-code' },
			phase: TerminalPhase.exited,
			revision: CanonicalUint64TextSchema.parse('9'),
		});
		const host = createHost(firstEpoch, [summary], identity);

		expect(await createCoordinator(host.value, ledger).reconcile()).toEqual({
			identity,
			kind: 'recovered',
			summary,
		});
		expect(ledger.document?.binding.state).toEqual({
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: '9',
			identity,
			kind: 'exited',
		});
	});

	it('persists a matching asynchronous terminal exit only after retention', async () => {
		const ledger = new MemoryLedger(createDocument('retained'));
		const host = createHost(firstEpoch, [createSummary()], identity);
		const coordinator = createCoordinator(host.value, ledger);
		const exited: TerminalExited = {
			exit: { code: 7, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('12'),
			identity,
		};

		await coordinator.recordExit(exited);

		expect(ledger.document?.binding.state).toEqual({
			exit: exited.exit,
			finalRevision: exited.finalRevision,
			identity,
			kind: 'exited',
		});
	});
});

function createCoordinator(host: HostConnection, ledger: TerminalRecoveryLedger) {
	return new TerminalRecoveryCoordinator(
		host,
		ledger,
		{ bindingId: 'desktop-terminal-foundation', launch: createLaunch() },
		{ randomCreationId: () => creationId },
	);
}

function createDocument(state: 'create-requested' | 'retained') {
	return TerminalRecoveryLedgerDocumentSchema.parse({
		binding: {
			bindingId: 'desktop-terminal-foundation',
			creationId,
			launch: createLaunch(),
			state:
				state === 'retained'
					? { identity, kind: 'retained' }
					: { hostEpoch: firstEpoch, kind: 'create-requested' },
		},
		version: 1,
	});
}

function createHost(
	hostEpoch: typeof firstEpoch,
	inventory: readonly TerminalSummary[],
	createResult: typeof identity | Error,
): { readonly requestKinds: number[]; readonly value: HostConnection } {
	const requestKinds: number[] = [];
	const requestControl = vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(async (kind) => {
		requestKinds.push(kind as number);
		if (kind === MessageKind.listTerminals) return { terminals: inventory };
		if (kind === MessageKind.createTerminal) {
			if (createResult instanceof Error) throw createResult;
			return createResult;
		}
		throw new Error(`Unexpected request kind ${String(kind)}`);
	});
	return {
		requestKinds,
		value: { hostEpoch, requestControl } as unknown as HostConnection,
	};
}

function createLaunch() {
	return {
		arguments: [],
		cellPixelHeight: 16,
		cellPixelWidth: 8,
		columns: 80,
		currentDirectory: '/users/orchbee',
		environment: [{ name: 'TERM', value: 'xterm-256color' }],
		executable: '/bin/sh',
		rows: 24,
		terminalProfile: createDefaultTerminalProfile(),
	};
}

function createSummary(overrides: Partial<TerminalSummary> = {}): TerminalSummary {
	return {
		columns: 80,
		exit: null,
		identity,
		phase: TerminalPhase.running,
		revision: firstRevision,
		rows: 24,
		terminateRequested: false,
		...overrides,
	};
}
