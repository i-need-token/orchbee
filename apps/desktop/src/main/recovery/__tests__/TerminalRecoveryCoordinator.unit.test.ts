import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	MessageKind,
	TerminalIdSchema,
	TerminalPhase,
	type TerminalExited,
	type TerminalInventoryCursor,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { createDefaultTerminalProfile, type HostConnection } from '../../host';
import { TerminalRecoveryCoordinator } from '../TerminalRecoveryCoordinator';
import {
	createTerminalBindingStoreDocument,
	putTerminalBinding,
	removeTerminalBinding,
	TerminalBindingIdSchema,
	TerminalBindingSchema,
	type TerminalBinding,
	type TerminalBindingId,
	type TerminalBindingStore,
	type TerminalBindingStoreDocument,
	type TerminalBindingStoreLoadResult,
} from '../terminal-binding-store';

const creationId = CreationIdSchema.parse('11'.repeat(16));
const otherCreationId = CreationIdSchema.parse('12'.repeat(16));
const firstEpoch = HostEpochSchema.parse('22'.repeat(16));
const secondEpoch = HostEpochSchema.parse('23'.repeat(16));
const terminalId = TerminalIdSchema.parse('33'.repeat(16));
const otherTerminalId = TerminalIdSchema.parse('34'.repeat(16));
const identity = { creationId, hostEpoch: firstEpoch, terminalId };
const otherIdentity = {
	creationId: otherCreationId,
	hostEpoch: firstEpoch,
	terminalId: otherTerminalId,
};
const firstRevision = CanonicalUint64TextSchema.parse('1');
const targetBindingId = TerminalBindingIdSchema.parse('desktop-terminal-foundation');

class MemoryStore implements TerminalBindingStore {
	document: TerminalBindingStoreDocument | undefined;
	readonly unavailable: TerminalBindingStoreLoadResult | undefined;
	readonly writes: TerminalBinding[] = [];

	constructor(
		document?: TerminalBindingStoreDocument,
		unavailable?: TerminalBindingStoreLoadResult,
	) {
		this.document = document;
		this.unavailable = unavailable;
	}

	load(): Promise<TerminalBindingStoreLoadResult> {
		return Promise.resolve(
			this.unavailable ??
				(this.document === undefined
					? { kind: 'missing' }
					: { document: this.document, kind: 'available' }),
		);
	}

	put(binding: TerminalBinding): Promise<void> {
		this.document = putTerminalBinding(
			this.document ?? createTerminalBindingStoreDocument(),
			binding,
		);
		this.writes.push(binding);
		return Promise.resolve();
	}

	remove(bindingId: TerminalBindingId): Promise<void> {
		this.document = removeTerminalBinding(
			this.document ?? createTerminalBindingStoreDocument(),
			bindingId,
		);
		return Promise.resolve();
	}
}

describe('TerminalRecoveryCoordinator', () => {
	it('persists planned and requested states before provisioning one terminal', async () => {
		const store = new MemoryStore();
		const host = createHost(firstEpoch, [[]], identity);
		const coordinator = createCoordinator(host.value, store);

		expect(await coordinator.reconcile()).toEqual({
			identity,
			kind: 'provisioning',
			summary: null,
		});
		expect(store.writes.map((binding) => binding.state.kind)).toEqual([
			'planned',
			'create-requested',
			'retained',
		]);
		expect(host.requestKinds).toEqual([MessageKind.listTerminals, MessageKind.createTerminal]);
	});

	it('recovers a lost create response by replaying the same creation in the same epoch', async () => {
		const store = new MemoryStore();
		const firstHost = createHost(firstEpoch, [[]], new Error('response lost'));
		const first = createCoordinator(firstHost.value, store);

		expect(await first.reconcile()).toEqual({
			kind: 'lost',
			reason: 'create-response-unavailable',
		});
		expect(targetBinding(store).state).toEqual({
			hostEpoch: firstEpoch,
			kind: 'create-requested',
		});

		const summary = createSummary();
		const secondHost = createHost(firstEpoch, [[summary]], identity);
		const second = createCoordinator(secondHost.value, store);
		expect(await second.reconcile()).toEqual({ identity, kind: 'recovered', summary });
		expect(secondHost.requestKinds).toEqual([
			MessageKind.listTerminals,
			MessageKind.createTerminal,
		]);
		expect(targetBinding(store).creationId).toBe(creationId);
	});

	it('marks a requested binding lost without creating across Host epochs', async () => {
		const store = new MemoryStore(createDocument('create-requested'));
		const host = createHost(secondEpoch, [[]], identity);

		expect(await createCoordinator(host.value, store).reconcile()).toEqual({
			kind: 'lost',
			reason: 'host-epoch-changed',
		});
		expect(host.requestKinds).toEqual([MessageKind.listTerminals]);
		expect(targetBinding(store).state.kind).toBe('lost');
	});

	it('leaves an unmatched Host terminal orphaned when the target binding is absent', async () => {
		const store = new MemoryStore();
		const host = createHost(firstEpoch, [[createSummary()]], identity);

		expect(await createCoordinator(host.value, store).reconcile()).toEqual({
			kind: 'orphaned',
			terminalCount: 1,
		});
		expect(host.requestKinds).toEqual([MessageKind.listTerminals]);
		expect(store.writes).toHaveLength(0);
	});

	it('recognizes another managed binding instead of treating it as an orphan', async () => {
		const other = createOtherBinding();
		const store = new MemoryStore(createTerminalBindingStoreDocument([other]));
		const host = createHost(firstEpoch, [[createSummary({ identity: otherIdentity })]], identity);

		expect(await createCoordinator(host.value, store).reconcile()).toEqual({
			identity,
			kind: 'provisioning',
			summary: null,
		});
		expect(
			store.document?.bindings.find((binding) => binding.bindingId === other.bindingId),
		).toEqual(other);
	});

	it('does not conceal an identity-conflicting terminal behind another binding CreationId', async () => {
		const other = createOtherBinding();
		const store = new MemoryStore(createTerminalBindingStoreDocument([other]));
		const conflictingSummary = createSummary({
			identity: {
				...otherIdentity,
				terminalId: TerminalIdSchema.parse('35'.repeat(16)),
			},
		});
		const host = createHost(firstEpoch, [[conflictingSummary]], identity);

		expect(await createCoordinator(host.value, store).reconcile()).toEqual({
			kind: 'orphaned',
			terminalCount: 1,
		});
		expect(store.writes).toHaveLength(0);
	});

	it('fails closed before Host inventory access when the store is corrupt', async () => {
		const store = new MemoryStore(undefined, { kind: 'unavailable', reason: 'invalid' });
		const host = createHost(firstEpoch, [[]], identity);

		expect(await createCoordinator(host.value, store).reconcile()).toEqual({
			kind: 'ledger-unavailable',
			reason: 'invalid',
		});
		expect(host.requestKinds).toHaveLength(0);
	});

	it('consumes every stable inventory page before recovering the target', async () => {
		const summary = createSummary();
		const otherSummary = createSummary({ identity: otherIdentity });
		const store = new MemoryStore(
			createTerminalBindingStoreDocument([createDocumentBinding('retained'), createOtherBinding()]),
		);
		const host = createHost(firstEpoch, [[otherSummary], [summary]], identity);

		expect(await createCoordinator(host.value, store).reconcile()).toEqual({
			identity,
			kind: 'recovered',
			summary,
		});
		expect(host.listCursors).toEqual([null, { inventoryGeneration: '7', offset: '1' }]);
	});

	it('rejects a repeated inventory cursor instead of exposing a partial scan', async () => {
		const store = new MemoryStore(createDocument('retained'));
		const host = createHost(firstEpoch, [[createSummary()], [], []], identity, true);

		await expect(createCoordinator(host.value, store).reconcile()).rejects.toThrow(
			'repeated a pagination cursor',
		);
		expect(host.requestKinds).toEqual([MessageKind.listTerminals, MessageKind.listTerminals]);
		expect(store.writes).toHaveLength(0);
	});

	it('records a matching exit without replacing another binding', async () => {
		const other = createOtherBinding();
		const store = new MemoryStore(
			createTerminalBindingStoreDocument([createDocumentBinding('retained'), other]),
		);
		const host = createHost(firstEpoch, [[createSummary()]], identity);
		const coordinator = createCoordinator(host.value, store);
		const exited: TerminalExited = {
			exit: { code: 7, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('12'),
			identity,
		};

		await coordinator.recordExit(exited);

		expect(targetBinding(store).state).toEqual({
			exit: exited.exit,
			finalRevision: exited.finalRevision,
			identity,
			kind: 'exited',
		});
		expect(
			store.document?.bindings.find((binding) => binding.bindingId === other.bindingId),
		).toEqual(other);
	});
});

function createCoordinator(host: HostConnection, store: TerminalBindingStore) {
	return new TerminalRecoveryCoordinator(
		host,
		store,
		{ bindingId: targetBindingId, launch: createLaunch() },
		{ randomCreationId: () => creationId },
	);
}

function createDocument(state: 'create-requested' | 'retained'): TerminalBindingStoreDocument {
	return createTerminalBindingStoreDocument([createDocumentBinding(state)]);
}

function createDocumentBinding(state: 'create-requested' | 'retained'): TerminalBinding {
	return TerminalBindingSchema.parse({
		bindingId: targetBindingId,
		creationId,
		launch: createLaunch(),
		state:
			state === 'retained'
				? { identity, kind: 'retained' }
				: { hostEpoch: firstEpoch, kind: 'create-requested' },
	});
}

function createOtherBinding(): TerminalBinding {
	return TerminalBindingSchema.parse({
		bindingId: TerminalBindingIdSchema.parse('other-binding'),
		creationId: otherCreationId,
		launch: createLaunch(),
		state: { identity: otherIdentity, kind: 'retained' },
	});
}

function createHost(
	hostEpoch: typeof firstEpoch | typeof secondEpoch,
	pages: readonly (readonly TerminalSummary[])[],
	createResult: typeof identity | Error,
	repeatCursor = false,
): {
	readonly listCursors: Array<TerminalInventoryCursor | null>;
	readonly requestKinds: number[];
	readonly value: HostConnection;
} {
	const listCursors: Array<TerminalInventoryCursor | null> = [];
	const requestKinds: number[] = [];
	const requestControl = vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(
		async (kind, value) => {
			requestKinds.push(kind as number);
			if (kind === MessageKind.listTerminals) {
				const cursor = (value as { readonly cursor: TerminalInventoryCursor | null }).cursor;
				listCursors.push(cursor);
				const pageIndex = cursor === null ? 0 : Number(cursor.offset);
				const terminals = pages[pageIndex] ?? [];
				const hasNext = pageIndex + 1 < pages.length;
				return {
					inventoryGeneration: CanonicalUint64TextSchema.parse('7'),
					nextCursor: hasNext
						? {
								inventoryGeneration: CanonicalUint64TextSchema.parse('7'),
								offset: CanonicalUint64TextSchema.parse(
									repeatCursor && pageIndex > 0 ? '1' : String(pageIndex + 1),
								),
							}
						: null,
					terminals,
				};
			}
			if (kind === MessageKind.createTerminal) {
				if (createResult instanceof Error) throw createResult;
				return createResult;
			}
			throw new Error(`Unexpected request kind ${String(kind)}`);
		},
	);
	return {
		listCursors,
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

function targetBinding(store: MemoryStore): TerminalBinding {
	const binding = store.document?.bindings.find(
		(candidate) => candidate.bindingId === targetBindingId,
	);
	if (binding === undefined) throw new Error('Target binding is missing.');
	return binding;
}
