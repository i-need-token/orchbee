import {
	CreationIdSchema,
	CanonicalUint64TextSchema,
	HostEpochSchema,
	type TerminalExited,
	type TerminalFailed,
	TerminalPhase,
	TerminalIdSchema,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
} from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import type { TerminalPresentationCommand } from '@orchbee/workbench/terminal-presentation';
import { resolveDesktopRuntimeConfiguration } from '../../composition';
import { createDefaultTerminalProfile } from '../../host';
import { TerminalBindingIdSchema, type TerminalBinding } from '../../recovery';
import { DesktopTerminalBindingRegistry } from '../../terminal-binding-registry';
import type {
	TerminalBindingLifecycleOutcome,
	TerminalCloseOutcome,
	TerminalExitDisposition,
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
): TerminalPresentationCommand {
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
): TerminalPresentationCommand {
	return {
		disposition: 'remove',
		operationId: terminalPresentationOperationId(operation),
		presentationId: terminalPresentationId(presentation),
		type: 'terminal-close-requested',
	};
}

function restoreCommand(
	presentation = 'presentation-1',
	operation = 'restore-1',
	launchCwd: string | null = '/workspace/orchbee',
): TerminalPresentationCommand {
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
	readonly recordExit: ReturnType<typeof vi.fn<DesktopTerminalBindingLifecyclePort['recordExit']>>;
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
		readonly resolveTerminalLaunch?:
			| DesktopTerminalLifecycleAdapterDependencies['resolveTerminalLaunch']
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
	const resolveTerminalLaunch = vi.fn<
		DesktopTerminalLifecycleAdapterDependencies['resolveTerminalLaunch']
	>(options.resolveTerminalLaunch ?? (() => createTerminalLaunch()));
	const adapter = new DesktopTerminalLifecycleAdapter({
		allocateFreshBindingId: options.allocateFreshBindingId ?? (() => 'binding-1'),
		createBindingLifecycle,
		loadBindings: async () => ({ kind: 'missing' }),
		registry,
		resolveTerminalLaunch,
		retireProjection,
	});
	return {
		adapter,
		createBindingLifecycle,
		lifecycle,
		registry,
		resolveTerminalLaunch,
		retireProjection,
	};
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

	it('captures the current launch once and ignores later profile changes on provision retry', async () => {
		const firstLaunch = { ...createTerminalLaunch(), executable: '/bin/first' };
		const secondLaunch = { ...createTerminalLaunch(), executable: '/bin/second' };
		const resolveTerminalLaunch = vi.fn<
			DesktopTerminalLifecycleAdapterDependencies['resolveTerminalLaunch']
		>(() => firstLaunch);
		resolveTerminalLaunch.mockReturnValueOnce(firstLaunch).mockReturnValue(secondLaunch);
		const harness = createHarness({ resolveTerminalLaunch });

		await harness.adapter.execute(provisionCommand());
		await harness.adapter.execute(provisionCommand('presentation-1', 'provision-2'));

		expect(harness.resolveTerminalLaunch).toHaveBeenCalledOnce();
		expect(harness.createBindingLifecycle.mock.calls[0]?.[1]).toMatchObject({
			executable: '/bin/first',
		});
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
		const resolveTerminalLaunch = vi.fn<
			DesktopTerminalLifecycleAdapterDependencies['resolveTerminalLaunch']
		>(() => createTerminalLaunch());
		const retained = new DesktopTerminalLifecycleAdapter({
			allocateFreshBindingId,
			createBindingLifecycle: () => retainedLifecycle,
			loadBindings: async () => ({
				document: { bindings: [retainedBinding], version: 3 },
				kind: 'available',
			}),
			registry: new DesktopTerminalBindingRegistry(),
			resolveTerminalLaunch,
			retireProjection: async () => undefined,
		});

		await expect(retained.execute(restoreCommand())).resolves.toEqual({
			operationId: 'restore-1',
			presentationId: 'presentation-1',
			type: 'terminal-restore-completed',
		});
		expect(retainedLifecycle.reconcile).toHaveBeenCalledOnce();
		expect(allocateFreshBindingId).not.toHaveBeenCalled();
		expect(resolveTerminalLaunch).not.toHaveBeenCalled();

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

	it('recovers a failed terminal from Host inventory as observable read-only state', async () => {
		const failedSummary = createFailedSummary();
		const retainedLifecycle = createLifecycle(async () => ({
			identity,
			kind: 'recovered',
			summary: failedSummary,
		}));
		const retainedBinding: TerminalBinding = {
			bindingId: TerminalBindingIdSchema.parse('binding-retained'),
			creationId: identity.creationId,
			launch: { ...createTerminalLaunch(), currentDirectory: '/workspace/orchbee' },
			presentationId: terminalPresentationId('presentation-1'),
			state: { identity, kind: 'retained' },
		};
		const adapter = new DesktopTerminalLifecycleAdapter({
			allocateFreshBindingId: () => 'binding-fresh',
			createBindingLifecycle: () => retainedLifecycle,
			loadBindings: async () => ({
				document: { bindings: [retainedBinding], version: 3 },
				kind: 'available',
			}),
			registry: new DesktopTerminalBindingRegistry(),
			resolveTerminalLaunch: () => createTerminalLaunch(),
			retireProjection: async () => undefined,
		});

		await expect(adapter.execute(restoreCommand())).resolves.toMatchObject({
			type: 'terminal-restore-completed',
		});
		const failure: TerminalFailed = {
			failure: failedSummary.failure,
			finalRevision: failedSummary.revision,
			identity,
			process: failedSummary.process,
		};
		expect(adapter.getTerminalFailure(terminalPresentationId('presentation-1'))).toEqual(failure);
		expect(adapter.getProjectionTarget(terminalPresentationId('presentation-1'))).toEqual({
			identity,
			readOnly: true,
			startRequired: false,
			terminalOutcome: { event: failure, kind: 'failed' },
			theme: failedSummary.theme,
		});
	});

	it('keeps accepted startup monotonic across a stale awaiting-start reconciliation', async () => {
		const awaitingStartSummary = createAwaitingStartSummary();
		const retainedLifecycle = createLifecycle(async () => ({
			identity,
			kind: 'recovered',
			summary: awaitingStartSummary,
		}));
		const retainedBinding: TerminalBinding = {
			bindingId: TerminalBindingIdSchema.parse('binding-awaiting-start'),
			creationId: identity.creationId,
			launch: { ...createTerminalLaunch(), currentDirectory: '/workspace/orchbee' },
			presentationId: terminalPresentationId('presentation-1'),
			state: { identity, kind: 'retained' },
		};
		const adapter = new DesktopTerminalLifecycleAdapter({
			allocateFreshBindingId: () => 'binding-fresh',
			createBindingLifecycle: () => retainedLifecycle,
			loadBindings: async () => ({
				document: { bindings: [retainedBinding], version: 3 },
				kind: 'available',
			}),
			registry: new DesktopTerminalBindingRegistry(),
			resolveTerminalLaunch: () => createTerminalLaunch(),
			retireProjection: async () => undefined,
		});
		const presentationId = terminalPresentationId('presentation-1');

		await expect(adapter.execute(restoreCommand())).resolves.toMatchObject({
			type: 'terminal-restore-completed',
		});
		expect(adapter.getProjectionTarget(presentationId)).toMatchObject({ startRequired: true });
		expect(
			adapter.markTerminalStarted(presentationId, {
				...identity,
				terminalId: TerminalIdSchema.parse('44'.repeat(16)),
			}),
		).toBe(false);
		expect(adapter.markTerminalStarted(presentationId, identity)).toBe(true);
		expect(adapter.getProjectionTarget(presentationId)).toMatchObject({ startRequired: false });

		await expect(
			adapter.execute(restoreCommand('presentation-1', 'restore-2')),
		).resolves.toMatchObject({ type: 'terminal-restore-completed' });
		expect(adapter.getProjectionTarget(presentationId)).toMatchObject({ startRequired: false });
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
			resolveTerminalLaunch: () => createTerminalLaunch(),
			retireProjection: async () => undefined,
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

	it('observes a Host exit before an in-flight lifecycle operation completes', async () => {
		const reconcile = createDeferred<TerminalBindingLifecycleOutcome>();
		const lifecycle = createLifecycle();
		lifecycle.reconcile
			.mockResolvedValueOnce({ identity, kind: 'provisioning', summary: null })
			.mockImplementationOnce(() => reconcile.promise);
		const harness = createHarness({ createBindingLifecycle: () => lifecycle });
		const presentationId = terminalPresentationId('presentation-1');
		await harness.adapter.execute(provisionCommand());

		const inFlight = harness.adapter.execute(provisionCommand('presentation-1', 'provision-2'));
		await vi.waitFor(() => expect(lifecycle.reconcile).toHaveBeenCalledTimes(2));
		const event: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('2'),
			identity,
		};

		const exit = harness.adapter.recordExit(event);
		expect(harness.adapter.getProjectionTarget(presentationId)).toMatchObject({
			readOnly: true,
			terminalOutcome: { event: event, kind: 'exited' },
		});
		expect(lifecycle.recordExit).not.toHaveBeenCalled();

		reconcile.resolve({ identity, kind: 'provisioning', summary: null });
		await expect(inFlight).resolves.toMatchObject({ type: 'terminal-provision-completed' });
		await expect(exit).resolves.toBe(presentationId);
	});

	it('associates a create response before the store save and replays its buffered exit', async () => {
		const reconcile = createDeferred<TerminalBindingLifecycleOutcome>();
		const lifecycle = createLifecycle(() => reconcile.promise);
		let associateIdentity: ((nextIdentity: typeof identity) => void) | undefined;
		const harness = createHarness({
			createBindingLifecycle: (_bindingId, _launch, _presentationId, onIdentityObserved) => {
				associateIdentity = onIdentityObserved;
				return lifecycle;
			},
		});
		const presentationId = terminalPresentationId('presentation-1');
		const provision = harness.adapter.execute(provisionCommand());
		await vi.waitFor(() => expect(lifecycle.reconcile).toHaveBeenCalledOnce());

		const event: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('3'),
			identity,
		};
		const exit = harness.adapter.recordExit(event);
		expect(harness.adapter.getProjectionTarget(presentationId)).toBeNull();
		expect(lifecycle.recordExit).not.toHaveBeenCalled();

		associateIdentity?.(identity);
		expect(harness.adapter.findPresentationByIdentity(identity)).toBe(presentationId);
		expect(harness.adapter.getDurableTerminalExit(presentationId)).toBeNull();
		reconcile.resolve({ identity, kind: 'provisioning', summary: null });

		await expect(provision).resolves.toMatchObject({ type: 'terminal-provision-completed' });
		await expect(exit).resolves.toBe(presentationId);
		expect(lifecycle.recordExit).toHaveBeenCalledWith(event);
		expect(harness.adapter.getProjectionTarget(presentationId)).toMatchObject({
			readOnly: true,
			terminalOutcome: { event: event, kind: 'exited' },
		});
		expect(harness.adapter.getDurableTerminalExit(presentationId)).toEqual(event);
	});

	it('replays a buffered runtime failure without letting a stale reconcile clear it', async () => {
		const reconcile = createDeferred<TerminalBindingLifecycleOutcome>();
		const lifecycle = createLifecycle(() => reconcile.promise);
		let associateIdentity: ((nextIdentity: typeof identity) => void) | undefined;
		const harness = createHarness({
			createBindingLifecycle: (_bindingId, _launch, _presentationId, onIdentityObserved) => {
				associateIdentity = onIdentityObserved;
				return lifecycle;
			},
		});
		const presentationId = terminalPresentationId('presentation-1');
		const provision = harness.adapter.execute(provisionCommand());
		await vi.waitFor(() => expect(lifecycle.reconcile).toHaveBeenCalledOnce());
		const event: TerminalFailed = {
			failure: { kind: 'pty-read', stage: 'read-output' },
			finalRevision: CanonicalUint64TextSchema.parse('7'),
			identity,
			process: { kind: 'observed', result: { code: 1, kind: 'exit-code' } },
		};
		const failure = harness.adapter.recordFailure(event);

		associateIdentity?.(identity);
		reconcile.resolve({ identity, kind: 'provisioning', summary: null });

		await expect(provision).resolves.toMatchObject({ type: 'terminal-provision-completed' });
		await expect(failure).resolves.toBe(presentationId);
		expect(harness.adapter.getTerminalFailure(presentationId)).toEqual(event);
		expect(harness.adapter.getProjectionTarget(presentationId)).toMatchObject({
			readOnly: true,
			terminalOutcome: { event, kind: 'failed' },
		});
		expect(lifecycle.recordExit).not.toHaveBeenCalled();
	});

	it('fails reconciliation when inventory contradicts an observed runtime failure', async () => {
		const reconcile = createDeferred<TerminalBindingLifecycleOutcome>();
		const lifecycle = createLifecycle(() => reconcile.promise);
		let associateIdentity: ((nextIdentity: typeof identity) => void) | undefined;
		const harness = createHarness({
			createBindingLifecycle: (_bindingId, _launch, _presentationId, onIdentityObserved) => {
				associateIdentity = onIdentityObserved;
				return lifecycle;
			},
		});
		const presentationId = terminalPresentationId('presentation-1');
		const provision = harness.adapter.execute(provisionCommand());
		await vi.waitFor(() => expect(lifecycle.reconcile).toHaveBeenCalledOnce());
		const event: TerminalFailed = {
			failure: { kind: 'pty-read', stage: 'read-output' },
			finalRevision: CanonicalUint64TextSchema.parse('7'),
			identity,
			process: { kind: 'observed', result: { code: 1, kind: 'exit-code' } },
		};
		const observed = harness.adapter.recordFailure(event);

		associateIdentity?.(identity);
		reconcile.resolve({ identity, kind: 'recovered', summary: createExitedSummary() });

		await expect(provision).resolves.toMatchObject({
			failure: { code: 'terminal-outcome-conflict', retryable: false },
			type: 'terminal-provision-failed',
		});
		await expect(observed).resolves.toBe(presentationId);
		expect(harness.adapter.getTerminalFailure(presentationId)).toEqual(event);
		expect(harness.adapter.getProjectionTarget(presentationId)).toMatchObject({
			readOnly: true,
			terminalOutcome: { event, kind: 'failed' },
		});
	});

	it('returns a typed association-pending outcome without holding the Main retry owner', async () => {
		const reconcile = createDeferred<TerminalBindingLifecycleOutcome>();
		const lifecycle = createLifecycle(() => reconcile.promise);
		let associateIdentity: ((nextIdentity: typeof identity) => void) | undefined;
		const harness = createHarness({
			createBindingLifecycle: (_bindingId, _launch, _presentationId, onIdentityObserved) => {
				associateIdentity = onIdentityObserved;
				return lifecycle;
			},
		});
		const provision = harness.adapter.execute(provisionCommand());
		await vi.waitFor(() => expect(lifecycle.reconcile).toHaveBeenCalledOnce());
		const event: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('3'),
			identity: {
				creationId: CreationIdSchema.parse('55'.repeat(16)),
				hostEpoch: identity.hostEpoch,
				terminalId: TerminalIdSchema.parse('66'.repeat(16)),
			},
		};

		await expect(harness.adapter.recordExitOutcome(event)).resolves.toEqual({
			code: 'terminal-exit-association-pending',
			kind: 'retryable',
			reason: 'association',
		});
		expect(lifecycle.recordExit).not.toHaveBeenCalled();

		associateIdentity?.(event.identity);
		reconcile.resolve({ identity: event.identity, kind: 'provisioning', summary: null });
		await expect(provision).resolves.toMatchObject({ type: 'terminal-provision-completed' });
		await vi.waitFor(() => expect(lifecycle.recordExit).toHaveBeenCalledWith(event));
	});

	it('preserves admitted exits and applies explicit identity backpressure at overflow', async () => {
		const reconcile = createDeferred<TerminalBindingLifecycleOutcome>();
		const lifecycle = createLifecycle(() => reconcile.promise);
		let associateIdentity: ((nextIdentity: typeof identity) => void) | undefined;
		const harness = createHarness({
			createBindingLifecycle: (_bindingId, _launch, _presentationId, onIdentityObserved) => {
				associateIdentity = onIdentityObserved;
				return lifecycle;
			},
		});
		const provision = harness.adapter.execute(provisionCommand());
		await vi.waitFor(() => expect(lifecycle.reconcile).toHaveBeenCalledOnce());

		const bufferedEvents: TerminalExited[] = Array.from({ length: 64 }, (_, index) => ({
			exit: { code: index, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse(String(index + 1)),
			identity:
				index === 0
					? identity
					: {
							creationId: CreationIdSchema.parse((index + 10).toString(16).padStart(32, '0')),
							hostEpoch: identity.hostEpoch,
							terminalId: TerminalIdSchema.parse((index + 10).toString(16).padStart(32, '0')),
						},
		}));
		const buffered = bufferedEvents.map((event) => harness.adapter.recordExit(event));
		const overflowEvent: TerminalExited = {
			exit: { code: 64, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('65'),
			identity: {
				creationId: CreationIdSchema.parse('4a'.repeat(16)),
				hostEpoch: identity.hostEpoch,
				terminalId: TerminalIdSchema.parse('4b'.repeat(16)),
			},
		};
		const overflow = harness.adapter.recordExit(overflowEvent);
		await expect(overflow).rejects.toMatchObject({ code: 'terminal-exit-buffer-full' });
		await expect(harness.adapter.recordExitOutcome(overflowEvent)).resolves.toEqual({
			code: 'terminal-exit-buffer-full',
			kind: 'retryable',
			reason: 'buffer-full',
		});
		expect(lifecycle.recordExit).not.toHaveBeenCalled();

		associateIdentity?.(identity);
		reconcile.resolve({ identity, kind: 'provisioning', summary: null });
		await expect(provision).resolves.toMatchObject({ type: 'terminal-provision-completed' });
		await expect(buffered[0]).resolves.toBe(terminalPresentationId('presentation-1'));
		expect(lifecycle.recordExit).toHaveBeenCalledWith(bufferedEvents[0]);

		harness.adapter.dispose();
		await expect(Promise.all(buffered)).resolves.toHaveLength(64);
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

	it('suppresses and retries exit publication after close retirement conflicts', async () => {
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
		const event: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('2'),
			identity,
		};

		await expect(harness.adapter.recordExit(event)).resolves.toBeNull();
		expect(harness.lifecycle.recordExit).not.toHaveBeenCalled();

		harness.registry.remove(presentationId, TerminalBindingIdSchema.parse('replacement-binding'));
		harness.registry.register({
			bindingId: TerminalBindingIdSchema.parse('binding-1'),
			presentationId,
		});
		await expect(harness.adapter.recordExit(event)).resolves.toBeNull();
		expect(harness.registry.size).toBe(0);
		expect(harness.lifecycle.recordExit).not.toHaveBeenCalled();
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

	it('persists monotonic exit revisions until Main explicitly commits publication', async () => {
		const harness = createHarness();
		await harness.adapter.execute(provisionCommand());
		const first: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('2'),
			identity,
		};
		const revised: TerminalExited = {
			...first,
			finalRevision: CanonicalUint64TextSchema.parse('3'),
		};

		await expect(harness.adapter.recordExit(first)).resolves.toBe(
			terminalPresentationId('presentation-1'),
		);
		await expect(harness.adapter.recordExit(first)).resolves.toBe(
			terminalPresentationId('presentation-1'),
		);
		expect(harness.adapter.commitExitPublication(first)).toBe(true);
		await expect(harness.adapter.recordExit(first)).resolves.toBeNull();
		await expect(harness.adapter.recordExit(revised)).resolves.toBe(
			terminalPresentationId('presentation-1'),
		);
		expect(harness.adapter.commitExitPublication(revised)).toBe(true);
		await expect(harness.adapter.recordExit(revised)).resolves.toBeNull();
		expect(harness.lifecycle.recordExit).toHaveBeenCalledTimes(2);
		expect(
			harness.adapter.getProjectionTarget(terminalPresentationId('presentation-1')),
		).toMatchObject({
			readOnly: true,
			terminalOutcome: { event: revised, kind: 'exited' },
		});
	});

	it('serializes concurrent duplicate exit deliveries until Main commits them', async () => {
		const harness = createHarness();
		await harness.adapter.execute(provisionCommand());
		const event: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('2'),
			identity,
		};
		const firstRecord = createDeferred<{ readonly handled: false }>();
		harness.lifecycle.recordExit.mockImplementationOnce(() => firstRecord.promise);

		const first = harness.adapter.recordExit(event);
		const second = harness.adapter.recordExit(event);
		await vi.waitFor(() => expect(harness.lifecycle.recordExit).toHaveBeenCalledOnce());
		firstRecord.resolve({ handled: false });

		await expect(first).resolves.toBe(terminalPresentationId('presentation-1'));
		await expect(second).resolves.toBe(terminalPresentationId('presentation-1'));
		expect(harness.adapter.commitExitPublication(event)).toBe(true);
		await expect(harness.adapter.recordExit(event)).resolves.toBeNull();
		expect(harness.lifecycle.recordExit).toHaveBeenCalledOnce();
	});

	it('keeps a higher observed revision authoritative while an older exit is in flight', async () => {
		const harness = createHarness();
		await harness.adapter.execute(provisionCommand());
		const older: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('2'),
			identity,
		};
		const newer: TerminalExited = {
			...older,
			finalRevision: CanonicalUint64TextSchema.parse('3'),
		};
		const firstRecord = createDeferred<TerminalExitDisposition>();
		harness.lifecycle.recordExit.mockImplementationOnce(() => firstRecord.promise);

		const first = harness.adapter.recordExit(older);
		await vi.waitFor(() => expect(harness.lifecycle.recordExit).toHaveBeenCalledOnce());
		const second = harness.adapter.recordExit(newer);
		expect(
			harness.adapter.getProjectionTarget(terminalPresentationId('presentation-1')),
		).toMatchObject({
			readOnly: true,
			terminalOutcome: { event: newer, kind: 'exited' },
		});

		firstRecord.resolve({ handled: false });
		await expect(first).resolves.toBeNull();
		await expect(second).resolves.toBe(terminalPresentationId('presentation-1'));
		expect(harness.adapter.commitExitPublication(newer)).toBe(true);
		await expect(harness.adapter.recordExit(newer)).resolves.toBeNull();
		expect(harness.lifecycle.recordExit).toHaveBeenNthCalledWith(1, older);
		expect(harness.lifecycle.recordExit).toHaveBeenNthCalledWith(2, newer);
	});

	it('keeps an exit revision retryable when durable recording fails', async () => {
		const harness = createHarness();
		await harness.adapter.execute(provisionCommand());
		const event: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('2'),
			identity,
		};
		harness.lifecycle.recordExit.mockRejectedValueOnce(new Error('transient store failure'));

		await expect(harness.adapter.recordExit(event)).rejects.toThrow('transient store failure');
		expect(
			harness.adapter.getProjectionTarget(terminalPresentationId('presentation-1')),
		).toMatchObject({
			readOnly: true,
			terminalOutcome: { event, kind: 'exited' },
		});
		await expect(harness.adapter.recordExit(event)).resolves.toBe(
			terminalPresentationId('presentation-1'),
		);
		expect(harness.adapter.commitExitPublication(event)).toBe(true);
		await expect(harness.adapter.recordExit(event)).resolves.toBeNull();
		expect(harness.lifecycle.recordExit).toHaveBeenCalledTimes(2);
	});

	it('retries pending exit recording without republishing the presentation', async () => {
		const harness = createHarness();
		await harness.adapter.execute(provisionCommand());
		const event: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('2'),
			identity,
		};
		const firstRecord = createDeferred<TerminalExitDisposition>();
		harness.lifecycle.recordExit
			.mockReset()
			.mockImplementationOnce(() => firstRecord.promise)
			.mockResolvedValueOnce({ handled: false });

		const first = harness.adapter.recordExit(event);
		await vi.waitFor(() => expect(harness.lifecycle.recordExit).toHaveBeenCalledOnce());
		expect(
			harness.adapter.getProjectionTarget(terminalPresentationId('presentation-1')),
		).toMatchObject({
			readOnly: true,
			terminalOutcome: { event, kind: 'exited' },
		});
		firstRecord.resolve({
			handled: true,
			outcome: { kind: 'pending', reason: 'terminal-not-running' },
		});

		await expect(first).resolves.toBeNull();
		await expect(harness.adapter.recordExit(event)).resolves.toBe(
			terminalPresentationId('presentation-1'),
		);
		expect(harness.adapter.commitExitPublication(event)).toBe(true);
		await expect(harness.adapter.recordExit(event)).resolves.toBeNull();
		await expect(
			harness.adapter.recordExit({ ...event, finalRevision: CanonicalUint64TextSchema.parse('1') }),
		).resolves.toBeNull();
		expect(harness.lifecycle.recordExit).toHaveBeenCalledTimes(2);
		expect(
			harness.adapter.getProjectionTarget(terminalPresentationId('presentation-1')),
		).toMatchObject({
			readOnly: true,
			terminalOutcome: { event, kind: 'exited' },
		});
	});

	it('waits for durable persistence before publishing a store-unavailable exit', async () => {
		const harness = createHarness();
		await harness.adapter.execute(provisionCommand());
		const event: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('2'),
			identity,
		};
		harness.lifecycle.recordExit
			.mockResolvedValueOnce({
				handled: true,
				outcome: { kind: 'store-unavailable', reason: 'read-failed' },
			})
			.mockResolvedValueOnce({ handled: false });

		await expect(harness.adapter.recordExit(event)).resolves.toBeNull();
		expect(
			harness.adapter.getProjectionTarget(terminalPresentationId('presentation-1')),
		).toMatchObject({
			readOnly: true,
			terminalOutcome: { event, kind: 'exited' },
		});
		await expect(harness.adapter.recordExit(event)).resolves.toBe(
			terminalPresentationId('presentation-1'),
		);
		expect(harness.adapter.commitExitPublication(event)).toBe(true);
		await expect(harness.adapter.recordExit(event)).resolves.toBeNull();
		await expect(
			harness.adapter.recordExit({ ...event, finalRevision: CanonicalUint64TextSchema.parse('1') }),
		).resolves.toBeNull();
		expect(harness.lifecycle.recordExit).toHaveBeenCalledTimes(2);
	});

	it('keeps recovery persistence retryable when handled-false reports store unavailability', async () => {
		const harness = createHarness();
		await harness.adapter.execute(provisionCommand());
		const presentationId = terminalPresentationId('presentation-1');
		const event: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('2'),
			identity,
		};
		harness.lifecycle.recordExit
			.mockResolvedValueOnce({
				handled: false,
				durable: false,
				outcome: { kind: 'store-unavailable', reason: 'read-failed' },
			})
			.mockResolvedValueOnce({ handled: false });

		await expect(harness.adapter.recordExit(event)).resolves.toBeNull();
		expect(harness.adapter.getProjectionTarget(presentationId)).toMatchObject({
			readOnly: true,
			terminalOutcome: { event: event, kind: 'exited' },
		});
		await expect(harness.adapter.recordExit(event)).resolves.toBe(presentationId);
		expect(harness.adapter.commitExitPublication(event)).toBe(true);
		await expect(harness.adapter.recordExit(event)).resolves.toBeNull();
		expect(harness.lifecycle.recordExit).toHaveBeenCalledTimes(2);
	});

	it('retires a handled closed exit before suppressing publication', async () => {
		const harness = createHarness();
		await harness.adapter.execute(provisionCommand());
		harness.lifecycle.recordExit.mockResolvedValue({
			handled: true,
			outcome: { kind: 'closed' },
		});
		const event: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('2'),
			identity,
		};

		await expect(harness.adapter.recordExit(event)).resolves.toBeNull();
		expect(harness.registry.size).toBe(0);
		expect(
			harness.adapter.getProjectionTarget(terminalPresentationId('presentation-1')),
		).toBeNull();
	});

	it('reports a close-consumed exit as ignored after successful retirement', async () => {
		const harness = createHarness();
		await harness.adapter.execute(provisionCommand());
		harness.lifecycle.recordExit.mockResolvedValue({
			handled: true,
			outcome: { kind: 'closed' },
		});
		const event: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('2'),
			identity,
		};

		await expect(harness.adapter.recordExitOutcome(event)).resolves.toEqual({
			kind: 'ignored',
			reason: 'closed',
		});
		expect(harness.registry.size).toBe(0);
	});

	it('suppresses a closed exit when registry retirement fails and retries retirement later', async () => {
		const harness = createHarness();
		await harness.adapter.execute(provisionCommand());
		const presentationId = terminalPresentationId('presentation-1');
		harness.registry.remove(presentationId, TerminalBindingIdSchema.parse('binding-1'));
		harness.registry.register({
			bindingId: TerminalBindingIdSchema.parse('replacement-binding'),
			presentationId,
		});
		harness.lifecycle.recordExit.mockResolvedValue({
			handled: true,
			outcome: { kind: 'closed' },
		});
		const event: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('2'),
			identity,
		};

		await expect(harness.adapter.recordExit(event)).resolves.toBeNull();
		expect(harness.registry.getByPresentationId(presentationId)).toEqual({
			bindingId: 'replacement-binding',
			presentationId,
		});
		expect(harness.adapter.getProjectionTarget(presentationId)).toMatchObject({
			terminalOutcome: { event: event, kind: 'exited' },
		});

		harness.registry.remove(presentationId, TerminalBindingIdSchema.parse('replacement-binding'));
		harness.registry.register({
			bindingId: TerminalBindingIdSchema.parse('binding-1'),
			presentationId,
		});
		await expect(harness.adapter.recordExit(event)).resolves.toBeNull();
		expect(harness.registry.size).toBe(0);
		expect(harness.adapter.getProjectionTarget(presentationId)).toBeNull();
		expect(harness.lifecycle.recordExit).toHaveBeenCalledOnce();
	});

	it('observes an exit read-only while close retirement is pending without recording it', async () => {
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
		expect(harness.adapter.getProjectionTarget(presentationId)).toMatchObject({
			readOnly: true,
			terminalOutcome: null,
		});

		const event: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('2'),
			identity,
		};
		await expect(harness.adapter.recordExit(event)).resolves.toBeNull();
		expect(harness.adapter.getProjectionTarget(presentationId)).toMatchObject({
			readOnly: true,
			terminalOutcome: { event: event, kind: 'exited' },
		});
		expect(harness.lifecycle.recordExit).not.toHaveBeenCalled();

		harness.registry.remove(presentationId, TerminalBindingIdSchema.parse('replacement-binding'));
		harness.registry.register({
			bindingId: TerminalBindingIdSchema.parse('binding-1'),
			presentationId,
		});
		await expect(harness.adapter.recordExit(event)).resolves.toBeNull();
		expect(harness.registry.size).toBe(0);
		expect(harness.lifecycle.recordExit).not.toHaveBeenCalled();
	});

	it('does not claim an exited summary was published until Main commits it', async () => {
		const provisionLifecycle = createLifecycle();
		provisionLifecycle.reconcile
			.mockResolvedValueOnce({ identity, kind: 'provisioning', summary: null })
			.mockResolvedValueOnce({
				identity,
				kind: 'recovered',
				summary: createExitedSummary(),
			});
		const provision = createHarness({ createBindingLifecycle: () => provisionLifecycle });
		await provision.adapter.execute(provisionCommand());
		await provision.adapter.execute(provisionCommand('presentation-1', 'provision-2'));

		const restoreLifecycle = createLifecycle(async () => ({
			identity,
			kind: 'recovered',
			summary: createExitedSummary(),
		}));
		const retainedBinding: TerminalBinding = {
			bindingId: TerminalBindingIdSchema.parse('binding-retained'),
			creationId: identity.creationId,
			launch: { ...createTerminalLaunch(), currentDirectory: '/workspace/orchbee' },
			presentationId: terminalPresentationId('presentation-1'),
			state: { identity, kind: 'retained' },
		};
		const restore = new DesktopTerminalLifecycleAdapter({
			allocateFreshBindingId: () => 'binding-fresh',
			createBindingLifecycle: () => restoreLifecycle,
			loadBindings: async () => ({
				document: { bindings: [retainedBinding], version: 3 },
				kind: 'available',
			}),
			registry: new DesktopTerminalBindingRegistry(),
			resolveTerminalLaunch: () => createTerminalLaunch(),
			retireProjection: async () => undefined,
		});
		await restore.execute(restoreCommand());

		const recoveredExit: TerminalExited = {
			exit: { code: 0, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('4'),
			identity,
		};
		await expect(provision.adapter.recordExit(recoveredExit)).resolves.toBe(
			terminalPresentationId('presentation-1'),
		);
		await expect(restore.recordExit(recoveredExit)).resolves.toBe(
			terminalPresentationId('presentation-1'),
		);
		expect(provision.adapter.commitExitPublication(recoveredExit)).toBe(true);
		expect(restore.commitExitPublication(recoveredExit)).toBe(true);
		await expect(provision.adapter.recordExit(recoveredExit)).resolves.toBeNull();
		await expect(restore.recordExit(recoveredExit)).resolves.toBeNull();
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
			state: { disposition: 'remove', identity, kind: 'closing' },
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
			resolveTerminalLaunch: () => createTerminalLaunch(),
			retireProjection: async () => undefined,
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
		posixRuntimeBaseDirectory: '/run/user/501',
		terminalHostName: 'orchbee-host',
		userDataPath: '/users/orchbee/.config/orchbee',
	}).terminalLaunch;
}

function createExitedSummary(): TerminalSummary {
	return {
		columns: 80,
		configuration: { scrollbackBytes: 10_000_000 },
		configurationRevision: CanonicalUint64TextSchema.parse('1'),
		exit: { code: 0, kind: 'exit-code' },
		failure: null,
		failureContainment: null,
		identity,
		phase: TerminalPhase.exited,
		process: null,
		revision: CanonicalUint64TextSchema.parse('4'),
		rows: 24,
		terminateRequested: false,
		theme: createInitialTheme(),
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
		theme: createInitialTheme(),
	};
}

function createFailedSummary(): Extract<
	TerminalSummary,
	{ readonly phase: typeof TerminalPhase.failed }
> {
	return {
		columns: 80,
		configuration: { scrollbackBytes: 10_000_000 },
		configurationRevision: CanonicalUint64TextSchema.parse('1'),
		exit: null,
		failure: { kind: 'pty-read', stage: 'read-output' },
		failureContainment: null,
		identity,
		phase: TerminalPhase.failed,
		process: { kind: 'observed', result: { code: 1, kind: 'exit-code' } },
		revision: CanonicalUint64TextSchema.parse('7'),
		rows: 24,
		terminateRequested: true,
		theme: createInitialTheme(),
	};
}

function createInitialTheme(): TerminalSummary['theme'] {
	const profile = createDefaultTerminalProfile();
	return {
		operationId: null,
		revision: CanonicalUint64TextSchema.parse('0'),
		terminal: {
			colorScheme: profile.colorScheme,
			defaultBackgroundRgb: profile.defaultBackgroundRgb,
			defaultCursorRgb: profile.defaultCursorRgb,
			defaultForegroundRgb: profile.defaultForegroundRgb,
			palette: profile.palette,
		},
	};
}

function createDeferred<Value>() {
	let resolve = (_value: Value): void => undefined;
	const promise = new Promise<Value>((nextResolve) => {
		resolve = nextResolve;
	});
	return { promise, resolve };
}
