import {
	SshEnvironmentRuntimeInstanceIdSchema,
	SshProjectRootPathSchema,
} from '@orchbee/ssh-environment-protocol';
import {
	WorkbenchSharedDocumentSchema,
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import { DesktopClientSessionCoordinator } from '../../client-session';
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
	type SshProjectRootInspectionOutcome,
} from '../../ssh-environment';
import { admitDesktopSshEnvironmentRemoteProject } from '../admitDesktopSshEnvironmentRemoteProject';
import type { WorkbenchDocumentStore } from '../../workbench-document';

const environmentId = EnvironmentIdSchema.parse('environment:00000000000000000000000000000001');
const environment = EnvironmentRecordSchema.parse({
	environmentId,
	name: 'builder',
	provider: {
		kind: 'ssh',
		runtimeInstanceId: SshEnvironmentRuntimeInstanceIdSchema.parse('1'.repeat(32)),
		target: { configurationFilePath: null, destination: 'builder.example' },
	},
});
const canonicalProjectRoot = SshProjectRootPathSchema.parse('/srv/orchbee');

function initialDocument(): WorkbenchSharedDocument {
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextProjectSequence: 1,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [],
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

function createStore(initial: WorkbenchSharedDocument = initialDocument()) {
	let current = initial;
	let nextCommitIsIndeterminate = false;
	const store: WorkbenchDocumentStore = {
		commit: vi.fn<WorkbenchDocumentStore['commit']>(async (expectedRevision, document) => {
			if (expectedRevision !== current.revision) {
				return { actualRevision: current.revision, kind: 'conflict' as const };
			}
			current = document;
			if (nextCommitIsIndeterminate) {
				nextCommitIsIndeterminate = false;
				return { kind: 'unavailable' as const, reason: 'write-indeterminate' as const };
			}
			return { kind: 'committed' as const, revision: document.revision };
		}),
		load: vi.fn<WorkbenchDocumentStore['load']>(async () => ({
			document: current,
			kind: 'available' as const,
		})),
	};
	return {
		current: () => current,
		failNextCommitAfterRename: () => {
			nextCommitIsIndeterminate = true;
		},
		store,
	};
}

async function createCoordinator(store: WorkbenchDocumentStore) {
	const coordinator = new DesktopClientSessionCoordinator({
		createClientSessionId: () => 'desktop:client:remote-project-test',
		defaultCwd: '/system',
		getTerminalEvents: () => [],
		initialPresentationId: terminalPresentationId('presentation-initial'),
		initializationDocument: initialDocument(),
		reconcileDocument: async () => undefined,
		store,
		terminalHostName: 'desktop',
		validateDocument: (document) => WorkbenchSharedDocumentSchema.safeParse(document).success,
	});
	expect(await coordinator.start()).toEqual({ kind: 'ready' });
	return coordinator;
}

function registry(): Pick<DesktopEnvironmentRegistry, 'get'> {
	return {
		get: vi.fn<DesktopEnvironmentRegistry['get']>(async () => ({
			kind: 'ready' as const,
			value: environment,
		})),
	};
}

function connectedObservation() {
	const stopped = createSshEnvironmentConnectionState({
		connectionId: environmentId,
		target: { configurationFilePath: null, destination: 'builder.example' },
	});
	const started = transitionSshEnvironmentConnection(stopped, { type: 'start-requested' });
	const connecting = transitionSshEnvironmentConnection(started.state, {
		generation: 1,
		resolvedTarget: {
			destination: stopped.target.destination,
			hostKeyLookupName: 'builder.example',
			hostKeyPolicy: 'ask',
			hostname: 'builder.example',
			port: 22,
			user: 'builder',
		},
		type: 'target-resolved',
	});
	const connected = transitionSshEnvironmentConnection(connecting.state, {
		generation: 1,
		runtimeEpoch: '2'.repeat(32),
		runtimeInstanceId: environment.provider.runtimeInstanceId,
		type: 'remote-runtime-ready',
	});
	return observeSshEnvironmentConnection(connected.state);
}

describe('admitDesktopSshEnvironmentRemoteProject', () => {
	it('follows registry, connectivity, inspection, then canonical mutation order', async () => {
		const persistence = createStore();
		const coordinator = await createCoordinator(persistence.store);
		const trace: string[] = [];
		const registered = registry();
		vi.mocked(registered.get).mockImplementation(async () => {
			trace.push('registry');
			return { kind: 'ready', value: environment };
		});

		await expect(
			admitDesktopSshEnvironmentRemoteProject({
				allocateDirectoryToken: () => {
					trace.push('mutation');
					return '33333333333333333333333333333333';
				},
				connections: {
					inspectProjectRoot: async () => {
						trace.push('inspection');
						return { canonicalPath: canonicalProjectRoot, kind: 'resolved' };
					},
					observation: () => {
						trace.push('connectivity');
						return { kind: 'ready', observation: connectedObservation() };
					},
				},
				coordinator,
				environmentId,
				registry: registered,
				requestedPath: '/srv/requested-alias',
			}),
		).resolves.toMatchObject({
			canonicalPath: canonicalProjectRoot,
			kind: 'created',
		});
		expect(trace).toEqual(['registry', 'connectivity', 'inspection', 'mutation']);
		expect(persistence.current().projectWorkspace.projects[0]).toMatchObject({
			directories: [{ name: 'orchbee', path: canonicalProjectRoot }],
			name: 'orchbee',
			workspaces: [
				{
					id: 'workspace-1',
					primaryDirectoryId: 'environment-directory:33333333333333333333333333333333',
					terminalContextId: 'terminal-context-2',
				},
			],
		});
		expect(persistence.current().terminalContexts[1]).toMatchObject({
			panelWorkspace: { tabs: [] },
			tabCollection: { tabs: [] },
			terminalContextId: 'terminal-context-2',
		});
		expect(JSON.stringify(persistence.current())).not.toContain('/srv/requested-alias');
	});

	it('serializes concurrent equivalent admissions into one Project', async () => {
		const persistence = createStore();
		const coordinator = await createCoordinator(persistence.store);
		const inspectProjectRoot = vi.fn<
			(
				environmentId: EnvironmentId,
				path: string,
				signal?: AbortSignal,
			) => Promise<SshProjectRootInspectionOutcome>
		>(async () => ({ canonicalPath: canonicalProjectRoot, kind: 'resolved' }));
		const allocateDirectoryToken = vi
			.fn<() => string>()
			.mockReturnValue('11111111111111111111111111111111');
		const options = {
			allocateDirectoryToken,
			connections: {
				inspectProjectRoot,
				observation: () => ({ kind: 'ready' as const, observation: connectedObservation() }),
			},
			coordinator,
			environmentId,
			registry: registry(),
			requestedPath: '/srv/requested',
		};

		const results = await Promise.all([
			admitDesktopSshEnvironmentRemoteProject(options),
			admitDesktopSshEnvironmentRemoteProject(options),
		]);

		expect(results.map((result) => result.kind).sort()).toEqual(['created', 'existing']);
		expect(results[0]).toMatchObject({
			canonicalPath: '/srv/orchbee',
			directoryId: 'environment-directory:11111111111111111111111111111111',
			projectId: 'project-1',
			workspaceId: 'workspace-1',
		});
		expect(results[1]).toMatchObject({
			canonicalPath: '/srv/orchbee',
			directoryId: 'environment-directory:11111111111111111111111111111111',
			projectId: 'project-1',
			workspaceId: 'workspace-1',
		});
		expect(allocateDirectoryToken).toHaveBeenCalledOnce();
		expect(inspectProjectRoot).toHaveBeenCalledTimes(2);
		expect(persistence.current().projectWorkspace.projects[0]).toMatchObject({
			directories: [
				{
					available: false,
					path: '/srv/orchbee',
					placement: { environmentId, kind: 'environment' },
				},
			],
			name: 'orchbee',
			workspaces: [
				{
					id: 'workspace-1',
					primaryDirectoryId: 'environment-directory:11111111111111111111111111111111',
					terminalContextId: 'terminal-context-2',
				},
			],
		});
		expect(persistence.current().terminalContexts).toHaveLength(2);
	});

	it('atomically upgrades a legacy remote Project with its missing Workspace and empty context', async () => {
		const empty = initialDocument();
		const legacy = createWorkbenchSharedDocument({
			projectWorkspace: {
				...empty.projectWorkspace,
				nextProjectSequence: 2,
				projects: [
					{
						directories: [
							{
								available: false,
								git: null,
								id: 'environment-directory:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
								kind: 'folder',
								name: 'orchbee',
								path: canonicalProjectRoot,
								placement: { environmentId, kind: 'environment' },
								source: 'selected',
							},
						],
						id: 'project-1',
						launchConfigurations: [],
						name: 'orchbee',
						primaryDirectoryId: 'environment-directory:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
						workspaces: [],
					},
				],
			},
			revision: workbenchSharedDocumentRevision('4'),
			terminalContexts: empty.terminalContexts,
		});
		const persistence = createStore(legacy);
		const coordinator = await createCoordinator(persistence.store);
		const allocateDirectoryToken = vi.fn<() => string>();

		await expect(
			admitDesktopSshEnvironmentRemoteProject({
				allocateDirectoryToken,
				connections: {
					inspectProjectRoot: async () => ({
						canonicalPath: canonicalProjectRoot,
						kind: 'resolved',
					}),
					observation: () => ({ kind: 'ready', observation: connectedObservation() }),
				},
				coordinator,
				environmentId,
				registry: registry(),
				requestedPath: canonicalProjectRoot,
			}),
		).resolves.toMatchObject({
			directoryId: 'environment-directory:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
			kind: 'created',
			projectId: 'project-1',
			workspaceId: 'workspace-1',
		});
		expect(allocateDirectoryToken).not.toHaveBeenCalled();
		expect(persistence.current()).toMatchObject({
			projectWorkspace: {
				nextProjectSequence: 2,
				nextTerminalContextSequence: 3,
				nextWorkspaceSequence: 2,
				projects: [{ workspaces: [{ terminalContextId: 'terminal-context-2' }] }],
			},
			revision: '5',
			terminalContexts: [
				{ terminalContextId: 'terminal-context-1' },
				{ terminalContextId: 'terminal-context-2' },
			],
		});
	});

	it('does not admit before a connected inspection completes', async () => {
		const persistence = createStore();
		const coordinator = await createCoordinator(persistence.store);
		const inspectProjectRoot = vi.fn<
			(
				environmentId: EnvironmentId,
				path: string,
				signal?: AbortSignal,
			) => Promise<SshProjectRootInspectionOutcome>
		>(async () => ({ kind: 'cancelled' }));

		expect(
			await admitDesktopSshEnvironmentRemoteProject({
				allocateDirectoryToken: () => '11111111111111111111111111111111',
				connections: {
					inspectProjectRoot,
					observation: () => ({ kind: 'ready', observation: connectedObservation() }),
				},
				coordinator,
				environmentId,
				registry: registry(),
				requestedPath: '/srv/orchbee',
			}),
		).toEqual({ kind: 'cancelled' });
		expect(persistence.current().projectWorkspace.projects).toEqual([]);

		const controller = new AbortController();
		expect(
			await admitDesktopSshEnvironmentRemoteProject({
				allocateDirectoryToken: () => '11111111111111111111111111111111',
				connections: {
					inspectProjectRoot: async () => {
						controller.abort();
						return { canonicalPath: canonicalProjectRoot, kind: 'resolved' };
					},
					observation: () => ({ kind: 'ready', observation: connectedObservation() }),
				},
				coordinator,
				environmentId,
				registry: registry(),
				requestedPath: '/srv/orchbee',
				signal: controller.signal,
			}),
		).toEqual({ kind: 'cancelled' });
		expect(persistence.current().projectWorkspace.projects).toEqual([]);

		expect(
			await admitDesktopSshEnvironmentRemoteProject({
				allocateDirectoryToken: () => '11111111111111111111111111111111',
				connections: {
					inspectProjectRoot,
					observation: () => ({ kind: 'inactive' }),
				},
				coordinator,
				environmentId,
				registry: registry(),
				requestedPath: '/srv/orchbee',
			}),
		).toEqual({ kind: 'failed', reason: 'environment-not-connected' });
	});

	it('reports post-rename uncertainty and reconciles the idempotency key after restart', async () => {
		const persistence = createStore();
		persistence.failNextCommitAfterRename();
		const firstCoordinator = await createCoordinator(persistence.store);
		let connected = true;
		const connections = {
			inspectProjectRoot: vi.fn<
				(
					environmentId: EnvironmentId,
					path: string,
					signal?: AbortSignal,
				) => Promise<SshProjectRootInspectionOutcome>
			>(async () => {
				connected = false;
				return { canonicalPath: canonicalProjectRoot, kind: 'resolved' };
			}),
			observation: () =>
				connected
					? { kind: 'ready' as const, observation: connectedObservation() }
					: { kind: 'inactive' as const },
		};
		const baseOptions = {
			allocateDirectoryToken: () => '22222222222222222222222222222222',
			connections,
			environmentId,
			registry: registry(),
			requestedPath: '/srv/orchbee',
		};

		expect(
			await admitDesktopSshEnvironmentRemoteProject({
				...baseOptions,
				coordinator: firstCoordinator,
			}),
		).toEqual({ kind: 'indeterminate', reason: 'durability-unconfirmed' });
		expect(persistence.current().projectWorkspace.projects).toHaveLength(1);

		connected = true;
		const restartedCoordinator = await createCoordinator(persistence.store);
		expect(
			await admitDesktopSshEnvironmentRemoteProject({
				...baseOptions,
				coordinator: restartedCoordinator,
			}),
		).toMatchObject({ kind: 'existing', projectId: 'project-1' });
		expect(persistence.current().projectWorkspace.projects).toHaveLength(1);
	});
});
