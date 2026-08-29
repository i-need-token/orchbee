import { EventEmitter } from 'node:events';

import {
	SshEnvironmentRuntimeInstanceIdSchema,
	SshProjectRootPathSchema,
} from '@orchbee/ssh-environment-protocol';
import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import { describe, expect, it, vi } from 'vitest';

import {
	EnvironmentIdSchema,
	EnvironmentRecordSchema,
	type DesktopEnvironmentRegistry,
	type EnvironmentId,
} from '../../environment';
import {
	createSshEnvironmentConnectionState,
	observeSshEnvironmentConnection,
	transitionSshEnvironmentConnection,
	type OpenSshTargetResolutionResult,
	type SshEnvironmentConnectionObservation,
	type SshEnvironmentSupervisorInteractionRequest,
} from '../../ssh-environment';
import {
	DesktopSshProjectEnrollmentRuntime,
	type DesktopSshProjectEnrollmentOperation,
	type DesktopSshProjectEnrollmentRuntimeOptions,
} from '../DesktopSshProjectEnrollmentRuntime';

type EnrollmentConnections = DesktopSshProjectEnrollmentRuntimeOptions['connections'];

const environmentId = EnvironmentIdSchema.parse(`environment:${'1'.repeat(32)}`);
const environment = EnvironmentRecordSchema.parse({
	environmentId,
	name: 'Authoritative build machine',
	provider: {
		kind: 'ssh',
		runtimeInstanceId: SshEnvironmentRuntimeInstanceIdSchema.parse('2'.repeat(32)),
		target: { configurationFilePath: null, destination: 'build-host' },
	},
});

describe('Desktop SSH Project enrollment runtime', () => {
	it('resolves before durability, connects with an exact prompt lease, inspects, then commits', async () => {
		const trace: string[] = [];
		let interaction:
			| ((request: SshEnvironmentSupervisorInteractionRequest) => Promise<unknown>)
			| undefined;
		const registry = {
			create: vi.fn<DesktopEnvironmentRegistry['create']>(async (record) => {
				trace.push('registry-create');
				return { environment: record, kind: 'created', revision: 1 };
			}),
			list: vi.fn<DesktopEnvironmentRegistry['list']>(async () => {
				trace.push('registry-list');
				return { kind: 'ready', value: [] };
			}),
		};
		const connections = {
			claimInteractionLease: (_candidate: EnvironmentId, present: typeof interaction) => {
				trace.push('lease');
				interaction = present;
				return { kind: 'claimed' as const, lease: { release: () => trace.push('release') } };
			},
			connect: async () => {
				trace.push('connect');
				if (interaction === undefined) throw new Error('Missing interaction lease.');
				await interaction({
					generation: 1,
					handle: 'ssh-prompt-test',
					kind: 'none',
					prompt: 'Password:',
				});
				return {
					activation: 'started' as const,
					kind: 'ready' as const,
					observation: connectedObservation(environmentId),
				};
			},
			disconnect: vi.fn<EnrollmentConnections['disconnect']>(async () => ({
				kind: 'disconnected' as const,
			})),
			inspectProjectRoot: vi.fn<EnrollmentConnections['inspectProjectRoot']>(async () => {
				trace.push('inspect');
				return {
					canonicalPath: SshProjectRootPathSchema.parse('/srv/orchbee'),
					kind: 'resolved' as const,
				};
			}),
			observation: () => ({
				kind: 'ready' as const,
				observation: connectedObservation(environmentId),
			}),
			subscribe: () => () => undefined,
		};
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			admitProject: async () => {
				trace.push('commit');
				return {
					canonicalPath: '/srv/orchbee',
					directoryId: 'directory-1',
					kind: 'created',
					projectId: 'project-1',
					workspaceId: 'workspace-1',
				};
			},
			allocateIdentifier: identifierSequence(),
			connections,
			getDocument: () => null,
			registry,
			resolveTarget: async () => {
				trace.push('resolve');
				return resolvedTarget();
			},
		});
		const progress: string[] = [];
		const interact = vi.fn<DesktopSshProjectEnrollmentOperation['interact']>(async () => ({
			kind: 'submitted' as const,
			response: 'secret',
		}));

		await expect(
			runtime.enroll(
				{
					configurationFilePath: null,
					destination: 'build-host',
					name: 'Build machine',
					projectPath: '/srv/orchbee-alias',
				},
				{ interact, progress: (stage) => progress.push(stage) },
				new AbortController().signal,
			),
		).resolves.toEqual({
			canonicalPath: '/srv/orchbee',
			kind: 'created',
			projectId: 'project-1',
			workspaceId: 'workspace-1',
		});
		expect(trace).toEqual([
			'resolve',
			'registry-list',
			'registry-create',
			'lease',
			'connect',
			'inspect',
			'commit',
			'release',
		]);
		expect(progress).toEqual([
			'resolving-target',
			'recording-environment',
			'connecting',
			'inspecting-project',
			'committing-project',
		]);
		expect(interact).toHaveBeenCalledWith({
			generation: 1,
			handle: 'ssh-prompt-test',
			kind: 'none',
			prompt: 'Password:',
		});
	});

	it('reuses the immutable identity and authoritative name for an exact target', async () => {
		const registry = {
			create: vi.fn<DesktopEnvironmentRegistry['create']>(),
			list: vi.fn<DesktopEnvironmentRegistry['list']>().mockResolvedValue({
				kind: 'ready',
				value: [environment],
			}),
		};
		const admittedEnvironmentIds: EnvironmentId[] = [];
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			admitProject: async (candidate) => {
				admittedEnvironmentIds.push(candidate);
				return {
					canonicalPath: '/srv/project',
					directoryId: 'directory-1',
					kind: 'existing',
					projectId: 'project-1',
					workspaceId: 'workspace-1',
				};
			},
			allocateIdentifier: identifierSequence(),
			connections: immediatelyConnectedConnections(environmentId),
			getDocument: () => null,
			registry,
			resolveTarget: async () => resolvedTarget(),
		});

		await runtime.enroll(
			{
				configurationFilePath: null,
				destination: 'build-host',
				name: 'A renderer cannot rename this',
				projectPath: '/srv/project',
			},
			{ interact: async () => ({ kind: 'cancelled' }), progress: () => undefined },
			new AbortController().signal,
		);

		expect(registry.create).not.toHaveBeenCalled();
		expect(admittedEnvironmentIds).toEqual([environmentId]);
	});

	it('cancels a pre-commit operation and stops only its unreferenced new attachment', async () => {
		const observations = new EventEmitter();
		const abortController = new AbortController();
		const disconnect = vi.fn<EnrollmentConnections['disconnect']>(async () => ({
			kind: 'disconnected' as const,
		}));
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			admitProject: vi.fn<DesktopSshProjectEnrollmentRuntimeOptions['admitProject']>(),
			allocateIdentifier: identifierSequence(),
			connections: {
				claimInteractionLease: () => ({
					kind: 'claimed' as const,
					lease: { release: () => undefined },
				}),
				connect: async () => ({
					activation: 'started' as const,
					kind: 'ready' as const,
					observation: resolvingObservation(environmentId),
				}),
				disconnect,
				inspectProjectRoot: vi.fn<EnrollmentConnections['inspectProjectRoot']>(),
				observation: () => ({
					kind: 'ready' as const,
					observation: resolvingObservation(environmentId),
				}),
				subscribe: (_candidate, listener) => {
					observations.on('observation', listener);
					return () => observations.off('observation', listener);
				},
			},
			getDocument: () => null,
			registry: {
				create: vi.fn<DesktopEnvironmentRegistry['create']>().mockResolvedValue({
					environment,
					kind: 'created',
					revision: 1,
				}),
				list: vi.fn<DesktopEnvironmentRegistry['list']>().mockResolvedValue({
					kind: 'ready',
					value: [],
				}),
			},
			resolveTarget: async () => resolvedTarget(),
		});
		const enrollment = runtime.enroll(
			{
				configurationFilePath: null,
				destination: 'build-host',
				name: 'Build machine',
				projectPath: '/srv/project',
			},
			{ interact: async () => ({ kind: 'cancelled' }), progress: () => undefined },
			abortController.signal,
		);
		await eventually(() => observations.listenerCount('observation') > 0);
		abortController.abort();

		await expect(enrollment).resolves.toEqual({ kind: 'cancelled' });
		expect(disconnect).toHaveBeenCalledWith(environmentId);
	});

	it('fails closed on indeterminate Environment durability before any connection starts', async () => {
		const connections = immediatelyConnectedConnections(environmentId);
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			admitProject: vi.fn<DesktopSshProjectEnrollmentRuntimeOptions['admitProject']>(),
			allocateIdentifier: identifierSequence(),
			connections,
			getDocument: () => null,
			registry: {
				create: vi.fn<DesktopEnvironmentRegistry['create']>().mockResolvedValue({
					kind: 'indeterminate',
					reason: 'durability-unconfirmed',
				}),
				list: vi.fn<DesktopEnvironmentRegistry['list']>().mockResolvedValue({
					kind: 'ready',
					value: [],
				}),
			},
			resolveTarget: async () => resolvedTarget(),
		});

		await expect(
			runtime.enroll(
				{
					configurationFilePath: null,
					destination: 'build-host',
					name: 'Build machine',
					projectPath: '/srv/project',
				},
				{ interact: async () => ({ kind: 'cancelled' }), progress: () => undefined },
				new AbortController().signal,
			),
		).resolves.toEqual({ kind: 'indeterminate', reason: 'durability-unconfirmed' });
		expect(connections.connect).not.toHaveBeenCalled();
		expect(connections.inspectProjectRoot).not.toHaveBeenCalled();
	});

	it('does not roll back a started attachment once the Workbench commit may have happened', async () => {
		const abortController = new AbortController();
		const disconnect = vi.fn<EnrollmentConnections['disconnect']>(async () => ({
			kind: 'disconnected' as const,
		}));
		const connections = {
			...immediatelyConnectedConnections(environmentId),
			disconnect,
		};
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			admitProject: async () => {
				abortController.abort();
				return { kind: 'cancelled' };
			},
			allocateIdentifier: identifierSequence(),
			connections,
			getDocument: () => null,
			registry: existingRegistry(),
			resolveTarget: async () => resolvedTarget(),
		});

		await expect(
			runtime.enroll(
				{
					configurationFilePath: null,
					destination: 'build-host',
					name: 'Build machine',
					projectPath: '/srv/project',
				},
				{ interact: async () => ({ kind: 'cancelled' }), progress: () => undefined },
				abortController.signal,
			),
		).resolves.toEqual({ kind: 'cancelled' });
		expect(disconnect).not.toHaveBeenCalled();
	});

	it('honors cancellation that races with a nonterminal initial connection observation', async () => {
		const abortController = new AbortController();
		const disconnect = vi.fn<EnrollmentConnections['disconnect']>(async () => ({
			kind: 'disconnected' as const,
		}));
		const connections = {
			...immediatelyConnectedConnections(environmentId),
			connect: vi.fn<EnrollmentConnections['connect']>(async () => {
				abortController.abort();
				return {
					activation: 'started' as const,
					kind: 'ready' as const,
					observation: resolvingObservation(environmentId),
				};
			}),
			disconnect,
			observation: () => ({
				kind: 'ready' as const,
				observation: resolvingObservation(environmentId),
			}),
			subscribe: vi.fn<EnrollmentConnections['subscribe']>(() => () => undefined),
		};
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			admitProject: vi.fn<DesktopSshProjectEnrollmentRuntimeOptions['admitProject']>(),
			allocateIdentifier: identifierSequence(),
			connections,
			getDocument: () => null,
			registry: existingRegistry(),
			resolveTarget: async () => resolvedTarget(),
		});

		await expect(
			runtime.enroll(
				{
					configurationFilePath: null,
					destination: 'build-host',
					name: 'Build machine',
					projectPath: '/srv/project',
				},
				{ interact: async () => ({ kind: 'cancelled' }), progress: () => undefined },
				abortController.signal,
			),
		).resolves.toEqual({ kind: 'cancelled' });
		expect(disconnect).toHaveBeenCalledWith(environmentId);
	});

	it('reports explicit Project status and disconnects only process-local runtime state', async () => {
		const document = remoteProjectDocument();
		let connection: ReturnType<typeof connectedObservation> | undefined =
			connectedObservation(environmentId);
		const disconnect = vi.fn<EnrollmentConnections['disconnect']>(async () => {
			connection = undefined;
			return { kind: 'disconnected' as const };
		});
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			admitProject: vi.fn<DesktopSshProjectEnrollmentRuntimeOptions['admitProject']>(),
			allocateIdentifier: identifierSequence(),
			connections: {
				...immediatelyConnectedConnections(environmentId),
				disconnect,
				observation: () =>
					connection === undefined
						? { kind: 'inactive' as const }
						: { kind: 'ready' as const, observation: connection },
			},
			getDocument: () => document,
			registry: existingRegistry(),
			resolveTarget: async () => resolvedTarget(),
		});

		expect(runtime.statuses()).toEqual([
			{ blockedReason: null, projectId: 'project-remote', state: 'connected' },
		]);
		await expect(
			runtime.disconnectProject('project-remote', new AbortController().signal),
		).resolves.toEqual({ kind: 'disconnected', projectId: 'project-remote' });
		expect(runtime.statuses()).toEqual([
			{ blockedReason: null, projectId: 'project-remote', state: 'disconnected' },
		]);
		expect(document.projectWorkspace.projects).toHaveLength(1);
		expect(document.projectWorkspace.projects[0]?.directories[0]?.placement).toEqual({
			environmentId,
			kind: 'environment',
		});
	});

	it('fails closed when the connection owner cannot confirm explicit disconnection', async () => {
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			admitProject: vi.fn<DesktopSshProjectEnrollmentRuntimeOptions['admitProject']>(),
			allocateIdentifier: identifierSequence(),
			connections: {
				...immediatelyConnectedConnections(environmentId),
				disconnect: async () => ({
					kind: 'unavailable' as const,
					reason: 'connection-owner-disposed' as const,
				}),
			},
			getDocument: remoteProjectDocument,
			registry: existingRegistry(),
			resolveTarget: async () => resolvedTarget(),
		});

		await expect(
			runtime.disconnectProject('project-remote', new AbortController().signal),
		).resolves.toEqual({ kind: 'failed', reason: 'connection-owner-unavailable' });
	});
});

function immediatelyConnectedConnections(candidate: EnvironmentId) {
	return {
		claimInteractionLease: () => ({
			kind: 'claimed' as const,
			lease: { release: () => undefined },
		}),
		connect: vi.fn<EnrollmentConnections['connect']>(async () => ({
			activation: 'started' as const,
			kind: 'ready' as const,
			observation: connectedObservation(candidate),
		})),
		disconnect: async () => ({ kind: 'disconnected' as const }),
		inspectProjectRoot: vi.fn<EnrollmentConnections['inspectProjectRoot']>(async () => ({
			canonicalPath: SshProjectRootPathSchema.parse('/srv/project'),
			kind: 'resolved' as const,
		})),
		observation: () => ({ kind: 'ready' as const, observation: connectedObservation(candidate) }),
		subscribe: () => () => undefined,
	};
}

function existingRegistry() {
	return {
		create: vi.fn<DesktopEnvironmentRegistry['create']>(),
		list: vi.fn<DesktopEnvironmentRegistry['list']>().mockResolvedValue({
			kind: 'ready',
			value: [environment],
		}),
	};
}

function remoteProjectDocument() {
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextProjectSequence: 2,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [
				{
					directories: [
						{
							available: false,
							git: null,
							id: 'directory-remote',
							kind: 'folder',
							name: 'project',
							path: '/srv/project',
							placement: { environmentId, kind: 'environment' },
							source: 'selected',
						},
					],
					id: 'project-remote',
					launchConfigurations: [],
					name: 'project',
					primaryDirectoryId: 'directory-remote',
					workspaces: [],
				},
			],
			standaloneTerminalContextId: 'terminal-context-1',
		},
		revision: workbenchSharedDocumentRevision('0'),
		terminalContexts: [
			{
				nextTabSequence: 1,
				panelWorkspace: {
					nextPanelSequence: 1,
					nextPanelSlotSequence: 1,
					nextSplitSequence: 1,
					nextTerminalSequence: 1,
					pendingRemovals: [],
					tabs: [],
				},
				tabCollection: { groups: [], tabs: [] },
				terminalContextId: 'terminal-context-1',
			},
		],
	});
}

function resolvingObservation(candidate: EnvironmentId): SshEnvironmentConnectionObservation {
	const state = createSshEnvironmentConnectionState({
		connectionId: candidate,
		target: { configurationFilePath: null, destination: 'build-host' },
	});
	return observeSshEnvironmentConnection(
		transitionSshEnvironmentConnection(state, { type: 'start-requested' }).state,
	);
}

function connectedObservation(candidate: EnvironmentId): SshEnvironmentConnectionObservation {
	const state = createSshEnvironmentConnectionState({
		connectionId: candidate,
		target: { configurationFilePath: null, destination: 'build-host' },
	});
	const resolving = transitionSshEnvironmentConnection(state, { type: 'start-requested' });
	const connecting = transitionSshEnvironmentConnection(resolving.state, {
		generation: 1,
		resolvedTarget: resolvedTarget().target,
		type: 'target-resolved',
	});
	const connected = transitionSshEnvironmentConnection(connecting.state, {
		generation: 1,
		runtimeEpoch: '3'.repeat(32),
		runtimeInstanceId: '2'.repeat(32),
		type: 'remote-runtime-ready',
	});
	return observeSshEnvironmentConnection(connected.state);
}

function resolvedTarget(): Extract<OpenSshTargetResolutionResult, { readonly kind: 'resolved' }> {
	return {
		kind: 'resolved',
		target: {
			destination: environment.provider.target.destination,
			hostKeyLookupName: 'build-host',
			hostKeyPolicy: 'ask',
			hostname: 'build-host',
			port: 22,
			user: 'builder',
		},
	};
}

function identifierSequence(): () => string {
	let sequence = 0;
	return () => (++sequence).toString(16).padStart(32, '0');
}

async function eventually(predicate: () => boolean): Promise<void> {
	for (let attempt = 0; attempt < 40; attempt += 1) {
		if (predicate()) return;
		await new Promise<void>((resolve) => setTimeout(resolve, 0));
	}
	throw new Error('Expected condition was not reached.');
}
