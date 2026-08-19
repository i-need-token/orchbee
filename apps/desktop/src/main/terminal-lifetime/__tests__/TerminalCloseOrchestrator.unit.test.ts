import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	CURRENT_PROTOCOL_VERSION,
	HostEpochSchema,
	MessageKind,
	PayloadEncoding,
	TerminalIdSchema,
	TerminalPhase,
	type DecodedProtocolEnvelope,
	type HostEpoch,
	type MessageKindValue,
	type TerminalExited,
	type TerminalIdentity,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import { createDefaultTerminalProfile, type HostConnection } from '../../host';
import {
	findTerminalBinding,
	TerminalBindingIdSchema,
	type TerminalBinding,
	type TerminalBindingId,
	type TerminalBindingStore,
} from '../../recovery';
import { TerminalCloseOrchestrator } from '../TerminalCloseOrchestrator';

const hostEpoch = HostEpochSchema.parse('11'.repeat(16));
const creationId = CreationIdSchema.parse('21'.repeat(16));
const terminalId = TerminalIdSchema.parse('31'.repeat(16));
const identity = { creationId, hostEpoch, terminalId };
const bindingId = TerminalBindingIdSchema.parse('binding-1');

describe('TerminalCloseOrchestrator', () => {
	it('persists closing and retires projection before graceful termination and disposal', async () => {
		const trace: string[] = [];
		const binding = createBinding({ identity, kind: 'retained' });
		const unrelated = createBinding(
			{ kind: 'planned' },
			TerminalBindingIdSchema.parse('binding-2'),
			CreationIdSchema.parse('22'.repeat(16)),
		);
		const store = new MemoryStore(createDocument([binding, unrelated]), trace);
		const host = new HostHarness(hostEpoch, [createSummary()], trace);
		host.afterRequest = (kind) => {
			if (kind === MessageKind.terminateTerminal) host.emitExit(createExit());
		};
		const orchestrator = new TerminalCloseOrchestrator(host.value, store, bindingId);

		expect(
			await orchestrator.requestClose(async () => {
				trace.push('projection:retired');
			}),
		).toEqual({ kind: 'closed' });
		expect(trace).toEqual([
			'store:put:closing',
			'projection:retired',
			'host:list',
			'host:terminate',
			'host:dispose',
			'store:remove',
		]);
		expect(store.document?.bindings).toEqual([unrelated]);
	});

	it('resolves a requested creation before persisting its exact close identity', async () => {
		const trace: string[] = [];
		const store = new MemoryStore(
			createDocument([createBinding({ hostEpoch, kind: 'create-requested' })]),
			trace,
		);
		const host = new HostHarness(hostEpoch, [createSummary()], trace);
		host.afterRequest = (kind) => {
			if (kind === MessageKind.terminateTerminal) host.emitExit(createExit());
		};
		const orchestrator = new TerminalCloseOrchestrator(host.value, store, bindingId);

		expect(
			await orchestrator.requestClose(async () => {
				trace.push('projection:retired');
			}),
		).toEqual({ kind: 'closed' });
		expect(trace).toEqual([
			'host:list',
			'store:put:closing',
			'projection:retired',
			'host:terminate',
			'host:dispose',
			'store:remove',
		]);
	});

	it('forces only after the grace period and then waits for exact exit', async () => {
		const trace: string[] = [];
		const store = new MemoryStore(
			createDocument([createBinding({ identity, kind: 'retained' })]),
			trace,
		);
		const host = new HostHarness(hostEpoch, [createSummary()], trace);
		const orchestrator = new TerminalCloseOrchestrator(host.value, store, bindingId, {
			waitForGracePeriod: () => {
				trace.push('grace:expired');
				return Promise.resolve();
			},
		});

		const result = orchestrator.requestClose(() => Promise.resolve());
		await vi.waitFor(() =>
			expect(host.lifecycleKinds).toContain(MessageKind.forceTerminateTerminal),
		);
		expect(host.envelopeListenerCount).toBe(1);
		host.emitExit(
			createExit({
				creationId: CreationIdSchema.parse('25'.repeat(16)),
				hostEpoch,
				terminalId: TerminalIdSchema.parse('35'.repeat(16)),
			}),
		);
		await Promise.resolve();
		expect(host.lifecycleKinds).not.toContain(MessageKind.disposeTerminal);
		host.emitExit(createExit());
		await vi.waitFor(() => expect(host.lifecycleKinds).toContain(MessageKind.disposeTerminal));
		expect(await result).toEqual({ kind: 'closed' });
		expect(trace).toEqual([
			'store:put:closing',
			'host:list',
			'host:terminate',
			'grace:expired',
			'host:force',
			'host:dispose',
			'store:remove',
		]);
	});

	it('resumes a durable closing binding and disposes an already exited terminal', async () => {
		const trace: string[] = [];
		const store = new MemoryStore(
			createDocument([createBinding({ identity, kind: 'closing' })]),
			trace,
		);
		const host = new HostHarness(hostEpoch, [createExitedSummary()], trace);
		const orchestrator = new TerminalCloseOrchestrator(host.value, store, bindingId);

		expect(await orchestrator.requestClose(() => Promise.resolve())).toEqual({ kind: 'closed' });
		expect(host.lifecycleKinds).toEqual([MessageKind.disposeTerminal]);
		expect(store.document?.bindings).toHaveLength(0);
	});

	it('disposes an awaiting-start terminal without sending process termination', async () => {
		const trace: string[] = [];
		const store = new MemoryStore(
			createDocument([createBinding({ identity, kind: 'retained' })]),
			trace,
		);
		const host = new HostHarness(hostEpoch, [createAwaitingStartSummary()], trace);
		const orchestrator = new TerminalCloseOrchestrator(host.value, store, bindingId);

		expect(await orchestrator.requestClose(() => Promise.resolve())).toEqual({ kind: 'closed' });
		expect(host.lifecycleKinds).toEqual([MessageKind.disposeTerminal]);
		expect(trace).toEqual(['store:put:closing', 'host:list', 'host:dispose', 'store:remove']);
	});

	it('keeps durable close intent when the Host transport closes during grace', async () => {
		const store = new MemoryStore(createDocument([createBinding({ identity, kind: 'retained' })]));
		const host = new HostHarness(hostEpoch, [createSummary()]);
		const orchestrator = new TerminalCloseOrchestrator(host.value, store, bindingId, {
			waitForGracePeriod: () => new Promise(() => undefined),
		});

		const result = orchestrator.requestClose(() => Promise.resolve());
		await vi.waitFor(() => expect(host.lifecycleKinds).toContain(MessageKind.terminateTerminal));
		expect(host.closeListenerCount).toBe(1);
		host.emitClosed();
		expect(await result).toEqual({
			kind: 'pending',
			reason: 'transport-unavailable',
		});
		expect(findTerminalBinding(requireDocument(store), bindingId)?.state).toEqual({
			identity,
			kind: 'closing',
		});
		expect(host.lifecycleKinds).toEqual([MessageKind.terminateTerminal]);
	});

	it('keeps a naturally finishing target pending until its exact exit arrives', async () => {
		const store = new MemoryStore(createDocument([createBinding({ identity, kind: 'retained' })]));
		const host = new HostHarness(hostEpoch, [createFinishingSummary()]);
		const orchestrator = new TerminalCloseOrchestrator(host.value, store, bindingId);

		expect(await orchestrator.requestClose(() => Promise.resolve())).toEqual({
			kind: 'pending',
			reason: 'terminal-not-running',
		});
		expect(host.lifecycleKinds).toHaveLength(0);
		expect(findTerminalBinding(requireDocument(store), bindingId)?.state.kind).toBe('closing');

		expect(await orchestrator.recordExit(createExit())).toMatchObject({
			handled: true,
			outcome: { kind: 'closed' },
		});
	});

	it('removes a confirmed-absent target without mutating another Host terminal', async () => {
		const binding = createBinding({ identity, kind: 'retained' });
		const store = new MemoryStore(createDocument([binding]));
		const otherIdentity = {
			creationId: CreationIdSchema.parse('24'.repeat(16)),
			hostEpoch,
			terminalId: TerminalIdSchema.parse('34'.repeat(16)),
		};
		const host = new HostHarness(hostEpoch, [createSummary({ identity: otherIdentity })]);
		const orchestrator = new TerminalCloseOrchestrator(host.value, store, bindingId);

		expect(await orchestrator.requestClose(() => Promise.resolve())).toEqual({ kind: 'closed' });
		expect(host.lifecycleKinds).toHaveLength(0);
		expect(store.document?.bindings).toHaveLength(0);
	});

	it('does not mutate Host state when durable close intent cannot be written', async () => {
		const store = new MemoryStore(createDocument([createBinding({ identity, kind: 'retained' })]));
		store.rejectWrites = true;
		const host = new HostHarness(hostEpoch, [createSummary()]);
		const retire = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
		const orchestrator = new TerminalCloseOrchestrator(host.value, store, bindingId);

		expect(await orchestrator.requestClose(retire)).toEqual({
			kind: 'store-unavailable',
			reason: 'write-failed',
		});
		expect(retire).not.toHaveBeenCalled();
		expect(host.requestKinds).toHaveLength(0);
	});

	it('completes a pending close when the exact terminal exit is routed later', async () => {
		const store = new MemoryStore(createDocument([createBinding({ identity, kind: 'closing' })]));
		const host = new HostHarness(hostEpoch, [createSummary()]);
		const orchestrator = new TerminalCloseOrchestrator(host.value, store, bindingId);

		expect(await orchestrator.recordExit(createExit())).toEqual({
			handled: true,
			outcome: { kind: 'closed' },
		});
		expect(host.lifecycleKinds).toEqual([MessageKind.disposeTerminal]);
		expect(store.document?.bindings).toHaveLength(0);
	});
});

class MemoryStore implements TerminalBindingStore {
	document: MemoryDocument | undefined;
	rejectWrites = false;
	readonly #trace: string[];

	constructor(document?: MemoryDocument, trace: string[] = []) {
		this.document = document;
		this.#trace = trace;
	}

	load() {
		return this.document === undefined
			? Promise.resolve({ kind: 'missing' } as const)
			: Promise.resolve({ document: this.document, kind: 'available' } as const);
	}

	put(binding: TerminalBinding): Promise<void> {
		if (this.rejectWrites) return Promise.reject(new Error('write failed'));
		this.#trace.push(`store:put:${binding.state.kind}`);
		this.document = createDocument([
			...(this.document?.bindings.filter(
				(candidate) => candidate.bindingId !== binding.bindingId,
			) ?? []),
			binding,
		]);
		return Promise.resolve();
	}

	remove(targetBindingId: TerminalBindingId): Promise<void> {
		if (this.rejectWrites) return Promise.reject(new Error('write failed'));
		this.#trace.push('store:remove');
		this.document = createDocument(
			this.document?.bindings.filter((binding) => binding.bindingId !== targetBindingId) ?? [],
		);
		return Promise.resolve();
	}
}

class HostHarness {
	afterRequest: ((kind: MessageKindValue) => void) | undefined;
	readonly lifecycleKinds: MessageKindValue[] = [];
	readonly requestKinds: MessageKindValue[] = [];
	readonly value: HostConnection;
	readonly #closeListeners = new Set<(reason: Error) => void>();
	readonly #envelopeListeners = new Set<(envelope: DecodedProtocolEnvelope) => void>();
	readonly #hostEpoch: HostEpoch;
	readonly #inventory: readonly TerminalSummary[];
	readonly #trace: string[];

	constructor(
		hostEpochValue: HostEpoch,
		inventory: readonly TerminalSummary[],
		trace: string[] = [],
	) {
		this.#hostEpoch = hostEpochValue;
		this.#inventory = inventory;
		this.#trace = trace;
		this.value = {
			hostEpoch: hostEpochValue,
			onClosed: (listener: (reason: Error) => void) => {
				this.#closeListeners.add(listener);
				return () => this.#closeListeners.delete(listener);
			},
			onEnvelope: (listener: (envelope: DecodedProtocolEnvelope) => void) => {
				this.#envelopeListeners.add(listener);
				return () => this.#envelopeListeners.delete(listener);
			},
			requestControl: async <Output>(kind: MessageKindValue) => {
				this.requestKinds.push(kind);
				if (kind === MessageKind.listTerminals) {
					this.#trace.push('host:list');
					return {
						inventoryGeneration: '1',
						nextCursor: null,
						terminals: this.#inventory,
					} as Output;
				}
				this.lifecycleKinds.push(kind);
				let traceKind = 'host:dispose';
				if (kind === MessageKind.terminateTerminal) traceKind = 'host:terminate';
				if (kind === MessageKind.forceTerminateTerminal) traceKind = 'host:force';
				this.#trace.push(traceKind);
				this.afterRequest?.(kind);
				return { terminalId } as Output;
			},
		} as unknown as HostConnection;
	}

	get closeListenerCount(): number {
		return this.#closeListeners.size;
	}

	get envelopeListenerCount(): number {
		return this.#envelopeListeners.size;
	}

	emitClosed(): void {
		const reason = new Error('Host connection closed.');
		for (const listener of this.#closeListeners) listener(reason);
	}

	emitExit(event: TerminalExited): void {
		const envelope: DecodedProtocolEnvelope = {
			correlationId: null,
			event: true,
			hostEpoch: this.#hostEpoch,
			ignoredOptionalFlags: 0,
			kind: MessageKind.terminalExited,
			knownMessage: true,
			optionalMessage: false,
			payload: new TextEncoder().encode(JSON.stringify(event)),
			payloadEncoding: PayloadEncoding.controlJson,
			protocol: CURRENT_PROTOCOL_VERSION,
			response: false,
		};
		for (const listener of this.#envelopeListeners) listener(envelope);
	}
}

function createBinding(
	state: TerminalBinding['state'],
	targetBindingId = bindingId,
	targetCreationId = creationId,
): TerminalBinding {
	return {
		bindingId: targetBindingId,
		creationId: targetCreationId,
		launch: {
			arguments: [],
			cellPixelHeight: 16,
			cellPixelWidth: 8,
			columns: 80,
			configuration: null,
			currentDirectory: '/users/orchbee',
			environment: [],
			executable: '/bin/sh',
			rows: 24,
			terminalProfile: createDefaultTerminalProfile(),
		},
		presentationId: terminalPresentationId(`presentation-${targetBindingId}`),
		state,
	};
}

type RunningTerminalSummary = Extract<
	TerminalSummary,
	{ readonly phase: typeof TerminalPhase.running }
>;

function createSummary(overrides: Partial<RunningTerminalSummary> = {}): RunningTerminalSummary {
	return {
		columns: 80,
		configuration: { scrollbackBytes: 10_000_000 },
		configurationRevision: CanonicalUint64TextSchema.parse('1'),
		exit: null,
		failure: null,
		failureContainment: null,
		identity,
		phase: TerminalPhase.running,
		process: null,
		revision: CanonicalUint64TextSchema.parse('4'),
		rows: 24,
		terminateRequested: false,
		...overrides,
	};
}

function createFinishingSummary(): Extract<
	TerminalSummary,
	{ readonly phase: typeof TerminalPhase.finishing }
> {
	return {
		...createSummary(),
		exit: null,
		phase: TerminalPhase.finishing,
	};
}

function createAwaitingStartSummary(): Extract<
	TerminalSummary,
	{ readonly phase: typeof TerminalPhase.awaitingStart }
> {
	return {
		columns: null,
		configuration: { scrollbackBytes: 10_000_000 },
		configurationRevision: CanonicalUint64TextSchema.parse('1'),
		exit: null,
		failure: null,
		failureContainment: null,
		identity,
		phase: TerminalPhase.awaitingStart,
		process: null,
		revision: CanonicalUint64TextSchema.parse('0'),
		rows: null,
		terminateRequested: false,
	};
}

function createExitedSummary(): Extract<
	TerminalSummary,
	{ readonly phase: typeof TerminalPhase.exited }
> {
	return {
		...createSummary(),
		exit: { code: 0, kind: 'exit-code' },
		phase: TerminalPhase.exited,
	};
}

function createExit(targetIdentity: TerminalIdentity = identity): TerminalExited {
	return {
		exit: { code: 0, kind: 'exit-code' },
		finalRevision: CanonicalUint64TextSchema.parse('5'),
		identity: targetIdentity,
	};
}

function requireDocument(store: MemoryStore): MemoryDocument {
	if (store.document === undefined) throw new Error('Expected a terminal binding store document.');
	return store.document;
}

interface MemoryDocument {
	readonly bindings: TerminalBinding[];
	readonly version: 3;
}

function createDocument(bindings: readonly TerminalBinding[] = []): MemoryDocument {
	return {
		bindings: [...bindings].sort((left, right) => left.bindingId.localeCompare(right.bindingId)),
		version: 3,
	};
}
