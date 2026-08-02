import { CreationIdSchema, HostEpochSchema, TerminalIdSchema } from '@orchbee/terminal-protocol';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
} from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import type { TerminalDocumentControlCommand } from '../../../shared';
import { resolveDesktopRuntimeConfiguration } from '../../composition';
import { TerminalBindingIdSchema, type TerminalBinding } from '../../recovery';
import { DesktopTerminalBindingRegistry } from '../../terminal-binding-registry';
import type {
	TerminalBindingLifecycleOutcome,
	TerminalCloseOutcome,
} from '../../terminal-lifetime';
import {
	DesktopTerminalLifecycleAdapter,
	type DesktopTerminalBindingLifecyclePort,
	type DesktopTerminalLifecycleAdapterDependencies,
} from '../DesktopTerminalLifecycleAdapter';

const identity = {
	creationId: CreationIdSchema.parse('11'.repeat(16)),
	hostEpoch: HostEpochSchema.parse('22'.repeat(16)),
	terminalId: TerminalIdSchema.parse('33'.repeat(16)),
};

function provisionCommand(
	presentation = 'presentation-1',
	operation = 'provision-1',
	launchCwd: string | null = '/workspace/orchbee',
): TerminalDocumentControlCommand {
	return {
		launchCwd,
		operationId: terminalPresentationOperationId(operation),
		presentationId: terminalPresentationId(presentation),
		type: 'terminal-provision-requested',
	};
}

function closeCommand(
	presentation = 'presentation-1',
	operation = 'close-1',
): TerminalDocumentControlCommand {
	return {
		operationId: terminalPresentationOperationId(operation),
		presentationId: terminalPresentationId(presentation),
		type: 'terminal-close-requested',
	};
}

function restoreCommand(
	presentation = 'presentation-1',
	operation = 'restore-1',
	launchCwd: string | null = '/workspace/orchbee',
): TerminalDocumentControlCommand {
	return {
		launchCwd,
		operationId: terminalPresentationOperationId(operation),
		presentationId: terminalPresentationId(presentation),
		type: 'terminal-restore-requested',
	};
}

function createLifecycle(
	reconcile: () => Promise<TerminalBindingLifecycleOutcome> = async () => ({
		identity,
		kind: 'provisioning',
		summary: null,
	}),
	requestClose: (retireProjection: () => Promise<void>) => Promise<TerminalCloseOutcome> = async (
		retireProjection,
	) => {
		await retireProjection();
		return { kind: 'closed' };
	},
): DesktopTerminalBindingLifecyclePort & {
	readonly reconcile: ReturnType<typeof vi.fn<() => Promise<TerminalBindingLifecycleOutcome>>>;
	readonly requestClose: ReturnType<
		typeof vi.fn<(retireProjection: () => Promise<void>) => Promise<TerminalCloseOutcome>>
	>;
} {
	return {
		reconcile: vi.fn<typeof reconcile>(reconcile),
		recordExit: vi
			.fn<DesktopTerminalBindingLifecyclePort['recordExit']>()
			.mockResolvedValue({ handled: false }),
		requestClose: vi.fn<typeof requestClose>(requestClose),
	};
}

function createHarness(
	options: {
		readonly allocateFreshBindingId?: (() => string) | undefined;
		readonly createBindingLifecycle?:
			| DesktopTerminalLifecycleAdapterDependencies['createBindingLifecycle']
			| undefined;
	} = {},
) {
	const registry = new DesktopTerminalBindingRegistry();
	const lifecycle = createLifecycle();
	const createBindingLifecycle = vi.fn<
		DesktopTerminalLifecycleAdapterDependencies['createBindingLifecycle']
	>(options.createBindingLifecycle ?? (() => lifecycle));
	const retireProjection = vi.fn<
		(presentationId: ReturnType<typeof terminalPresentationId>) => Promise<void>
	>(async () => undefined);
	const adapter = new DesktopTerminalLifecycleAdapter({
		allocateFreshBindingId: options.allocateFreshBindingId ?? (() => 'binding-1'),
		createBindingLifecycle,
		loadBindings: async () => ({ kind: 'missing' }),
		registry,
		retireProjection,
		terminalLaunch: createTerminalLaunch(),
	});
	return { adapter, createBindingLifecycle, lifecycle, registry, retireProjection };
}

describe('DesktopTerminalLifecycleAdapter', () => {
	it('drains only bindings absent from the durably referenced shared document', async () => {
		let bindingSequence = 1;
		const harness = createHarness({
			allocateFreshBindingId: () => `binding-${bindingSequence++}`,
		});
		await harness.adapter.execute(provisionCommand('presentation-1', 'provision-1'));
		await harness.adapter.execute(provisionCommand('presentation-2', 'provision-2'));

		await expect(
			harness.adapter.closeBindingsNotReferencedBy(
				new Set([terminalPresentationId('presentation-1')]),
			),
		).resolves.toEqual({ closedCount: 1, failures: [] });
		expect(harness.registry.getSnapshot()).toEqual([
			{ bindingId: 'binding-1', presentationId: 'presentation-1' },
		]);
		expect(harness.lifecycle.requestClose).toHaveBeenCalledOnce();
	});

	it('allocates one exact binding and applies only the presentation launch CWD', async () => {
		const harness = createHarness();

		await expect(harness.adapter.execute(provisionCommand())).resolves.toEqual({
			operationId: 'provision-1',
			presentationId: 'presentation-1',
			type: 'terminal-provision-completed',
		});

		expect(harness.registry.getByPresentationId(terminalPresentationId('presentation-1'))).toEqual({
			bindingId: 'binding-1',
			presentationId: 'presentation-1',
		});
		const launch = harness.createBindingLifecycle.mock.calls[0]?.[1];
		expect(launch).toMatchObject({
			arguments: [],
			currentDirectory: '/workspace/orchbee',
			executable: '/bin/sh',
		});
		expect(launch?.environment).toEqual(createTerminalLaunch().environment);
	});

	it('reuses one binding for a same-launch provision retry', async () => {
		const harness = createHarness();

		await harness.adapter.execute(provisionCommand());
		await expect(
			harness.adapter.execute(provisionCommand('presentation-1', 'provision-2')),
		).resolves.toMatchObject({ type: 'terminal-provision-completed' });

		expect(harness.createBindingLifecycle).toHaveBeenCalledOnce();
		expect(harness.lifecycle.reconcile).toHaveBeenCalledTimes(2);
		expect(harness.registry.size).toBe(1);
	});

	it('restores only an already restorable binding and never allocates a replacement', async () => {
		const retainedLifecycle = createLifecycle(async () => ({
			identity,
			kind: 'provisioning',
			summary: null,
		}));
		const allocateFreshBindingId = vi.fn<() => string>(() => 'replacement-binding');
		const retainedBinding: TerminalBinding = {
			bindingId: TerminalBindingIdSchema.parse('binding-retained'),
			creationId: identity.creationId,
			launch: { ...createTerminalLaunch(), currentDirectory: '/workspace/orchbee' },
			presentationId: terminalPresentationId('presentation-1'),
			state: { identity, kind: 'retained' },
		};
		const retained = new DesktopTerminalLifecycleAdapter({
			allocateFreshBindingId,
			createBindingLifecycle: () => retainedLifecycle,
			loadBindings: async () => ({
				document: { bindings: [retainedBinding], version: 3 },
				kind: 'available',
			}),
			registry: new DesktopTerminalBindingRegistry(),
			retireProjection: async () => undefined,
			terminalLaunch: createTerminalLaunch(),
		});

		await expect(retained.execute(restoreCommand())).resolves.toEqual({
			operationId: 'restore-1',
			presentationId: 'presentation-1',
			type: 'terminal-restore-completed',
		});
		expect(retainedLifecycle.reconcile).toHaveBeenCalledOnce();
		expect(allocateFreshBindingId).not.toHaveBeenCalled();

		const missing = createHarness({ allocateFreshBindingId });
		await expect(missing.adapter.execute(restoreCommand())).resolves.toEqual({
			failure: { code: 'terminal-identity-unavailable', retryable: false },
			operationId: 'restore-1',
			presentationId: 'presentation-1',
			type: 'terminal-restore-failed',
		});
		expect(missing.createBindingLifecycle).not.toHaveBeenCalled();
		expect(allocateFreshBindingId).not.toHaveBeenCalled();
	});

	it('rejects restore for a planned binding instead of turning it into provision', async () => {
		const lifecycle = createLifecycle();
		const plannedBinding: TerminalBinding = {
			bindingId: TerminalBindingIdSchema.parse('binding-planned'),
			creationId: identity.creationId,
			launch: { ...createTerminalLaunch(), currentDirectory: '/workspace/orchbee' },
			presentationId: terminalPresentationId('presentation-1'),
			state: { kind: 'planned' },
		};
		const adapter = new DesktopTerminalLifecycleAdapter({
			allocateFreshBindingId: () => 'replacement-binding',
			createBindingLifecycle: () => lifecycle,
			loadBindings: async () => ({
				document: { bindings: [plannedBinding], version: 3 },
				kind: 'available',
			}),
			registry: new DesktopTerminalBindingRegistry(),
			retireProjection: async () => undefined,
			terminalLaunch: createTerminalLaunch(),
		});

		await expect(adapter.execute(restoreCommand())).resolves.toMatchObject({
			failure: { code: 'terminal-identity-unavailable', retryable: false },
			type: 'terminal-restore-failed',
		});
		expect(lifecycle.reconcile).not.toHaveBeenCalled();
	});

	it('rejects launch mutation for an existing presentation without touching its lifecycle', async () => {
		const harness = createHarness();
		await harness.adapter.execute(provisionCommand());

		await expect(
			harness.adapter.execute(
				provisionCommand('presentation-1', 'provision-2', '/different-workspace'),
			),
		).resolves.toEqual({
			failure: { code: 'terminal-launch-conflict', retryable: false },
			operationId: 'provision-2',
			presentationId: 'presentation-1',
			type: 'terminal-provision-failed',
		});
		expect(harness.lifecycle.reconcile).toHaveBeenCalledOnce();
	});

	it('leaves no registration when binding allocation is invalid or collides', async () => {
		const invalid = createHarness({ allocateFreshBindingId: () => 'contains spaces' });
		await expect(invalid.adapter.execute(provisionCommand())).resolves.toMatchObject({
			failure: { code: 'terminal-binding-allocation-unavailable', retryable: true },
			type: 'terminal-provision-failed',
		});
		expect(invalid.createBindingLifecycle).not.toHaveBeenCalled();
		expect(invalid.registry.size).toBe(0);

		const collision = createHarness({ allocateFreshBindingId: () => 'same-binding' });
		await collision.adapter.execute(provisionCommand());
		await expect(
			collision.adapter.execute(provisionCommand('presentation-2', 'provision-2')),
		).resolves.toMatchObject({
			failure: { code: 'terminal-binding-allocation-conflict', retryable: true },
			type: 'terminal-provision-failed',
		});
		expect(collision.registry.size).toBe(1);
	});

	it('serializes provision and close for one presentation through exact projection retirement', async () => {
		const provision = createDeferred<TerminalBindingLifecycleOutcome>();
		const lifecycle = createLifecycle(
			() => provision.promise,
			async (retireProjection) => {
				await retireProjection();
				return { kind: 'closed' };
			},
		);
		const harness = createHarness({ createBindingLifecycle: () => lifecycle });

		const provisionResult = harness.adapter.execute(provisionCommand());
		const closeResult = harness.adapter.execute(closeCommand());
		expect(lifecycle.requestClose).not.toHaveBeenCalled();
		provision.resolve({ identity, kind: 'provisioning', summary: null });

		await expect(provisionResult).resolves.toMatchObject({
			type: 'terminal-provision-completed',
		});
		await expect(closeResult).resolves.toMatchObject({ type: 'terminal-close-completed' });
		expect(harness.retireProjection).toHaveBeenCalledWith(terminalPresentationId('presentation-1'));
		expect(harness.registry.size).toBe(0);
	});

	it('allows independent presentations to reconcile concurrently', async () => {
		const first = createDeferred<TerminalBindingLifecycleOutcome>();
		const second = createDeferred<TerminalBindingLifecycleOutcome>();
		const firstLifecycle = createLifecycle(() => first.promise);
		const secondLifecycle = createLifecycle(() => second.promise);
		const lifecycles = [firstLifecycle, secondLifecycle];
		let bindingSequence = 0;
		const harness = createHarness({
			allocateFreshBindingId: () => `binding-${++bindingSequence}`,
			createBindingLifecycle: () => lifecycles.shift() ?? createLifecycle(),
		});

		const firstResult = harness.adapter.execute(provisionCommand());
		const secondResult = harness.adapter.execute(provisionCommand('presentation-2', 'provision-2'));
		await vi.waitFor(() => {
			expect(harness.createBindingLifecycle).toHaveBeenCalledTimes(2);
			expect(firstLifecycle.reconcile).toHaveBeenCalledOnce();
			expect(secondLifecycle.reconcile).toHaveBeenCalledOnce();
		});
		first.resolve({ identity, kind: 'provisioning', summary: null });
		second.resolve({
			identity: {
				...identity,
				creationId: CreationIdSchema.parse('44'.repeat(16)),
				terminalId: TerminalIdSchema.parse('55'.repeat(16)),
			},
			kind: 'provisioning',
			summary: null,
		});
		await Promise.all([firstResult, secondResult]);
		expect(harness.registry.size).toBe(2);
	});

	it('retains a pending close for an exact retry and removes only after completion', async () => {
		const lifecycle = createLifecycle();
		lifecycle.requestClose
			.mockResolvedValueOnce({ kind: 'pending', reason: 'transport-unavailable' })
			.mockImplementationOnce(async (retireProjection) => {
				await retireProjection();
				return { kind: 'closed' };
			});
		const harness = createHarness({ createBindingLifecycle: () => lifecycle });
		await harness.adapter.execute(provisionCommand());

		await expect(harness.adapter.execute(closeCommand())).resolves.toMatchObject({
			failure: { code: 'terminal-transport-unavailable', retryable: true },
			type: 'terminal-close-failed',
		});
		expect(harness.registry.size).toBe(1);
		await expect(
			harness.adapter.execute(closeCommand('presentation-1', 'close-2')),
		).resolves.toMatchObject({ type: 'terminal-close-completed' });
		expect(harness.registry.size).toBe(0);
	});

	it('never removes a conflicting replacement registration after terminal close', async () => {
		const harness = createHarness();
		await harness.adapter.execute(provisionCommand());
		const presentationId = terminalPresentationId('presentation-1');
		harness.registry.remove(presentationId, TerminalBindingIdSchema.parse('binding-1'));
		harness.registry.register({
			bindingId: TerminalBindingIdSchema.parse('replacement-binding'),
			presentationId,
		});

		await expect(harness.adapter.execute(closeCommand())).resolves.toMatchObject({
			failure: { code: 'terminal-binding-registry-conflict', retryable: false },
			type: 'terminal-close-failed',
		});
		expect(harness.registry.getByPresentationId(presentationId)).toMatchObject({
			bindingId: 'replacement-binding',
		});
	});

	it('makes an unbound close idempotent and permanently rejects presentation reuse', async () => {
		const harness = createHarness();

		await expect(harness.adapter.execute(closeCommand())).resolves.toMatchObject({
			type: 'terminal-close-completed',
		});
		await expect(
			harness.adapter.execute(closeCommand('presentation-1', 'close-2')),
		).resolves.toMatchObject({ type: 'terminal-close-completed' });
		await expect(harness.adapter.execute(provisionCommand())).resolves.toMatchObject({
			failure: { code: 'terminal-presentation-retired', retryable: false },
			type: 'terminal-provision-failed',
		});
		expect(harness.createBindingLifecycle).not.toHaveBeenCalled();
	});

	it('maps expected recovery and storage outcomes to stable presentation failures', async () => {
		const cases: readonly [TerminalBindingLifecycleOutcome, string, boolean][] = [
			[{ kind: 'lost', reason: 'create-response-unavailable' }, 'terminal-unavailable', true],
			[{ kind: 'lost', reason: 'host-epoch-changed' }, 'terminal-identity-unavailable', false],
			[{ kind: 'conflict', reason: 'identity-conflict' }, 'terminal-identity-conflict', false],
			[{ kind: 'orphaned', terminalCount: 2 }, 'terminal-orphan-conflict', false],
			[{ kind: 'ledger-unavailable', reason: 'read-failed' }, 'terminal-storage-unavailable', true],
			[{ kind: 'ledger-unavailable', reason: 'invalid' }, 'terminal-storage-invalid', false],
		];

		for (const [outcome, code, retryable] of cases) {
			const harness = createHarness({
				createBindingLifecycle: () => createLifecycle(async () => outcome),
			});
			await expect(harness.adapter.execute(provisionCommand())).resolves.toMatchObject({
				failure: { code, retryable },
				type: 'terminal-provision-failed',
			});
		}
	});

	it('fails closed when a registry association has no owned runtime', async () => {
		const harness = createHarness();
		harness.registry.register({
			bindingId: TerminalBindingIdSchema.parse('foreign-binding'),
			presentationId: terminalPresentationId('presentation-1'),
		});

		await expect(harness.adapter.execute(provisionCommand())).resolves.toMatchObject({
			failure: { code: 'terminal-binding-runtime-unavailable', retryable: false },
			type: 'terminal-provision-failed',
		});
		await expect(harness.adapter.execute(closeCommand())).resolves.toMatchObject({
			failure: { code: 'terminal-binding-runtime-unavailable', retryable: false },
			type: 'terminal-close-failed',
		});
		expect(harness.createBindingLifecycle).not.toHaveBeenCalled();
	});

	it('resumes durable closing bindings before exposing recovered presentations', async () => {
		const bindingId = TerminalBindingIdSchema.parse('binding-closing');
		const closingPresentationId = terminalPresentationId('presentation-closing');
		const closingBinding: TerminalBinding = {
			bindingId,
			creationId: identity.creationId,
			launch: createTerminalLaunch(),
			presentationId: closingPresentationId,
			state: { identity, kind: 'closing' },
		};
		const registry = new DesktopTerminalBindingRegistry();
		const closingLifecycle: DesktopTerminalBindingLifecyclePort = {
			reconcile: vi
				.fn<DesktopTerminalBindingLifecyclePort['reconcile']>()
				.mockResolvedValue({ kind: 'close-completed' }),
			recordExit: vi.fn<DesktopTerminalBindingLifecyclePort['recordExit']>().mockResolvedValue({
				handled: true,
				outcome: { kind: 'closed' },
			}),
			requestClose: vi
				.fn<DesktopTerminalBindingLifecyclePort['requestClose']>()
				.mockResolvedValue({ kind: 'closed' }),
		};
		const adapter = new DesktopTerminalLifecycleAdapter({
			allocateFreshBindingId: () => 'binding-fresh',
			createBindingLifecycle: () => closingLifecycle,
			loadBindings: async () => ({
				document: { bindings: [closingBinding], version: 3 },
				kind: 'available',
			}),
			registry,
			retireProjection: async () => undefined,
			terminalLaunch: createTerminalLaunch(),
		});

		await expect(adapter.initialize()).resolves.toEqual({ kind: 'ready' });
		expect(closingLifecycle.reconcile).toHaveBeenCalledOnce();
		expect(adapter.getRecoveredPresentations()).toEqual([]);
		expect(registry.size).toBe(0);
	});
});

function createTerminalLaunch() {
	return resolveDesktopRuntimeConfiguration({
		applicationPath: '/repo/apps/desktop',
		environment: { SHELL: '/bin/sh' },
		homeDirectory: '/users/orchbee',
		platform: 'linux',
		userDataPath: '/users/orchbee/.config/orchbee',
	}).terminalLaunch;
}

function createDeferred<Value>() {
	let resolve = (_value: Value): void => undefined;
	const promise = new Promise<Value>((nextResolve) => {
		resolve = nextResolve;
	});
	return { promise, resolve };
}
