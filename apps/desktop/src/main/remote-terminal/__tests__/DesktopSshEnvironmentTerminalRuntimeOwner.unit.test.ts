import { HostEpochSchema, type DecodedProtocolEnvelope } from '@orchbee/terminal-protocol';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	type TerminalPresentationCommand,
} from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import { EnvironmentIdSchema, type EnvironmentId } from '../../environment';
import type { HostConnection } from '../../host';
import type { SshTerminalHostConnectionOutcome } from '../../ssh-environment';
import type { DesktopTerminalProjectionLease } from '../../terminal-projection-control';
import { DesktopSshEnvironmentTerminalRuntimeOwner } from '../DesktopSshEnvironmentTerminalRuntimeOwner';

const firstEnvironmentId = EnvironmentIdSchema.parse(`environment:${'1'.repeat(32)}`);
const secondEnvironmentId = EnvironmentIdSchema.parse(`environment:${'2'.repeat(32)}`);
const presentationId = terminalPresentationId('remote-presentation');
const hostEpoch = HostEpochSchema.parse('a'.repeat(32));
const geometry = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 80, rows: 24 };
type HostClosedListener = (reason: Error) => void;
type DesktopSshEnvironmentTerminalRuntimeOwnerOptions = ConstructorParameters<
	typeof DesktopSshEnvironmentTerminalRuntimeOwner
>[0];
type DesktopSshEnvironmentTerminalRuntimeHandle = ReturnType<
	DesktopSshEnvironmentTerminalRuntimeOwnerOptions['createRuntime']
>;
type DesktopSshEnvironmentTerminalObservationHandle = ReturnType<
	DesktopSshEnvironmentTerminalRuntimeOwnerOptions['createObservation']
>;

describe('DesktopSshEnvironmentTerminalRuntimeOwner', () => {
	it('converges concurrent preparation and reuses one runtime per Environment', async () => {
		const host = controlledHost();
		const runtime = runtimeHandle();
		const openTerminalHost = vi.fn<
			(environmentId: EnvironmentId) => Promise<SshTerminalHostConnectionOutcome>
		>(async () => ({ connection: host.connection, kind: 'ready' }));
		const createRuntime = vi.fn<DesktopSshEnvironmentTerminalRuntimeOwnerOptions['createRuntime']>(
			() => runtime.handle,
		);
		const owner = new DesktopSshEnvironmentTerminalRuntimeOwner({
			connections: { openTerminalHost },
			createObservation: () => observationHandle(),
			createRuntime,
		});

		await expect(
			Promise.all([owner.prepare(firstEnvironmentId), owner.prepare(firstEnvironmentId)]),
		).resolves.toEqual([
			{ activation: 'started', kind: 'ready' },
			{ activation: 'existing', kind: 'ready' },
		]);
		expect(openTerminalHost).toHaveBeenCalledOnce();
		expect(createRuntime).toHaveBeenCalledOnce();
		expect(runtime.initialize).toHaveBeenCalledOnce();

		await owner.dispose();
		expect(runtime.dispose).toHaveBeenCalledOnce();
		expect(host.close).toHaveBeenCalledOnce();
	});

	it('loads recovery and installs the current-record envelope fence before runtime initialization', async () => {
		const host = controlledHost();
		const runtime = runtimeHandle();
		const observation = observationHandle();
		const observedEnvelope = {} as DecodedProtocolEnvelope;
		runtime.initialize.mockImplementationOnce(async () => {
			host.emitEnvelope(observedEnvelope);
			return { kind: 'ready' };
		});
		const openTerminalHost = vi.fn<
			(environmentId: EnvironmentId) => Promise<SshTerminalHostConnectionOutcome>
		>(async () => ({ connection: host.connection, kind: 'ready' }));
		const owner = new DesktopSshEnvironmentTerminalRuntimeOwner({
			connections: { openTerminalHost },
			createObservation: () => observation,
			createRuntime: () => runtime.handle,
		});

		await expect(owner.prepare(firstEnvironmentId)).resolves.toEqual({
			activation: 'started',
			kind: 'ready',
		});
		expect(observation.initialize).toHaveBeenCalledOnce();
		expect(openTerminalHost).toHaveBeenCalledOnce();
		expect(observation.attachRuntime).toHaveBeenCalledWith(runtime.handle);
		expect(observation.acceptEnvelope).toHaveBeenCalledWith(observedEnvelope, hostEpoch);
		expect(observation.activate).toHaveBeenCalledOnce();
		expect(vi.mocked(observation.initialize).mock.invocationCallOrder[0]).toBeLessThan(
			openTerminalHost.mock.invocationCallOrder[0] ?? Number.POSITIVE_INFINITY,
		);
		expect(host.onEnvelope.mock.invocationCallOrder[0]).toBeLessThan(
			runtime.initialize.mock.invocationCallOrder[0] ?? Number.POSITIVE_INFINITY,
		);

		const acceptedBeforeClosure = vi.mocked(observation.acceptEnvelope).mock.calls.length;
		host.closeFromRemote();
		host.emitEnvelope(observedEnvelope);
		expect(observation.acceptEnvelope).toHaveBeenCalledTimes(acceptedBeforeClosure);
		await vi.waitFor(() => expect(runtime.dispose).toHaveBeenCalledOnce());
		await owner.dispose();
	});

	it.each(['observation-initialize', 'connection-open'] as const)(
		'releases the pre-runtime observation when %s throws',
		async (failure) => {
			const observation = observationHandle();
			if (failure === 'observation-initialize') {
				vi.mocked(observation.initialize).mockRejectedValueOnce(new Error('recovery unavailable'));
			}
			const openTerminalHost = vi.fn<
				(environmentId: EnvironmentId) => Promise<SshTerminalHostConnectionOutcome>
			>(async () => {
				if (failure === 'connection-open') throw new Error('relay unavailable');
				return { kind: 'failed', reason: 'unavailable' };
			});
			const owner = new DesktopSshEnvironmentTerminalRuntimeOwner({
				connections: { openTerminalHost },
				createObservation: () => observation,
				createRuntime: () => runtimeHandle().handle,
			});

			await expect(owner.prepare(firstEnvironmentId)).resolves.toEqual({
				kind: 'failed',
				reason: 'unavailable',
			});
			expect(observation.dispose).toHaveBeenCalledOnce();
			await owner.dispose();
		},
	);

	it('isolates Environment runtimes and invalidates only the relay that closes', async () => {
		const firstHost = controlledHost();
		const replacementHost = controlledHost();
		const secondHost = controlledHost();
		const firstRuntime = runtimeHandle();
		const replacementRuntime = runtimeHandle();
		const secondRuntime = runtimeHandle();
		const hosts = new Map<EnvironmentId, ReturnType<typeof controlledHost>[]>([
			[firstEnvironmentId, [firstHost, replacementHost]],
			[secondEnvironmentId, [secondHost]],
		]);
		const runtimes = new Map<EnvironmentId, ReturnType<typeof runtimeHandle>[]>([
			[firstEnvironmentId, [firstRuntime, replacementRuntime]],
			[secondEnvironmentId, [secondRuntime]],
		]);
		const owner = new DesktopSshEnvironmentTerminalRuntimeOwner({
			connections: {
				openTerminalHost: async (environmentId) => {
					const host = hosts.get(environmentId)?.shift();
					return host === undefined
						? { kind: 'failed', reason: 'unavailable' }
						: { connection: host.connection, kind: 'ready' };
				},
			},
			createObservation: () => observationHandle(),
			createRuntime: (environmentId) => {
				const runtime = runtimes.get(environmentId)?.shift();
				if (runtime === undefined) throw new Error('Missing test runtime.');
				return runtime.handle;
			},
		});

		await owner.prepare(firstEnvironmentId);
		await owner.prepare(secondEnvironmentId);
		firstHost.closeFromRemote();
		await vi.waitFor(() => expect(firstRuntime.dispose).toHaveBeenCalledOnce());

		await expect(owner.prepare(firstEnvironmentId)).resolves.toEqual({
			activation: 'started',
			kind: 'ready',
		});
		await expect(owner.prepare(secondEnvironmentId)).resolves.toEqual({
			activation: 'existing',
			kind: 'ready',
		});
		expect(secondRuntime.initialize).toHaveBeenCalledOnce();

		await owner.dispose();
		expect(replacementRuntime.dispose).toHaveBeenCalledOnce();
		expect(secondRuntime.dispose).toHaveBeenCalledOnce();
	});

	it('defers retryable or unknown Host outcomes without caching a false settlement', async () => {
		const host = controlledHost();
		const runtime = runtimeHandle();
		const command = provisionCommand();
		runtime.execute
			.mockResolvedValueOnce({
				failure: { code: 'terminal-create-unavailable', retryable: true },
				operationId: command.operationId,
				presentationId: command.presentationId,
				type: 'terminal-provision-failed',
			})
			.mockRejectedValueOnce(new Error('response unknown'))
			.mockResolvedValueOnce({
				operationId: command.operationId,
				presentationId: command.presentationId,
				type: 'terminal-provision-completed',
			});
		const owner = new DesktopSshEnvironmentTerminalRuntimeOwner({
			connections: {
				openTerminalHost: async () => ({ connection: host.connection, kind: 'ready' }),
			},
			createObservation: () => observationHandle(),
			createRuntime: () => runtime.handle,
		});
		await owner.prepare(firstEnvironmentId);

		await expect(owner.execute(firstEnvironmentId, command)).resolves.toEqual({
			kind: 'deferred',
		});
		await expect(owner.execute(firstEnvironmentId, command)).resolves.toEqual({
			kind: 'deferred',
		});
		await expect(owner.execute(firstEnvironmentId, command)).resolves.toEqual({
			event: expect.objectContaining({ type: 'terminal-provision-completed' }),
			kind: 'settled',
		});
		await owner.dispose();
	});

	it('projects only through an already prepared Environment runtime', async () => {
		const host = controlledHost();
		const runtime = runtimeHandle();
		const projection = projectionLease();
		runtime.open.mockResolvedValueOnce(projection.lease);
		const openTerminalHost = vi.fn<
			(environmentId: EnvironmentId) => Promise<SshTerminalHostConnectionOutcome>
		>(async () => ({ connection: host.connection, kind: 'ready' }));
		const owner = new DesktopSshEnvironmentTerminalRuntimeOwner({
			connections: { openTerminalHost },
			createObservation: () => observationHandle(),
			createRuntime: () => runtime.handle,
		});

		await expect(owner.open(firstEnvironmentId, presentationId, geometry)).rejects.toMatchObject({
			cause: { code: 'terminal-projection-unavailable', retryable: true },
		});
		expect(openTerminalHost).not.toHaveBeenCalled();
		expect(runtime.open).not.toHaveBeenCalled();

		await owner.prepare(firstEnvironmentId);
		await expect(
			owner.open(firstEnvironmentId, presentationId, geometry, {
				allowArchivedFallback: true,
				brokerEffectTarget: 'client',
			}),
		).resolves.toBe(projection.lease);
		expect(runtime.open).toHaveBeenCalledWith(presentationId, geometry, {
			allowArchivedFallback: false,
			brokerEffectTarget: 'client',
		});
		expect(projection.dispose).not.toHaveBeenCalled();

		await owner.dispose();
	});

	it('revokes a projection admitted concurrently with relay disconnection', async () => {
		const host = controlledHost();
		const runtime = runtimeHandle();
		const projection = projectionLease();
		let resolveOpen: ((lease: DesktopTerminalProjectionLease) => void) | undefined;
		runtime.open.mockImplementationOnce(
			async () =>
				new Promise<DesktopTerminalProjectionLease>((resolve) => {
					resolveOpen = resolve;
				}),
		);
		const owner = new DesktopSshEnvironmentTerminalRuntimeOwner({
			connections: {
				openTerminalHost: async () => ({ connection: host.connection, kind: 'ready' }),
			},
			createObservation: () => observationHandle(),
			createRuntime: () => runtime.handle,
		});
		await owner.prepare(firstEnvironmentId);

		const opening = owner.open(firstEnvironmentId, presentationId, geometry);
		await vi.waitFor(() => expect(runtime.open).toHaveBeenCalledOnce());
		host.closeFromRemote();
		if (resolveOpen === undefined) throw new Error('Projection open did not reach the runtime.');
		resolveOpen(projection.lease);

		await expect(opening).rejects.toMatchObject({
			cause: { code: 'terminal-projection-unavailable', retryable: true },
		});
		expect(projection.dispose).toHaveBeenCalledOnce();
		await vi.waitFor(() => expect(runtime.dispose).toHaveBeenCalledOnce());
		await owner.dispose();
	});

	it('compensates a failed runtime initialization by closing the relay', async () => {
		const host = controlledHost();
		const runtime = runtimeHandle();
		runtime.initialize.mockResolvedValueOnce({ kind: 'conflict' });
		const owner = new DesktopSshEnvironmentTerminalRuntimeOwner({
			connections: {
				openTerminalHost: async () => ({ connection: host.connection, kind: 'ready' }),
			},
			createObservation: () => observationHandle(),
			createRuntime: () => runtime.handle,
		});

		await expect(owner.prepare(firstEnvironmentId)).resolves.toEqual({
			kind: 'failed',
			reason: 'unavailable',
		});
		expect(runtime.dispose).toHaveBeenCalledOnce();
		expect(host.close).toHaveBeenCalledOnce();
		await owner.dispose();
	});
});

function controlledHost(): {
	readonly close: ReturnType<typeof vi.fn<() => void>>;
	closeFromRemote(): void;
	readonly connection: HostConnection;
	emitEnvelope(envelope: DecodedProtocolEnvelope): void;
	readonly onEnvelope: ReturnType<typeof vi.fn<HostConnection['onEnvelope']>>;
} {
	const listeners = new Set<HostClosedListener>();
	const envelopeListeners = new Set<Parameters<HostConnection['onEnvelope']>[0]>();
	const close = vi.fn<() => void>();
	const onEnvelope = vi.fn<HostConnection['onEnvelope']>((listener) => {
		envelopeListeners.add(listener);
		return () => envelopeListeners.delete(listener);
	});
	const connection = {
		close,
		hostEpoch,
		onClosed: (listener: HostClosedListener) => {
			listeners.add(listener);
			return () => {
				listeners.delete(listener);
			};
		},
		onEnvelope,
	} as unknown as HostConnection;
	return {
		close,
		closeFromRemote: () => {
			for (const listener of listeners) listener(new Error('relay closed'));
		},
		connection,
		emitEnvelope: (envelope) => {
			for (const listener of envelopeListeners) listener(envelope);
		},
		onEnvelope,
	};
}

function runtimeHandle(): {
	readonly dispose: ReturnType<typeof vi.fn<() => Promise<void>>>;
	readonly execute: ReturnType<typeof vi.fn<DesktopSshEnvironmentTerminalRuntimeHandle['execute']>>;
	readonly handle: DesktopSshEnvironmentTerminalRuntimeHandle;
	readonly initialize: ReturnType<
		typeof vi.fn<DesktopSshEnvironmentTerminalRuntimeHandle['initialize']>
	>;
	readonly open: ReturnType<typeof vi.fn<DesktopSshEnvironmentTerminalRuntimeHandle['open']>>;
} {
	const dispose = vi.fn<() => Promise<void>>(async () => undefined);
	const execute = vi.fn<DesktopSshEnvironmentTerminalRuntimeHandle['execute']>(async (command) => ({
		operationId: command.operationId,
		presentationId: command.presentationId,
		type: 'terminal-provision-completed',
	}));
	const initialize = vi.fn<DesktopSshEnvironmentTerminalRuntimeHandle['initialize']>(async () => ({
		kind: 'ready',
	}));
	const open = vi.fn<DesktopSshEnvironmentTerminalRuntimeHandle['open']>();
	return {
		dispose,
		execute,
		handle: {
			commitExitPublication: () => true,
			dispose,
			execute,
			findPresentationByIdentity: () => null,
			getDurableTerminalExit: () => null,
			getTerminalFailure: () => null,
			initialize,
			open,
			recordExitOutcome: async () => ({ kind: 'ignored' }),
			recordFailure: async () => null,
			setIdentityObservedListener: () => undefined,
		},
		initialize,
		open,
	};
}

function observationHandle(): DesktopSshEnvironmentTerminalObservationHandle {
	return {
		acceptEnvelope: vi.fn<DesktopSshEnvironmentTerminalObservationHandle['acceptEnvelope']>(),
		activate: vi
			.fn<DesktopSshEnvironmentTerminalObservationHandle['activate']>()
			.mockResolvedValue(undefined),
		attachRuntime: vi.fn<DesktopSshEnvironmentTerminalObservationHandle['attachRuntime']>(),
		commitReconciledExit: vi
			.fn<DesktopSshEnvironmentTerminalObservationHandle['commitReconciledExit']>()
			.mockReturnValue(true),
		dispose: vi
			.fn<DesktopSshEnvironmentTerminalObservationHandle['dispose']>()
			.mockResolvedValue(undefined),
		initialize: vi
			.fn<DesktopSshEnvironmentTerminalObservationHandle['initialize']>()
			.mockResolvedValue({ kind: 'ready' }),
		retryPending: vi
			.fn<DesktopSshEnvironmentTerminalObservationHandle['retryPending']>()
			.mockResolvedValue(undefined),
		sealInbound: vi.fn<DesktopSshEnvironmentTerminalObservationHandle['sealInbound']>(),
	};
}

function projectionLease(): {
	readonly dispose: ReturnType<typeof vi.fn<() => Promise<void>>>;
	readonly lease: DesktopTerminalProjectionLease;
} {
	const dispose = vi.fn<() => Promise<void>>(async () => undefined);
	return {
		dispose,
		lease: { dispose } as unknown as DesktopTerminalProjectionLease,
	};
}

function provisionCommand(): TerminalPresentationCommand {
	return {
		launchCwd: '/srv/orchbee',
		operationId: terminalPresentationOperationId('remote-operation'),
		presentationId: terminalPresentationId('remote-presentation'),
		type: 'terminal-provision-requested',
	};
}
