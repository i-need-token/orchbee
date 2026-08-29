import { describe, expect, it, vi } from 'vitest';
import { SshProjectRootPathSchema } from '@orchbee/ssh-environment-protocol';

import { EnvironmentIdSchema, EnvironmentRecordSchema } from '../../environment';
import {
	DesktopSshEnvironmentConnectionOwner,
	type DesktopSshEnvironmentConnectionOwnerOptions,
	type DesktopSshEnvironmentSupervisorPort,
} from '../DesktopSshEnvironmentConnectionOwner';
import {
	createSshEnvironmentConnectionState,
	observeSshEnvironmentConnection,
} from '../../ssh-environment';

const environmentId = EnvironmentIdSchema.parse(`environment:${'1'.repeat(32)}`);
const canonicalProjectRoot = SshProjectRootPathSchema.parse('/srv/project');
const environment = EnvironmentRecordSchema.parse({
	environmentId,
	name: 'Build machine',
	provider: {
		kind: 'ssh',
		runtimeInstanceId: '2'.repeat(32),
		target: { configurationFilePath: '/tmp/test-ssh-config', destination: 'build-host' },
	},
});
type CreateSupervisor = DesktopSshEnvironmentConnectionOwnerOptions['createSupervisor'];
type RegistryGet = DesktopSshEnvironmentConnectionOwnerOptions['registry']['get'];

describe('Desktop SSH Environment connection owner', () => {
	it('activates one supervisor per durable Environment and pins a copied intent', async () => {
		const registry = {
			get: vi.fn<RegistryGet>().mockResolvedValue({ kind: 'ready', value: environment }),
		};
		const supervisors: DesktopSshEnvironmentSupervisorPort[] = [];
		const createSupervisor = vi.fn<CreateSupervisor>((configuration) => {
			const supervisor = createSupervisorPort(configuration.connectionId);
			supervisors.push(supervisor);
			return supervisor;
		});
		const owner = new DesktopSshEnvironmentConnectionOwner({
			createSupervisor,
			registry,
			supported: true,
		});

		const [first, repeated] = await Promise.all([
			owner.connect(environmentId),
			owner.connect(environmentId),
		]);

		expect(first).toMatchObject({ activation: 'started', kind: 'ready' });
		expect(repeated).toMatchObject({ activation: 'existing', kind: 'ready' });
		expect(registry.get).toHaveBeenCalledOnce();
		expect(createSupervisor).toHaveBeenCalledOnce();
		expect(createSupervisor).toHaveBeenCalledWith(
			{
				connectionId: environmentId,
				runtimeInstanceId: environment.provider.runtimeInstanceId,
				target: environment.provider.target,
			},
			expect.any(Function),
		);
		expect(supervisors[0]?.start).toHaveBeenCalledTimes(2);
		await expect(owner.inspectProjectRoot(environmentId, '/srv/project')).resolves.toEqual({
			kind: 'resolved',
			canonicalPath: canonicalProjectRoot,
		});
		expect(supervisors[0]?.inspectProjectRoot).toHaveBeenCalledWith('/srv/project', undefined);
		await expect(owner.listProjectDirectory(environmentId, null)).resolves.toEqual({
			kind: 'failed',
			reason: 'unavailable',
		});
		expect(supervisors[0]?.listProjectDirectory).toHaveBeenCalledWith(null, undefined);
		await expect(owner.openTerminalHost(environmentId)).resolves.toEqual({
			kind: 'failed',
			reason: 'unavailable',
		});
		expect(supervisors[0]?.openTerminalHost).toHaveBeenCalledOnce();

		await owner.dispose();
		expect(supervisors[0]?.stop).toHaveBeenCalledOnce();
	});

	it('grants one exact transient interaction lease and cancels prompts without an owner', async () => {
		let presentInteraction:
			| Parameters<DesktopSshEnvironmentConnectionOwnerOptions['createSupervisor']>[1]
			| undefined;
		const owner = new DesktopSshEnvironmentConnectionOwner({
			createSupervisor: (configuration, interaction) => {
				presentInteraction = interaction;
				return createSupervisorPort(configuration.connectionId);
			},
			registry: {
				get: vi.fn<RegistryGet>().mockResolvedValue({ kind: 'ready', value: environment }),
			},
			supported: true,
		});
		const interaction = vi
			.fn<NonNullable<typeof presentInteraction>>()
			.mockResolvedValue({ kind: 'submitted', response: 'one-use secret' });
		const claimed = owner.claimInteractionLease(environmentId, interaction);
		expect(owner.claimInteractionLease(environmentId, interaction)).toEqual({ kind: 'busy' });
		await owner.connect(environmentId);
		if (presentInteraction === undefined || claimed.kind !== 'claimed') {
			throw new Error('Expected an active interaction lease.');
		}
		const request = {
			generation: 1,
			handle: 'ssh-prompt-test',
			kind: 'none' as const,
			prompt: 'Password:',
		};
		await expect(presentInteraction(request)).resolves.toEqual({
			kind: 'submitted',
			response: 'one-use secret',
		});
		claimed.lease.release();
		await expect(presentInteraction(request)).resolves.toEqual({ kind: 'cancelled' });
		expect(owner.claimInteractionLease(environmentId, interaction).kind).toBe('claimed');
		await owner.dispose();
		await expect(presentInteraction(request)).resolves.toEqual({ kind: 'cancelled' });
	});

	it('does not activate an absent, unavailable, or unsupported durable intent', async () => {
		const createSupervisor = vi.fn<CreateSupervisor>(() => createSupervisorPort(environmentId));
		const missingRegistry = {
			get: vi.fn<RegistryGet>().mockResolvedValue({ kind: 'ready', value: null }),
		};
		const unavailableRegistry = {
			get: vi
				.fn<RegistryGet>()
				.mockResolvedValue({ kind: 'unavailable', reason: 'not-initialized' }),
		};

		await expect(
			new DesktopSshEnvironmentConnectionOwner({
				createSupervisor,
				registry: missingRegistry,
				supported: true,
			}).connect(environmentId),
		).resolves.toEqual({ kind: 'not-found' });
		await expect(
			new DesktopSshEnvironmentConnectionOwner({
				createSupervisor,
				registry: unavailableRegistry,
				supported: true,
			}).connect(environmentId),
		).resolves.toEqual({ kind: 'unavailable', reason: 'registry-unavailable' });

		const unsupported = new DesktopSshEnvironmentConnectionOwner({
			createSupervisor,
			registry: missingRegistry,
			supported: false,
		});
		await expect(unsupported.connect(environmentId)).resolves.toEqual({
			kind: 'unavailable',
			reason: 'local-platform-unsupported',
		});
		expect(missingRegistry.get).toHaveBeenCalledOnce();
		expect(createSupervisor).not.toHaveBeenCalled();
	});

	it('serializes disconnect with activation and exposes only the active supervisor', async () => {
		const lookup = createDeferred<{ readonly kind: 'ready'; readonly value: typeof environment }>();
		const supervisor = createSupervisorPort(environmentId);
		const owner = new DesktopSshEnvironmentConnectionOwner({
			createSupervisor: () => supervisor,
			registry: { get: vi.fn<RegistryGet>(() => lookup.promise) },
			supported: true,
		});
		const activation = owner.connect(environmentId);
		const disconnection = owner.disconnect(environmentId);

		expect(owner.observation(environmentId)).toEqual({ kind: 'inactive' });
		expect(
			owner.subscribe(
				environmentId,
				vi.fn<Parameters<DesktopSshEnvironmentConnectionOwner['subscribe']>[1]>(),
			),
		).toBeNull();
		lookup.resolve({ kind: 'ready', value: environment });
		await expect(activation).resolves.toMatchObject({ activation: 'started', kind: 'ready' });
		await expect(disconnection).resolves.toEqual({ kind: 'disconnected' });
		expect(supervisor.stop).toHaveBeenCalledOnce();
		expect(owner.observation(environmentId)).toEqual({ kind: 'inactive' });
		await expect(owner.disconnect(environmentId)).resolves.toEqual({ kind: 'inactive' });
	});

	it('fences an in-flight registry read and rejects later activation after disposal', async () => {
		const lookup = createDeferred<{ readonly kind: 'ready'; readonly value: typeof environment }>();
		const createSupervisor = vi.fn<CreateSupervisor>(() => createSupervisorPort(environmentId));
		const owner = new DesktopSshEnvironmentConnectionOwner({
			createSupervisor,
			registry: { get: vi.fn<RegistryGet>(() => lookup.promise) },
			supported: true,
		});
		const activation = owner.connect(environmentId);
		const disposal = owner.dispose();
		lookup.resolve({ kind: 'ready', value: environment });

		await expect(activation).resolves.toEqual({
			kind: 'unavailable',
			reason: 'connection-owner-disposed',
		});
		await expect(disposal).resolves.toBeUndefined();
		expect(createSupervisor).not.toHaveBeenCalled();
		await expect(owner.connect(environmentId)).resolves.toEqual({
			kind: 'unavailable',
			reason: 'connection-owner-disposed',
		});
		await expect(owner.inspectProjectRoot(environmentId, '/srv/project')).resolves.toEqual({
			kind: 'failed',
			reason: 'unavailable',
		});
		await expect(owner.openTerminalHost(environmentId)).resolves.toEqual({
			kind: 'failed',
			reason: 'unavailable',
		});
	});
});

function createSupervisorPort(connectionId: string): DesktopSshEnvironmentSupervisorPort {
	const observation = observeSshEnvironmentConnection(
		createSshEnvironmentConnectionState({
			connectionId,
			target: { configurationFilePath: null, destination: 'build-host' },
		}),
	);
	return {
		inspectProjectRoot: vi
			.fn<DesktopSshEnvironmentSupervisorPort['inspectProjectRoot']>()
			.mockResolvedValue({ kind: 'resolved', canonicalPath: canonicalProjectRoot }),
		listProjectDirectory: vi
			.fn<DesktopSshEnvironmentSupervisorPort['listProjectDirectory']>()
			.mockResolvedValue({ kind: 'failed', reason: 'unavailable' }),
		openTerminalHost: vi
			.fn<DesktopSshEnvironmentSupervisorPort['openTerminalHost']>()
			.mockResolvedValue({ kind: 'failed', reason: 'unavailable' }),
		observation: () => observation,
		start: vi.fn<() => void>(),
		stop: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
		subscribe: vi.fn<DesktopSshEnvironmentSupervisorPort['subscribe']>(() => vi.fn<() => void>()),
	};
}

function createDeferred<Value>() {
	let resolve!: (value: Value) => void;
	const promise = new Promise<Value>((resolver) => {
		resolve = resolver;
	});
	return { promise, resolve };
}
