import { SshEnvironmentRuntimeInstanceIdSchema } from '@orchbee/ssh-environment-protocol';
import { DesktopRemoteTerminalCreationRequestIdSchema } from '@orchbee/desktop-client-protocol';
import {
	WorkbenchSharedDocumentSchema,
	createEmptyWorkbenchSharedTerminalContext,
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
} from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopClientSessionCoordinator,
	DesktopTerminalIntentReconciler,
} from '../../client-session';
import {
	EnvironmentIdSchema,
	EnvironmentRecordSchema,
	type DesktopEnvironmentRegistry,
} from '../../environment';
import type { WorkbenchDocumentStore } from '../../workbench-document';
import type { SshEnvironmentConnectionObservation } from '../../ssh-environment';
import type { DesktopSshEnvironmentTerminalRuntimeOwner } from '../DesktopSshEnvironmentTerminalRuntimeOwner';
import { createDesktopSshEnvironmentRemoteTerminal } from '../createDesktopSshEnvironmentRemoteTerminal';
import { resolveDesktopSshEnvironmentRemoteTerminalCreationTarget } from '../resolveDesktopSshEnvironmentRemoteTerminalCreationTarget';

const environmentId = EnvironmentIdSchema.parse('environment:00000000000000000000000000000001');
const otherEnvironmentId = EnvironmentIdSchema.parse(
	'environment:00000000000000000000000000000002',
);
const environment = EnvironmentRecordSchema.parse({
	environmentId,
	name: 'builder',
	provider: {
		kind: 'ssh',
		runtimeInstanceId: SshEnvironmentRuntimeInstanceIdSchema.parse('1'.repeat(32)),
		target: { configurationFilePath: null, destination: 'builder.example' },
	},
});

describe('createDesktopSshEnvironmentRemoteTerminal', () => {
	it('derives Environment authority only from the canonical Workspace', () => {
		const document = remoteDocument();
		expect(
			resolveDesktopSshEnvironmentRemoteTerminalCreationTarget(
				document,
				'project-1',
				'workspace-1',
			),
		).toMatchObject({
			directoryPath: '/srv/orchbee',
			environmentId,
			kind: 'resolved',
		});
		expect(
			resolveDesktopSshEnvironmentRemoteTerminalCreationTarget(
				document,
				'project-missing',
				'workspace-1',
			),
		).toEqual({ kind: 'rejected', reason: 'project-not-found' });
		expect(
			resolveDesktopSshEnvironmentRemoteTerminalCreationTarget(
				document,
				'project-1',
				'workspace-missing',
			),
		).toEqual({ kind: 'rejected', reason: 'workspace-not-found' });

		const project = document.projectWorkspace.projects[0];
		const directory = project?.directories[0];
		if (project === undefined || directory === undefined)
			throw new Error('Expected fixture Project.');
		const localDocument: WorkbenchSharedDocument = {
			...document,
			projectWorkspace: {
				...document.projectWorkspace,
				projects: [
					{
						...project,
						directories: [{ ...directory, available: true, placement: { kind: 'local' } }],
					},
				],
			},
		};
		expect(
			resolveDesktopSshEnvironmentRemoteTerminalCreationTarget(
				localDocument,
				'project-1',
				'workspace-1',
			),
		).toEqual({ kind: 'rejected', reason: 'workspace-environment-mismatch' });
	});

	it('commits before effect and explicitly reconciles the same pending terminal on retry', async () => {
		const trace: string[] = [];
		const persistence = createStore(remoteDocument(), false, () => trace.push('commit'));
		const execute = vi
			.fn<ConstructorParameters<typeof DesktopTerminalIntentReconciler>[0]['execute']>()
			.mockImplementationOnce(async () => {
				trace.push('effect-deferred');
				return { kind: 'deferred' };
			})
			.mockImplementationOnce(async (command, target) => {
				trace.push('effect-settled');
				expect(target).toEqual({ environmentId, kind: 'ssh-environment' });
				return {
					event: {
						operationId: command.operationId,
						presentationId: command.presentationId,
						type: 'terminal-provision-completed',
					},
					kind: 'settled',
				};
			});
		const coordinator = await createCoordinator(persistence.store, execute);
		const prepare = vi.fn<DesktopSshEnvironmentTerminalRuntimeOwner['prepare']>(async () => {
			trace.push('prepare');
			return { activation: 'existing' as const, kind: 'ready' as const };
		});
		let identifierSequence = 0;
		const options = {
			allocateIdentifier: () => `${++identifierSequence}`.padStart(32, '0'),
			connections: connectedConnections(),
			coordinator,
			environmentId,
			projectId: 'project-1',
			requestId: DesktopRemoteTerminalCreationRequestIdSchema.parse('remote-terminal:retry'),
			registry: registry(),
			runtimeOwner: { prepare },
			terminalTitle: 'Remote Terminal',
			workspaceId: 'workspace-1',
		};

		const first = await createDesktopSshEnvironmentRemoteTerminal(options);
		expect(first).toMatchObject({
			kind: 'created',
			projectId: 'project-1',
			revision: '5',
			workspaceId: 'workspace-1',
		});
		if (first.kind !== 'created') throw new Error('Expected one created remote terminal.');
		expect(trace).toEqual(['prepare', 'commit', 'effect-deferred']);
		const firstCommand = execute.mock.calls[0]?.[0];
		expect(firstCommand).toMatchObject({
			launchCwd: '/srv/orchbee',
			type: 'terminal-provision-requested',
		});

		const second = await createDesktopSshEnvironmentRemoteTerminal(options);
		expect(second).toMatchObject({
			kind: 'existing',
			presentationId: first.presentationId,
			revision: '5',
		});
		expect(trace).toEqual(['prepare', 'commit', 'effect-deferred', 'prepare', 'effect-settled']);
		expect(execute.mock.calls[1]?.[0]).toEqual(firstCommand);
		expect(persistence.store.commit).toHaveBeenCalledOnce();
		expect(persistence.current().projectWorkspace.projects[0]?.directories[0]).toMatchObject({
			available: false,
			path: '/srv/orchbee',
			placement: { environmentId, kind: 'environment' },
		});
	});

	it('creates one top-level remote Terminal for each distinct request identity', async () => {
		const persistence = createStore(remoteDocument());
		const execute = vi.fn<
			ConstructorParameters<typeof DesktopTerminalIntentReconciler>[0]['execute']
		>(async (command) => ({
			event: {
				operationId: command.operationId,
				presentationId: command.presentationId,
				type: 'terminal-provision-completed',
			},
			kind: 'settled',
		}));
		const coordinator = await createCoordinator(persistence.store, execute);
		let identifierSequence = 0;
		const base = {
			allocateIdentifier: () => `${++identifierSequence}`.padStart(32, '0'),
			connections: connectedConnections(),
			coordinator,
			environmentId,
			projectId: 'project-1',
			registry: registry(),
			runtimeOwner: {
				prepare: async () => ({ activation: 'existing' as const, kind: 'ready' as const }),
			},
			terminalTitle: 'Remote Terminal',
			workspaceId: 'workspace-1',
		};

		const first = await createDesktopSshEnvironmentRemoteTerminal({
			...base,
			requestId: DesktopRemoteTerminalCreationRequestIdSchema.parse('remote-terminal:first'),
		});
		const retitled = await coordinator.mutateCanonicalDocument((canonical) => {
			const remoteContext = canonical.terminalContexts[1];
			const tab = remoteContext?.tabCollection.tabs[0];
			if (remoteContext === undefined || tab === undefined) {
				return { kind: 'rejected', reason: 'missing-remote-terminal' };
			}
			return {
				document: createWorkbenchSharedDocument({
					projectWorkspace: canonical.projectWorkspace,
					revision: workbenchSharedDocumentRevision((BigInt(canonical.revision) + 1n).toString()),
					terminalContexts: [
						canonical.terminalContexts[0] ?? remoteContext,
						{
							...remoteContext,
							tabCollection: {
								...remoteContext.tabCollection,
								tabs: [{ ...tab, title: 'remote-host' }],
							},
						},
					],
				}),
				kind: 'commit',
			};
		});
		expect(retitled.kind).toBe('committed');
		const second = await createDesktopSshEnvironmentRemoteTerminal({
			...base,
			requestId: DesktopRemoteTerminalCreationRequestIdSchema.parse('remote-terminal:second'),
		});

		expect(first).toMatchObject({ kind: 'created', tabId: 'terminal-context-2-tab-1' });
		expect(second).toMatchObject({ kind: 'created', tabId: 'terminal-context-2-tab-2' });
		if (first.kind !== 'created' || second.kind !== 'created') {
			throw new Error('Expected two created remote Terminals.');
		}
		expect(second.presentationId).not.toBe(first.presentationId);
		expect(persistence.current().terminalContexts[1]?.panelWorkspace.tabs).toHaveLength(2);
	});

	it('fails closed for disconnected or mismatched Workspace authority', async () => {
		const persistence = createStore(remoteDocument());
		const execute =
			vi.fn<ConstructorParameters<typeof DesktopTerminalIntentReconciler>[0]['execute']>();
		const coordinator = await createCoordinator(persistence.store, execute);
		const prepare = vi.fn<DesktopSshEnvironmentTerminalRuntimeOwner['prepare']>(async () => ({
			activation: 'started',
			kind: 'ready',
		}));
		const base = {
			allocateIdentifier: () => 'a'.repeat(32),
			coordinator,
			environmentId,
			projectId: 'project-1',
			requestId: DesktopRemoteTerminalCreationRequestIdSchema.parse('remote-terminal:authority'),
			registry: registry(),
			runtimeOwner: { prepare },
			terminalTitle: 'Remote Terminal',
			workspaceId: 'workspace-1',
		};

		await expect(
			createDesktopSshEnvironmentRemoteTerminal({
				...base,
				connections: { observation: () => ({ kind: 'inactive' as const }) },
			}),
		).resolves.toEqual({ kind: 'failed', reason: 'environment-not-connected' });
		expect(prepare).not.toHaveBeenCalled();

		await expect(
			createDesktopSshEnvironmentRemoteTerminal({
				...base,
				connections: connectedConnections(),
				environmentId: otherEnvironmentId,
				registry: registry(otherEnvironmentId),
			}),
		).resolves.toEqual({ kind: 'failed', reason: 'workspace-environment-mismatch' });
		expect(prepare).toHaveBeenCalledOnce();
		expect(execute).not.toHaveBeenCalled();
		expect(persistence.store.commit).not.toHaveBeenCalled();
	});

	it('cancels before queue admission and reports post-rename durability uncertainty', async () => {
		const cancelledPersistence = createStore(remoteDocument());
		const cancelledCoordinator = await createCoordinator(cancelledPersistence.store, vi.fn());
		const controller = new AbortController();

		await expect(
			createDesktopSshEnvironmentRemoteTerminal({
				allocateIdentifier: () => 'b'.repeat(32),
				connections: connectedConnections(),
				coordinator: cancelledCoordinator,
				environmentId,
				projectId: 'project-1',
				requestId: DesktopRemoteTerminalCreationRequestIdSchema.parse('remote-terminal:cancel'),
				registry: registry(),
				runtimeOwner: {
					prepare: async () => {
						controller.abort();
						return { activation: 'started', kind: 'ready' };
					},
				},
				signal: controller.signal,
				terminalTitle: 'Remote Terminal',
				workspaceId: 'workspace-1',
			}),
		).resolves.toEqual({ kind: 'cancelled' });
		expect(cancelledPersistence.store.commit).not.toHaveBeenCalled();

		const uncertainPersistence = createStore(remoteDocument(), true);
		const effect =
			vi.fn<ConstructorParameters<typeof DesktopTerminalIntentReconciler>[0]['execute']>();
		const uncertainCoordinator = await createCoordinator(uncertainPersistence.store, effect);
		await expect(
			createDesktopSshEnvironmentRemoteTerminal({
				allocateIdentifier: () => 'c'.repeat(32),
				connections: connectedConnections(),
				coordinator: uncertainCoordinator,
				environmentId,
				projectId: 'project-1',
				requestId: DesktopRemoteTerminalCreationRequestIdSchema.parse('remote-terminal:uncertain'),
				registry: registry(),
				runtimeOwner: {
					prepare: async () => ({ activation: 'started', kind: 'ready' }),
				},
				terminalTitle: 'Remote Terminal',
				workspaceId: 'workspace-1',
			}),
		).resolves.toEqual({ kind: 'indeterminate', reason: 'durability-unconfirmed' });
		expect(uncertainPersistence.current().revision).toBe('5');
		expect(effect).not.toHaveBeenCalled();
	});
});

function remoteDocument(): WorkbenchSharedDocument {
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextProjectSequence: 2,
			nextTerminalContextSequence: 3,
			nextWorkspaceSequence: 2,
			projects: [
				{
					directories: [
						{
							available: false,
							git: null,
							id: 'environment-directory-1',
							kind: 'folder',
							name: 'orchbee',
							path: '/srv/orchbee',
							placement: { environmentId, kind: 'environment' },
							source: 'selected',
						},
					],
					id: 'project-1',
					launchConfigurations: [],
					name: 'orchbee',
					primaryDirectoryId: 'environment-directory-1',
					workspaces: [
						{
							id: 'workspace-1',
							launchConfigurationId: null,
							name: 'orchbee',
							primaryDirectoryId: 'environment-directory-1',
							terminalContextId: 'terminal-context-2',
						},
					],
				},
			],
			standaloneTerminalContextId: 'terminal-context-1',
		},
		revision: workbenchSharedDocumentRevision('4'),
		terminalContexts: [
			createEmptyWorkbenchSharedTerminalContext('terminal-context-1'),
			createEmptyWorkbenchSharedTerminalContext('terminal-context-2'),
		],
	});
}

function createStore(
	initial: WorkbenchSharedDocument,
	indeterminate = false,
	onCommit: () => void = () => undefined,
) {
	let current = initial;
	let uncertain = indeterminate;
	const store: WorkbenchDocumentStore = {
		commit: vi.fn<WorkbenchDocumentStore['commit']>(async (expectedRevision, document) => {
			expect(expectedRevision).toBe(current.revision);
			onCommit();
			current = document;
			if (uncertain) {
				uncertain = false;
				return { kind: 'unavailable', reason: 'write-indeterminate' };
			}
			return { kind: 'committed', revision: document.revision };
		}),
		load: vi
			.fn<WorkbenchDocumentStore['load']>()
			.mockImplementation(async () => ({ document: current, kind: 'available' })),
	};
	return { current: () => current, store };
}

async function createCoordinator(
	store: WorkbenchDocumentStore,
	execute: ConstructorParameters<typeof DesktopTerminalIntentReconciler>[0]['execute'],
): Promise<DesktopClientSessionCoordinator> {
	let restoreSequence = 0;
	const reconciler = new DesktopTerminalIntentReconciler({
		createOperationId: () => terminalPresentationOperationId(`remote-restore-${++restoreSequence}`),
		execute,
		getTerminalOutcome: () => null,
	});
	const coordinator = new DesktopClientSessionCoordinator({
		createClientSessionId: () => 'remote-terminal-client',
		defaultCwd: '/local',
		getTerminalEvents: () => reconciler.getTerminalEvents(),
		initialPresentationId: terminalPresentationId('initial-presentation'),
		initializationDocument: remoteDocument(),
		reconcileDocument: (document, publish) => reconciler.reconcileDocument(document, publish),
		store,
		terminalHostName: 'desktop',
		validateDocument: (document) =>
			WorkbenchSharedDocumentSchema.safeParse(document).success &&
			reconciler.validateDocument(document),
	});
	expect(await coordinator.start()).toEqual({ kind: 'ready' });
	return coordinator;
}

function registry(
	registeredEnvironmentId = environmentId,
): Pick<DesktopEnvironmentRegistry, 'get'> {
	return {
		get: vi.fn<DesktopEnvironmentRegistry['get']>(async () => ({
			kind: 'ready',
			value: { ...environment, environmentId: registeredEnvironmentId },
		})),
	};
}

function connectedConnections(): {
	readonly observation: () => {
		readonly kind: 'ready';
		readonly observation: SshEnvironmentConnectionObservation;
	};
} {
	return {
		observation: () => ({
			kind: 'ready' as const,
			observation: { connected: true } as SshEnvironmentConnectionObservation,
		}),
	};
}
