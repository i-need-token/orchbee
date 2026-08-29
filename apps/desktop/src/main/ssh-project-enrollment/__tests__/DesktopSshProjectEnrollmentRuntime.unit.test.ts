import { EventEmitter } from 'node:events';

import {
	SshEnvironmentRuntimeInstanceIdSchema,
	SshProjectRootDirectoryNameSchema,
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

const rejectFolderAttachment: DesktopSshProjectEnrollmentRuntimeOptions['attachProjectFolder'] =
	async () => ({ kind: 'failed', reason: 'workbench-unavailable' });

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
			listProjectDirectory: vi.fn<EnrollmentConnections['listProjectDirectory']>(async () => {
				trace.push('list');
				return directoryListing('/srv/orchbee');
			}),
			observation: () => ({
				kind: 'ready' as const,
				observation: connectedObservation(environmentId),
			}),
			subscribe: () => () => undefined,
		};
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			attachProjectFolder: rejectFolderAttachment,
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
		const selectDirectory = vi.fn<DesktopSshProjectEnrollmentOperation['selectDirectory']>(
			async () => {
				trace.push('select');
				return { kind: 'selected' };
			},
		);

		await expect(
			runtime.enroll(
				{
					destination: 'ssh build-host',
				},
				{ interact, progress: (stage) => progress.push(stage), selectDirectory },
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
			'list',
			'select',
			'inspect',
			'commit',
			'release',
		]);
		expect(progress).toEqual([
			'resolving-target',
			'recording-environment',
			'connecting',
			'selecting-project-directory',
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
			attachProjectFolder: rejectFolderAttachment,
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
				destination: 'build-host',
			},
			{
				interact: async () => ({ kind: 'cancelled' }),
				progress: () => undefined,
				selectDirectory: selectCurrentDirectory,
			},
			new AbortController().signal,
		);

		expect(registry.create).not.toHaveBeenCalled();
		expect(admittedEnvironmentIds).toEqual([environmentId]);
	});

	it('publishes Project connection status only after the canonical commit is visible', async () => {
		let document: ReturnType<typeof remoteProjectDocument> | null = null;
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			attachProjectFolder: rejectFolderAttachment,
			admitProject: async () => {
				await Promise.resolve();
				document = remoteProjectDocument();
				return {
					canonicalPath: '/srv/project',
					directoryId: 'directory-remote',
					kind: 'created',
					projectId: 'project-remote',
					workspaceId: 'workspace-remote',
				};
			},
			allocateIdentifier: identifierSequence(),
			connections: immediatelyConnectedConnections(environmentId),
			getDocument: () => document,
			registry: existingRegistry(),
			resolveTarget: async () => resolvedTarget(),
		});
		const publications: (readonly { readonly projectId: string; readonly state: string }[])[] = [];
		runtime.subscribeStatuses((statuses) => publications.push(statuses));

		await runtime.enroll(
			{ destination: 'build-host' },
			{
				interact: async () => ({ kind: 'cancelled' }),
				progress: () => undefined,
				selectDirectory: selectCurrentDirectory,
			},
			new AbortController().signal,
		);

		expect(publications.at(-1)).toEqual([
			{ blockedReason: null, projectId: 'project-remote', state: 'connected' },
		]);
	});

	it('reuses the connection and directory workflow to attach a Folder to the exact Project', async () => {
		const connections = immediatelyConnectedConnections(environmentId);
		const attachProjectFolder = vi.fn<
			DesktopSshProjectEnrollmentRuntimeOptions['attachProjectFolder']
		>(async (_candidateEnvironmentId, projectId, canonicalPath) => ({
			canonicalPath,
			directoryId: 'environment-directory:44444444444444444444444444444444',
			kind: 'attached',
			projectId,
		}));
		const admitProject = vi.fn<DesktopSshProjectEnrollmentRuntimeOptions['admitProject']>();
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			admitProject,
			allocateIdentifier: identifierSequence(),
			attachProjectFolder,
			connections,
			getDocument: () => null,
			registry: existingRegistry(),
			resolveTarget: async () => resolvedTarget(),
		});
		const progress: string[] = [];

		await expect(
			runtime.attachFolder(
				'project-local',
				{ destination: 'ssh build-host' },
				{
					interact: async () => ({ kind: 'cancelled' }),
					progress: (stage) => progress.push(stage),
					selectDirectory: selectCurrentDirectory,
				},
				new AbortController().signal,
			),
		).resolves.toEqual({
			canonicalPath: '/srv/project',
			directoryId: 'environment-directory:44444444444444444444444444444444',
			kind: 'attached',
			projectId: 'project-local',
		});
		expect(attachProjectFolder).toHaveBeenCalledWith(
			environmentId,
			'project-local',
			'/srv/project',
			expect.any(AbortSignal),
		);
		expect(admitProject).not.toHaveBeenCalled();
		expect(progress).toEqual([
			'resolving-target',
			'recording-environment',
			'connecting',
			'selecting-project-directory',
			'inspecting-project',
			'committing-project-folder',
		]);
	});

	it('retains the last valid snapshot after failed navigation and inspects only the selection', async () => {
		const connections = immediatelyConnectedConnections(environmentId);
		connections.listProjectDirectory
			.mockResolvedValueOnce({
				...directoryListing('/home/builder'),
				directories: [{ name: SshProjectRootDirectoryNameSchema.parse('src') }],
				parentPath: SshProjectRootPathSchema.parse('/home'),
			})
			.mockResolvedValueOnce({ kind: 'failed', reason: 'not-found' })
			.mockResolvedValueOnce(directoryListing('/home/builder/src'));
		const snapshots: Parameters<DesktopSshProjectEnrollmentOperation['selectDirectory']>[0][] = [];
		const selectDirectory = vi.fn<DesktopSshProjectEnrollmentOperation['selectDirectory']>(
			async (snapshot) => {
				snapshots.push(snapshot);
				if (snapshots.length === 1) return { kind: 'navigate', path: '/missing' };
				if (snapshots.length === 2) {
					return { kind: 'navigate', path: '/home/builder/src' };
				}
				return { kind: 'selected' };
			},
		);
		const admitProject = vi.fn<DesktopSshProjectEnrollmentRuntimeOptions['admitProject']>(
			async () => ({
				canonicalPath: '/srv/project',
				directoryId: 'directory-1',
				kind: 'created',
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			}),
		);
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			attachProjectFolder: rejectFolderAttachment,
			admitProject,
			allocateIdentifier: identifierSequence(),
			connections,
			getDocument: () => null,
			registry: existingRegistry(),
			resolveTarget: async () => resolvedTarget(),
		});

		await runtime.enroll(
			{ destination: 'build-host' },
			{
				interact: async () => ({ kind: 'cancelled' }),
				progress: () => undefined,
				selectDirectory,
			},
			new AbortController().signal,
		);

		expect(snapshots).toEqual([
			expect.objectContaining({ canonicalPath: '/home/builder', navigationFailure: null }),
			expect.objectContaining({
				canonicalPath: '/home/builder',
				navigationFailure: 'not-found',
			}),
			expect.objectContaining({ canonicalPath: '/home/builder/src', navigationFailure: null }),
		]);
		expect(connections.inspectProjectRoot).toHaveBeenCalledWith(
			environmentId,
			'/home/builder/src',
			expect.any(AbortSignal),
		);
		expect(admitProject).toHaveBeenCalledWith(
			environmentId,
			'/srv/project',
			expect.any(AbortSignal),
		);
	});

	it('cancels a pre-commit operation and stops only its unreferenced new attachment', async () => {
		const observations = new EventEmitter();
		const abortController = new AbortController();
		const disconnect = vi.fn<EnrollmentConnections['disconnect']>(async () => ({
			kind: 'disconnected' as const,
		}));
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			attachProjectFolder: rejectFolderAttachment,
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
				listProjectDirectory: vi.fn<EnrollmentConnections['listProjectDirectory']>(),
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
				destination: 'build-host',
			},
			{
				interact: async () => ({ kind: 'cancelled' }),
				progress: () => undefined,
				selectDirectory: selectCurrentDirectory,
			},
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
			attachProjectFolder: rejectFolderAttachment,
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
					destination: 'build-host',
				},
				{
					interact: async () => ({ kind: 'cancelled' }),
					progress: () => undefined,
					selectDirectory: selectCurrentDirectory,
				},
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
			attachProjectFolder: rejectFolderAttachment,
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
					destination: 'build-host',
				},
				{
					interact: async () => ({ kind: 'cancelled' }),
					progress: () => undefined,
					selectDirectory: selectCurrentDirectory,
				},
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
			attachProjectFolder: rejectFolderAttachment,
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
					destination: 'build-host',
				},
				{
					interact: async () => ({ kind: 'cancelled' }),
					progress: () => undefined,
					selectDirectory: selectCurrentDirectory,
				},
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
			attachProjectFolder: rejectFolderAttachment,
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
			attachProjectFolder: rejectFolderAttachment,
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

	it('builds a reusable catalog from saved Environments and bounded SSH Host discovery', async () => {
		const discoverHostAliases = vi
			.fn<NonNullable<DesktopSshProjectEnrollmentRuntimeOptions['discoverHostAliases']>>()
			.mockResolvedValue({ aliases: ['build-host', 'gpu-lab'], issue: 'truncated' });
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			attachProjectFolder: rejectFolderAttachment,
			admitProject: vi.fn<DesktopSshProjectEnrollmentRuntimeOptions['admitProject']>(),
			allocateIdentifier: identifierSequence(),
			connections: immediatelyConnectedConnections(environmentId),
			discoverHostAliases,
			getDocument: remoteProjectDocument,
			registry: existingRegistry(),
			resolveTarget: async () => resolvedTarget(),
		});

		await expect(runtime.environmentCatalog(false)).resolves.toEqual({
			discoveryIssue: 'truncated',
			managed: [
				{
					blockedReason: null,
					destination: 'build-host',
					environmentId,
					name: 'Authoritative build machine',
					projectCount: 1,
					state: 'connected',
				},
			],
			registryIssue: null,
			suggestions: [{ destination: 'gpu-lab' }],
		});
		await runtime.environmentCatalog(false);
		expect(discoverHostAliases).toHaveBeenCalledOnce();
		await runtime.environmentCatalog(true);
		expect(discoverHostAliases).toHaveBeenCalledTimes(2);
	});

	it('validates and records a suggested host without opening a connection', async () => {
		const records: (typeof environment)[] = [];
		const registry = {
			create: vi.fn<DesktopEnvironmentRegistry['create']>(async (record) => {
				records.push(record);
				return { environment: record, kind: 'created', revision: 1 };
			}),
			list: vi.fn<DesktopEnvironmentRegistry['list']>(async () => ({
				kind: 'ready' as const,
				value: [...records],
			})),
		};
		const connections = immediatelyConnectedConnections(environmentId);
		const resolveTarget = vi.fn<DesktopSshProjectEnrollmentRuntimeOptions['resolveTarget']>(
			async () => resolvedTarget(),
		);
		const runtime = new DesktopSshProjectEnrollmentRuntime({
			attachProjectFolder: rejectFolderAttachment,
			admitProject: vi.fn<DesktopSshProjectEnrollmentRuntimeOptions['admitProject']>(),
			allocateIdentifier: identifierSequence(),
			connections,
			discoverHostAliases: async () => ({ aliases: ['gpu-lab'], issue: null }),
			getDocument: () => null,
			registry,
			resolveTarget,
		});
		const catalogListener = vi.fn<() => void>();
		runtime.subscribeEnvironmentCatalog(catalogListener);

		await expect(
			runtime.addEnvironment({ destination: 'gpu-lab' }, new AbortController().signal),
		).resolves.toMatchObject({ kind: 'added' });
		expect(resolveTarget).toHaveBeenCalledWith({
			configurationFilePath: null,
			destination: 'gpu-lab',
		});
		expect(registry.create).toHaveBeenCalledOnce();
		expect(connections.connect).not.toHaveBeenCalled();
		expect(catalogListener).toHaveBeenCalledOnce();
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
		listProjectDirectory: vi.fn<EnrollmentConnections['listProjectDirectory']>(async () =>
			directoryListing('/srv/project'),
		),
		observation: () => ({ kind: 'ready' as const, observation: connectedObservation(candidate) }),
		subscribe: () => () => undefined,
	};
}

function directoryListing(path: string) {
	return {
		canonicalPath: SshProjectRootPathSchema.parse(path),
		directories: [],
		kind: 'listed' as const,
		parentPath: path === '/' ? null : SshProjectRootPathSchema.parse('/'),
		truncated: false,
	};
}

async function selectCurrentDirectory() {
	return { kind: 'selected' as const };
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
