import { EventEmitter } from 'node:events';
import { mkdir, mkdtemp, readFile, rename, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	DesktopAgentIntegrationManagementSnapshotSchema,
	DesktopRemoteTerminalCreationRequestIdSchema,
	DesktopTerminalDefaultSettingsSnapshotSchema,
	DesktopTerminalLaunchProfileSnapshotSchema,
} from '@orchbee/desktop-client-protocol';
import { PersonalAccessTokenSchema } from '@orchbee/remote-client-protocol';
import { SshProjectRootPathSchema } from '@orchbee/ssh-environment-protocol';
import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	CURRENT_PROTOCOL_VERSION,
	HostEpochSchema,
	MessageKind,
	PayloadEncoding,
	PROTOCOL_LIMITS,
	TerminalIdSchema,
	type DecodedProtocolEnvelope,
	type TerminalExited,
	type TerminalFailed,
	type TerminalIdentity,
	type TerminalApplicationSignal,
	type TerminalMetadataChanged,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';
import {
	terminalPresentationId,
	type TerminalPresentationEvent,
} from '@orchbee/workbench/terminal-presentation';
import {
	createEmptyWorkbenchSharedTerminalContext,
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import type { WorkbenchTerminalMetadata } from '@orchbee/workbench-client-protocol';
import type { BrowserWindow, BrowserWindowConstructorOptions, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import { createDefaultTerminalProfile, type HostConnection } from '../../host';
import type { DesktopApplicationLifetimeBrokerDependencies } from '../../application-lifetime';
import {
	DesktopClientSessionBroker,
	DesktopClientSessionCoordinator,
	DesktopTerminalIntentReconciler,
} from '../../client-session';
import { DesktopEnvironmentRegistry, EnvironmentRecordSchema } from '../../environment';
import { SshEnvironmentTargetReferenceSchema } from '../../ssh-environment';
import type { WorkbenchDocumentStore } from '../../workbench-document';
import type { DesktopWindowStateStore } from '../../window-state';
import {
	TerminalExitSpillStore,
	type TerminalExitPublicationLedger,
	type TerminalExitSpillReason,
} from '../../recovery';
import {
	composeDesktopTerminalRuntime,
	type DesktopTerminalRuntime,
} from '../composeDesktopTerminalRuntime';
import {
	DesktopApplication,
	type DesktopApplicationDependencies,
	type DesktopApplicationOptions,
} from '../DesktopApplication';
import { resolveDesktopRuntimeConfiguration } from '../resolveDesktopRuntimeConfiguration';

const identity = {
	creationId: CreationIdSchema.parse('11'.repeat(16)),
	hostEpoch: HostEpochSchema.parse('22'.repeat(16)),
	terminalId: TerminalIdSchema.parse('33'.repeat(16)),
};
const presentationId = terminalPresentationId('presentation-recovered');
const mountedGeometry = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 96, rows: 32 } as const;
type EnvironmentRegistryStore = ConstructorParameters<typeof DesktopEnvironmentRegistry>[0];
type EnvironmentRegistryDocument = Extract<
	Awaited<ReturnType<EnvironmentRegistryStore['load']>>,
	{ readonly kind: 'ready' }
>['document'];

class TestMainPort extends EventEmitter {
	readonly close = vi.fn<() => void>();
	readonly postMessage = vi.fn<(value: unknown) => void>();
	readonly start = vi.fn<() => void>();
}

describe('DesktopApplication', () => {
	it('routes only a bounded terminal Agent launch hint to integration management', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();

		harness.hostEnvelopeListener?.(
			terminalApplicationSignalEnvelope('{"integrationId":"claude-code","version":3}'),
		);
		harness.hostEnvelopeListener?.(
			terminalApplicationSignalEnvelope(
				'{"command":"claude --prompt secret","integrationId":"claude-code","version":3}',
				'2',
			),
		);

		expect(harness.agentIntegrationManagementController.observeLaunch).toHaveBeenCalledOnce();
		expect(harness.agentIntegrationManagementController.observeLaunch).toHaveBeenCalledWith(
			'claude-code',
		);
		await application.shutdown();
	});

	it('composes durable Environment identity without resolving or opening its SSH target', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const environment = EnvironmentRecordSchema.parse({
			environmentId: `environment:${'4'.repeat(32)}`,
			name: 'Build machine',
			provider: {
				kind: 'ssh',
				runtimeInstanceId: '5'.repeat(32),
				target: { configurationFilePath: null, destination: 'build-host' },
			},
		});

		await expect(application.createEnvironment(environment)).resolves.toEqual({
			environment,
			kind: 'created',
			revision: 1,
		});
		await expect(application.listEnvironments()).resolves.toEqual({
			kind: 'ready',
			value: [environment],
		});
		await expect(application.getEnvironment(environment.environmentId)).resolves.toEqual({
			kind: 'ready',
			value: environment,
		});
		expect(harness.sshEnvironmentTargetResolver.resolve).not.toHaveBeenCalled();
		expect(harness.sshEnvironmentConnectionOwner.connect).not.toHaveBeenCalled();
		await application.shutdown();
	});

	it('registers an enrolled SSH Environment before admitting its canonical remote Project', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		const resolvedTarget = {
			destination: SshEnvironmentTargetReferenceSchema.parse({
				configurationFilePath: null,
				destination: 'build-host',
			}).destination,
			hostKeyLookupName: 'build-host',
			hostKeyPolicy: 'ask' as const,
			hostname: 'build-host',
			port: 22,
			user: 'builder',
		};
		let connectedObservation:
			| ReturnType<DesktopApplication['observeSshEnvironmentConnection']>
			| undefined;
		harness.sshEnvironmentTargetResolver.resolve.mockResolvedValue({
			kind: 'resolved',
			target: resolvedTarget,
		});
		harness.sshEnvironmentConnectionOwner.claimInteractionLease.mockReturnValue({
			kind: 'claimed',
			lease: { release: vi.fn<() => void>() },
		});
		harness.sshEnvironmentConnectionOwner.connect.mockImplementation(async (environmentId) => {
			const stopped = application.createSshEnvironmentConnectionState({
				connectionId: environmentId,
				target: { configurationFilePath: null, destination: 'build-host' },
			});
			const started = application.transitionSshEnvironmentConnection(stopped, {
				type: 'start-requested',
			});
			const resolving = application.transitionSshEnvironmentConnection(started.state, {
				generation: 1,
				resolvedTarget,
				type: 'target-resolved',
			});
			const ready = application.transitionSshEnvironmentConnection(resolving.state, {
				generation: 1,
				loginShell: '/bin/zsh',
				runtimeEpoch: 'a'.repeat(32),
				runtimeInstanceId: 'b'.repeat(32),
				type: 'remote-runtime-ready',
			});
			connectedObservation = application.observeSshEnvironmentConnection(ready.state);
			return {
				activation: 'started' as const,
				kind: 'ready' as const,
				observation: connectedObservation,
			};
		});
		harness.sshEnvironmentConnectionOwner.inspectProjectRoot.mockResolvedValue({
			canonicalPath: SshProjectRootPathSchema.parse('/srv/orchbee'),
			kind: 'resolved',
		});
		harness.sshEnvironmentConnectionOwner.listProjectDirectory.mockResolvedValue({
			canonicalPath: SshProjectRootPathSchema.parse('/srv/orchbee'),
			directories: [],
			kind: 'listed',
			parentPath: SshProjectRootPathSchema.parse('/srv'),
			truncated: false,
		});
		harness.sshEnvironmentConnectionOwner.observation.mockImplementation(() =>
			connectedObservation === undefined
				? { kind: 'inactive' }
				: { kind: 'ready', observation: connectedObservation },
		);
		harness.sshEnvironmentConnectionOwner.subscribe.mockReturnValue(() => undefined);
		harness.workbenchDocumentStore.commit.mockImplementation(async (_revision, document) => ({
			kind: 'committed',
			revision: document.revision,
		}));

		await application.start();
		await proposeInitializationDocument(harness, 'initialize-for-ssh-enrollment');
		const enrollmentRuntime = harness.sshProjectEnrollmentBrokers[0]?.runtime;
		if (enrollmentRuntime === undefined) throw new Error('Expected the SSH enrollment runtime.');
		const result = await enrollmentRuntime.enroll(
			{ destination: 'build-host' },
			{
				interact: async () => ({ kind: 'cancelled' }),
				progress: () => undefined,
				selectDirectory: async () => ({ kind: 'selected' }),
			},
			new AbortController().signal,
		);
		expect(result).toEqual({
			canonicalPath: '/srv/orchbee',
			kind: 'created',
			projectId: 'project-1',
			workspaceId: 'workspace-1',
		});
		await expect(application.listEnvironments()).resolves.toMatchObject({
			kind: 'ready',
			value: [expect.objectContaining({ name: 'build-host' })],
		});
		expect(harness.sshEnvironmentConnectionOwner.listProjectDirectory).toHaveBeenCalledWith(
			expect.any(String),
			null,
			expect.any(AbortSignal),
		);
		expect(enrollmentRuntime.statuses()).toEqual([
			{ blockedReason: null, projectId: 'project-1', state: 'connected' },
		]);

		harness.sshEnvironmentConnectionOwner.listProjectDirectory.mockResolvedValue({
			canonicalPath: SshProjectRootPathSchema.parse('/srv/shared'),
			directories: [],
			kind: 'listed',
			parentPath: SshProjectRootPathSchema.parse('/srv'),
			truncated: false,
		});
		harness.sshEnvironmentConnectionOwner.inspectProjectRoot.mockResolvedValue({
			canonicalPath: SshProjectRootPathSchema.parse('/srv/shared'),
			kind: 'resolved',
		});
		await expect(
			enrollmentRuntime.attachFolder(
				'project-1',
				{ destination: 'build-host' },
				{
					interact: async () => ({ kind: 'cancelled' }),
					progress: () => undefined,
					selectDirectory: async () => ({ kind: 'selected' }),
				},
				new AbortController().signal,
			),
		).resolves.toMatchObject({
			canonicalPath: '/srv/shared',
			kind: 'attached',
			projectId: 'project-1',
		});
		const attachedDocument = harness.workbenchDocumentStore.commit.mock.calls.at(-1)?.[1];
		expect(attachedDocument?.projectWorkspace.projects[0]).toMatchObject({
			directories: [
				{ path: '/srv/orchbee' },
				{ path: '/srv/shared', placement: { kind: 'environment' } },
			],
			primaryDirectoryId: expect.any(String),
			workspaces: [{ id: 'workspace-1' }],
		});
		await application.shutdown();
	});

	it('exposes explicit process-local SSH Environment activation and owns its shutdown', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const environmentId = EnvironmentRecordSchema.parse({
			environmentId: `environment:${'6'.repeat(32)}`,
			name: 'Build machine',
			provider: {
				kind: 'ssh',
				runtimeInstanceId: '7'.repeat(32),
				target: { configurationFilePath: null, destination: 'build-host' },
			},
		}).environmentId;
		const listener = vi.fn<(observation: unknown) => void>();
		harness.sshEnvironmentConnectionOwner.connect.mockResolvedValueOnce({ kind: 'not-found' });
		harness.sshEnvironmentConnectionOwner.inspectProjectRoot.mockResolvedValueOnce({
			kind: 'failed',
			reason: 'unavailable',
		});
		harness.sshEnvironmentConnectionOwner.openTerminalHost.mockResolvedValueOnce({
			kind: 'failed',
			reason: 'unavailable',
		});

		await expect(application.connectSshEnvironment(environmentId)).resolves.toEqual({
			kind: 'not-found',
		});
		await expect(application.disconnectSshEnvironment(environmentId)).resolves.toEqual({
			kind: 'inactive',
		});
		await expect(
			application.inspectSshEnvironmentProjectRoot(environmentId, '/srv/project'),
		).resolves.toEqual({ kind: 'failed', reason: 'unavailable' });
		await expect(application.openSshEnvironmentTerminalHost(environmentId)).resolves.toEqual({
			kind: 'failed',
			reason: 'unavailable',
		});
		expect(application.getSshEnvironmentConnection(environmentId)).toEqual({ kind: 'inactive' });
		expect(application.subscribeSshEnvironmentConnection(environmentId, listener)).toBeNull();
		expect(harness.sshEnvironmentConnectionOwner.connect).toHaveBeenCalledWith(environmentId);
		expect(harness.sshEnvironmentConnectionOwner.disconnect).toHaveBeenCalledWith(environmentId);
		expect(harness.sshEnvironmentConnectionOwner.inspectProjectRoot).toHaveBeenCalledWith(
			environmentId,
			'/srv/project',
			undefined,
		);
		expect(harness.sshEnvironmentConnectionOwner.openTerminalHost).toHaveBeenCalledWith(
			environmentId,
		);
		expect(harness.sshEnvironmentConnectionOwner.observation).toHaveBeenCalledWith(environmentId);
		expect(harness.sshEnvironmentConnectionOwner.subscribe).toHaveBeenCalledWith(
			environmentId,
			listener,
		);

		await application.shutdown();
		expect(harness.sshEnvironmentConnectionOwner.dispose).toHaveBeenCalledOnce();
	});

	it('provisions and projects a remote Workspace terminal without opening SSH during startup', async () => {
		const harness = createHarness(undefined, true);
		const environment = EnvironmentRecordSchema.parse({
			environmentId: `environment:${'8'.repeat(32)}`,
			name: 'Build machine',
			provider: {
				kind: 'ssh',
				runtimeInstanceId: '9'.repeat(32),
				target: { configurationFilePath: null, destination: 'build-host' },
			},
		});
		let canonical = remoteWorkspaceDocument(environment.environmentId);
		harness.environmentRegistryStore.load.mockResolvedValueOnce({
			document: { environments: [environment], revision: 1, schemaVersion: 2 },
			kind: 'ready',
		});
		harness.workbenchDocumentStore.load.mockResolvedValueOnce({
			document: canonical,
			kind: 'available',
		});
		harness.workbenchDocumentStore.commit.mockImplementation(async (_revision, document) => {
			canonical = document;
			return { kind: 'committed', revision: document.revision };
		});
		let remotePresentationId = presentationId;
		let remoteExit: TerminalExited | null = null;
		const remoteRuntime = {
			...harness.terminalRuntime,
			commitExitPublication: vi
				.fn<NonNullable<DesktopTerminalRuntime['commitExitPublication']>>()
				.mockReturnValue(true),
			dispose: vi.fn<DesktopTerminalRuntime['dispose']>().mockResolvedValue(undefined),
			execute: vi.fn<DesktopTerminalRuntime['execute']>(async (command) => ({
				operationId: command.operationId,
				presentationId: command.presentationId,
				type: 'terminal-provision-completed',
			})),
			findPresentationByIdentity: vi
				.fn<DesktopTerminalRuntime['findPresentationByIdentity']>()
				.mockImplementation(() => remotePresentationId),
			getDurableTerminalExit: vi
				.fn<DesktopTerminalRuntime['getDurableTerminalExit']>()
				.mockImplementation((candidatePresentationId) =>
					candidatePresentationId === remotePresentationId ? remoteExit : null,
				),
			initialize: vi
				.fn<DesktopTerminalRuntime['initialize']>()
				.mockResolvedValue({ kind: 'ready' }),
			open: vi.fn<DesktopTerminalRuntime['open']>().mockResolvedValue({
				dispose: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
			} as unknown as Awaited<ReturnType<DesktopTerminalRuntime['open']>>),
			recordExitOutcome: vi
				.fn<DesktopTerminalRuntime['recordExitOutcome']>()
				.mockImplementation(async (event) => {
					remoteExit = event;
					return {
						finalRevision: event.finalRevision,
						kind: 'durable',
						presentationId: remotePresentationId,
					};
				}),
			recordFailure: vi
				.fn<DesktopTerminalRuntime['recordFailure']>()
				.mockImplementation(async () => remotePresentationId),
		};
		harness.createTerminalRuntime
			.mockReturnValueOnce(harness.terminalRuntime)
			.mockReturnValueOnce(remoteRuntime);
		const application = createApplication(
			harness.dependencies,
			'linux',
			createRuntime({
				ORCHBEE_DESKTOP_GATEWAY: '1',
				ORCHBEE_GATEWAY_ENTRY_PATH: '/repo/apps/gateway/dist/main/gateway.cjs',
				SHELL: '/bin/sh',
			}),
		);
		const stopped = application.createSshEnvironmentConnectionState({
			connectionId: environment.environmentId,
			target: environment.provider.target,
		});
		const started = application.transitionSshEnvironmentConnection(stopped, {
			type: 'start-requested',
		});
		const resolving = application.transitionSshEnvironmentConnection(started.state, {
			generation: 1,
			resolvedTarget: {
				destination: stopped.target.destination,
				hostKeyLookupName: 'build-host',
				hostKeyPolicy: 'ask',
				hostname: 'build-host',
				port: 22,
				user: 'builder',
			},
			type: 'target-resolved',
		});
		const ready = application.transitionSshEnvironmentConnection(resolving.state, {
			generation: 1,
			loginShell: '/bin/zsh',
			runtimeEpoch: 'a'.repeat(32),
			runtimeInstanceId: environment.provider.runtimeInstanceId,
			type: 'remote-runtime-ready',
		});
		harness.sshEnvironmentConnectionOwner.observation.mockReturnValue({
			kind: 'ready',
			observation: application.observeSshEnvironmentConnection(ready.state),
		});
		harness.sshEnvironmentConnectionOwner.openTerminalHost.mockResolvedValue({
			connection: harness.host,
			kind: 'ready',
		});

		await application.start();
		expect(harness.sshEnvironmentConnectionOwner.openTerminalHost).not.toHaveBeenCalled();

		const created = await application.createSshEnvironmentRemoteTerminalForWorkspace(
			'project-1',
			'workspace-1',
			DesktopRemoteTerminalCreationRequestIdSchema.parse('remote-terminal:application'),
		);
		expect(created).toMatchObject({ kind: 'created', revision: '5' });
		if (created.kind !== 'created') throw new Error('Expected one created remote terminal.');
		remotePresentationId = created.presentationId;
		expect(harness.sshEnvironmentConnectionOwner.openTerminalHost).toHaveBeenCalledOnce();
		expect(remoteRuntime.execute).toHaveBeenCalledWith(
			expect.objectContaining({
				launchCwd: '/srv/orchbee',
				type: 'terminal-provision-requested',
			}),
		);
		const remoteRuntimeComposition = harness.createTerminalRuntime.mock.calls[1];
		expect(remoteRuntimeComposition?.[1].terminalBindingStorePath).toContain(
			`ssh-environments-v1/${'8'.repeat(32)}/bindings-v1.json`,
		);
		expect(harness.createTerminalExitSpillStore).toHaveBeenCalledWith(
			expect.stringContaining(`ssh-environments-v1/${'8'.repeat(32)}/terminal-exits-v1.json`),
		);
		expect(harness.createTerminalExitPublicationLedger).toHaveBeenCalledWith(
			expect.stringContaining(
				`ssh-environments-v1/${'8'.repeat(32)}/terminal-exit-publications-v1.json`,
			),
		);
		expect(remoteRuntimeComposition?.[5]()).toMatchObject({
			currentDirectory: null,
			executable: '/bin/sh',
		});
		expect(canonical.projectWorkspace.projects[0]?.directories[0]?.available).toBe(false);

		const desktopProjectionService = harness.projectionServices[0];
		if (desktopProjectionService === undefined) {
			throw new Error('Desktop projection service was not composed.');
		}
		const desktopLease = await desktopProjectionService.open(
			created.presentationId,
			mountedGeometry,
		);
		expect(
			desktopProjectionService.isPublicationTargetCurrent?.(created.presentationId, desktopLease),
		).toBe(true);
		expect(remoteRuntime.open).toHaveBeenCalledWith(created.presentationId, mountedGeometry, {
			allowArchivedFallback: false,
			brokerEffectTarget: 'desktop',
		});
		expect(harness.terminalRuntime.open).not.toHaveBeenCalled();

		const metadata = terminalMetadataChanged('20', 'remote background agent');
		harness.hostEnvelopeListener?.(terminalMetadataChangedEnvelope(metadata));
		await vi.waitFor(() =>
			expect(harness.clientSessionPorts[0]?.postMessage).toHaveBeenCalledWith(
				expect.objectContaining({
					metadata: { ...metadata, presentationId: created.presentationId },
					type: 'terminal-metadata-published',
				}),
			),
		);

		const exit: TerminalExited = {
			exit: { code: 23, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('21'),
			identity,
		};
		harness.hostEnvelopeListener?.(terminalExitedEnvelope(exit));
		await vi.waitFor(() => expect(remoteRuntime.recordExitOutcome).toHaveBeenCalledWith(exit));
		await vi.waitFor(() => expect(remoteRuntime.commitExitPublication).toHaveBeenCalledWith(exit));
		await vi.waitFor(() =>
			expect(terminalEventPublications(harness)).toContainEqual(
				expect.objectContaining({
					event: expect.objectContaining({
						exitCode: 23,
						presentationId: created.presentationId,
						type: 'terminal-exited',
					}),
				}),
			),
		);
		expect(harness.terminalRuntime.recordExitOutcome).not.toHaveBeenCalled();
		expect(harness.publishedExitLedgerEvents).toContainEqual(exit);

		const openClient = harness.createBrowserAccessRuntime.mock.calls[0]?.[1];
		if (openClient === undefined) throw new Error('Browser access client factory was not created.');
		const gatewayClient = await openClient();
		const gatewayProjectionService = harness.projectionServices[1];
		if (gatewayProjectionService === undefined) {
			throw new Error('Gateway projection service was not composed.');
		}
		const gatewayLease = await gatewayProjectionService.open(
			created.presentationId,
			mountedGeometry,
		);
		expect(
			gatewayProjectionService.isPublicationTargetCurrent?.(created.presentationId, gatewayLease),
		).toBe(true);
		expect(remoteRuntime.open).toHaveBeenLastCalledWith(created.presentationId, mountedGeometry, {
			allowArchivedFallback: false,
			brokerEffectTarget: 'client',
		});
		expect(harness.terminalRuntime.open).not.toHaveBeenCalled();

		await gatewayClient.dispose();
		await application.shutdown();
		expect(remoteRuntime.dispose).toHaveBeenCalledOnce();
	});

	it('owns the process-local SSH connection foundation without opening a transport', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		harness.sshEnvironmentTargetResolver.resolve.mockResolvedValueOnce({
			code: 'openssh-unavailable',
			kind: 'failed',
		});

		const stopped = application.createSshEnvironmentConnectionState({
			connectionId: 'connection-1',
			target: { configurationFilePath: null, destination: 'build-host' },
		});
		const target = stopped.target;
		const started = application.transitionSshEnvironmentConnection(stopped, {
			type: 'start-requested',
		});

		expect(application.observeSshEnvironmentConnection(started.state)).toMatchObject({
			connected: false,
			generation: 1,
			phase: 'resolving',
		});
		await expect(application.resolveSshEnvironmentTarget(target)).resolves.toEqual({
			code: 'openssh-unavailable',
			kind: 'failed',
		});
		expect(harness.sshEnvironmentTargetResolver.resolve).toHaveBeenCalledWith(target);
		expect(harness.dependencies.connectHost).not.toHaveBeenCalled();
	});

	it('publishes only the accepted Workbench and Agent operations through one local endpoint', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);

		await application.start();

		expect(harness.semanticCommandLocalEndpoints).toHaveLength(1);
		const endpoint = harness.semanticCommandLocalEndpoints[0];
		if (endpoint === undefined) throw new Error('Expected one semantic command endpoint.');
		expect(endpoint.start).toHaveBeenCalledOnce();
		expect(
			endpoint.options.service
				.list({ kind: 'local-user' })
				.operations.map((operation) => operation.id),
		).toEqual([
			'agent.session.changes',
			'agent.session.explain',
			'agent.session.get',
			'agent.session.list',
			'agent.session.wait',
			'workbench.directory.list',
			'workbench.panel.list',
			'workbench.project.list',
			'workbench.tab.list',
			'workbench.terminal.create',
			'workbench.terminal.list',
			'workbench.terminal.output.read',
			'workbench.workspace.list',
		]);
		expect(endpoint.options.workerEntryPath).toContain(
			'semantic-command-local-endpoint-worker.cjs',
		);

		await application.shutdown();
		expect(endpoint.dispose).toHaveBeenCalledOnce();
	});

	it('uses the launch profile background for the pre-document window surface', async () => {
		const harness = createHarness();
		const runtime = createRuntime();
		const application = createApplication(harness.dependencies, 'linux', runtime);

		await application.start();

		expect(harness.windowOptions?.backgroundColor).toBe(
			`#${runtime.terminalLaunch.terminalProfile.defaultBackgroundRgb.toString(16).padStart(6, '0')}`,
		);
	});

	it('centers macOS traffic lights without reserving a blocking title-bar overlay', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies, 'darwin');

		await application.start();

		expect(harness.windowOptions).toMatchObject({
			titleBarStyle: 'hidden',
			trafficLightPosition: { x: 8, y: 10 },
		});
		expect(harness.windowOptions?.titleBarOverlay).toBeUndefined();
	});

	it('retains the title-bar overlay for Windows and Linux window controls', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies, 'linux');

		await application.start();

		expect(harness.windowOptions).toMatchObject({
			titleBarOverlay: true,
			titleBarStyle: 'hidden',
		});
		expect(harness.windowOptions?.trafficLightPosition).toBeUndefined();
	});

	it('restores safe persisted bounds and maximized state before showing the window', async () => {
		const harness = createHarness();
		harness.windowStateStore.load.mockResolvedValueOnce({
			kind: 'available',
			state: {
				bounds: { height: 640, width: 960, x: 240, y: 120 },
				maximized: true,
				version: 1,
			},
		});
		const application = createApplication(harness.dependencies);

		await application.start();

		expect(harness.windowOptions).toMatchObject({
			height: 640,
			width: 960,
			x: 240,
			y: 120,
		});
		expect(harness.browserWindow.maximize).toHaveBeenCalledOnce();
		await application.shutdown();
	});

	it('restores a minimized window before showing and focusing it', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		vi.mocked(harness.browserWindow.isMinimized).mockReturnValue(true);
		vi.mocked(harness.browserWindow.restore).mockClear();
		vi.mocked(harness.browserWindow.show).mockClear();
		vi.mocked(harness.browserWindow.focus).mockClear();

		application.focusWindow();

		expect(harness.browserWindow.restore).toHaveBeenCalledOnce();
		expect(harness.browserWindow.show).toHaveBeenCalledOnce();
		expect(harness.browserWindow.focus).toHaveBeenCalledOnce();
		const restoreOrder = vi.mocked(harness.browserWindow.restore).mock.invocationCallOrder[0];
		const showOrder = vi.mocked(harness.browserWindow.show).mock.invocationCallOrder[0];
		const focusOrder = vi.mocked(harness.browserWindow.focus).mock.invocationCallOrder[0];
		expect(restoreOrder).toBeLessThan(showOrder ?? -1);
		expect(showOrder).toBeLessThan(focusOrder ?? -1);
		await application.shutdown();
	});

	it('restores a minimized existing window when openWindow handles application activation', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies, 'darwin');
		await application.start();
		vi.mocked(harness.browserWindow.isMinimized).mockReturnValue(true);
		vi.mocked(harness.browserWindow.restore).mockClear();
		vi.mocked(harness.browserWindow.show).mockClear();
		vi.mocked(harness.browserWindow.focus).mockClear();

		await application.openWindow();

		expect(harness.createWindow).toHaveBeenCalledOnce();
		expect(harness.browserWindow.restore).toHaveBeenCalledOnce();
		expect(harness.browserWindow.show).toHaveBeenCalledOnce();
		expect(harness.browserWindow.focus).toHaveBeenCalledOnce();
		await application.shutdown();
	});

	it('exposes safe maximized and full-screen transitions for semantic actions', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();

		expect(application.getWindowState()).toBe('normal');
		expect(application.toggleWindowMaximized()).toBe(true);
		expect(harness.browserWindow.maximize).toHaveBeenCalledOnce();

		vi.mocked(harness.browserWindow.isMaximized).mockReturnValue(true);
		expect(application.getWindowState()).toBe('maximized');
		expect(application.toggleWindowMaximized()).toBe(true);
		expect(harness.browserWindow.unmaximize).toHaveBeenCalledOnce();

		vi.mocked(harness.browserWindow.isFullScreen).mockReturnValue(true);
		expect(application.getWindowState()).toBe('full-screen');
		expect(application.toggleWindowMaximized()).toBe(false);
		expect(harness.browserWindow.unmaximize).toHaveBeenCalledOnce();
		expect(application.toggleWindowFullScreen()).toBe(true);
		expect(harness.browserWindow.setFullScreen).toHaveBeenLastCalledWith(false);

		vi.mocked(harness.browserWindow.isFullScreen).mockReturnValue(false);
		expect(application.toggleWindowFullScreen()).toBe(true);
		expect(harness.browserWindow.setFullScreen).toHaveBeenLastCalledWith(true);
		await application.shutdown();
	});

	it('captures the final normal bounds and flushes them before shutdown completes', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies, 'linux');
		await application.start();
		harness.windowStateStore.save.mockClear();
		harness.windowStateStore.flush.mockClear();

		harness.browserWindow.emit('move');
		await application.shutdown();

		expect(harness.windowStateStore.save).toHaveBeenCalledWith({
			bounds: { height: 720, width: 1_024, x: 100, y: 100 },
			maximized: false,
			version: 1,
		});
		expect(harness.windowStateStore.flush).toHaveBeenCalledOnce();
	});

	it('registers the Host exit listener before terminal runtime reconciliation', async () => {
		const harness = createHarness();
		const initialization =
			createDeferred<Awaited<ReturnType<DesktopTerminalRuntime['initialize']>>>();
		harness.terminalRuntime.initialize.mockReturnValue(initialization.promise);
		const application = createApplication(harness.dependencies);
		const start = application.start();

		await vi.waitFor(() => expect(harness.host.onEnvelope).toHaveBeenCalledOnce());
		const event: TerminalExited = {
			exit: { kind: 'signal', signal: 15 },
			finalRevision: CanonicalUint64TextSchema.parse('1'),
			identity,
		};
		harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
		await vi.waitFor(() =>
			expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledWith(event),
		);

		initialization.resolve({ kind: 'ready' });
		await start;
		await application.shutdown();
	});

	it('buffers Host metadata during reconciliation and includes it in renderer bootstrap', async () => {
		const harness = createHarness();
		const initialization =
			createDeferred<Awaited<ReturnType<DesktopTerminalRuntime['initialize']>>>();
		harness.terminalRuntime.initialize.mockReturnValue(initialization.promise);
		const application = createApplication(harness.dependencies);
		const start = application.start();
		const metadata = terminalMetadataChanged('7', 'background agent');

		await vi.waitFor(() => expect(harness.host.onEnvelope).toHaveBeenCalledOnce());
		harness.hostEnvelopeListener?.(terminalMetadataChangedEnvelope(metadata));
		initialization.resolve({ kind: 'ready' });
		await start;

		expect(latestRendererBootstrap(harness)?.clientSession?.terminalMetadata).toEqual([
			{
				...metadata,
				presentationId,
			},
		]);
		await application.shutdown();
	});

	it('publishes Host metadata for a terminal without requiring a projected surface', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const metadata = terminalMetadataChanged('8', 'background agent');

		harness.hostEnvelopeListener?.(terminalMetadataChangedEnvelope(metadata));

		await vi.waitFor(() =>
			expect(harness.clientSessionPorts[0]?.postMessage).toHaveBeenCalledWith({
				metadata: { ...metadata, presentationId },
				sequence: '1',
				type: 'terminal-metadata-published',
			}),
		);
		await application.shutdown();
	});

	it('publishes a Host runtime failure as the retained presentation outcome', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		await proposeInitializationDocument(harness, 'terminal-runtime-failure');
		const event: TerminalFailed = {
			failure: { kind: 'pty-read', stage: 'read-output' },
			finalRevision: CanonicalUint64TextSchema.parse('9'),
			identity,
			process: { kind: 'observed', result: { code: 1, kind: 'exit-code' } },
		};
		harness.terminalRuntime.recordFailure.mockResolvedValueOnce(presentationId);

		harness.hostEnvelopeListener?.(terminalFailedEnvelope(event));

		await vi.waitFor(() =>
			expect(terminalEventPublications(harness, 'terminal-failed')).toContainEqual(
				expect.objectContaining({
					event: {
						failure: {
							code: 'terminal-runtime-pty-read-read-output',
							retryable: false,
						},
						presentationId,
						type: 'terminal-failed',
					},
				}),
			),
		);
		expect(harness.terminalRuntime.recordFailure).toHaveBeenCalledWith(event);
		await application.shutdown();
	});

	it('starts without creating an attachment and atomically transfers thirteen scoped controls', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);

		await application.start();

		expect(harness.windowOptions?.webPreferences).toMatchObject({
			contextIsolation: true,
			nodeIntegration: false,
			sandbox: true,
			webSecurity: true,
		});
		expect(harness.terminalRuntime.open).not.toHaveBeenCalled();
		expect(harness.browserWindow.loadURL).toHaveBeenCalledWith('http://127.0.0.1:3000/');
		expect(harness.webContents.postMessage).toHaveBeenCalledOnce();
		expect(harness.webContents.postMessage).toHaveBeenCalledWith(
			'orchbee.desktop.renderer-bootstrap',
			expect.objectContaining({
				clientSession: expect.objectContaining({
					defaultCwd: '/users/orchbee',
					documentState: { kind: 'missing' },
					terminalHostName: 'orchbee-host',
					type: 'orchbee.workbench.client-session-bootstrap',
				}),
				sshProjectEnrollmentAvailable: false,
				type: 'orchbee.desktop.renderer-bootstrap',
			}),
			[
				expect.anything(),
				expect.anything(),
				expect.anything(),
				expect.anything(),
				expect.anything(),
				expect.anything(),
				expect.anything(),
				expect.anything(),
				expect.anything(),
				expect.anything(),
				expect.anything(),
				expect.anything(),
				expect.anything(),
			],
		);
		expect(harness.clientSessionPorts[0]?.start).toHaveBeenCalledOnce();
		expect(harness.browserAccessBrokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.clipboardBrokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.projectionBrokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.projectFolderAcquisitionBrokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.projectWorktreeBrokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.remoteTerminalCreationBrokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.sshProjectEnrollmentBrokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.terminalLaunchProfileBrokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.terminalDefaultSettingsBrokers[0]?.start).toHaveBeenCalledOnce();
		expect(harness.terminalDefaultSettingsController.initialize).toHaveBeenCalledOnce();
		expect(harness.terminalLaunchProfileController.initialize).toHaveBeenCalledOnce();
		expect(harness.createTerminalRuntime.mock.calls[0]?.[5]?.()).toEqual(
			createRuntime().terminalLaunch,
		);
		expect(harness.createWorkbenchDocumentStore).toHaveBeenCalledWith(
			createRuntime().workbenchDocumentStorePath,
			createRuntime().workbenchDocumentLegacyStorePath,
		);
		expect(harness.createBrowserAccessRuntime).not.toHaveBeenCalled();
		expect(harness.requestControl).toHaveBeenCalledWith(
			MessageKind.setTerminalDefaults,
			{ configuration: { scrollbackBytes: 10_000_000 } },
			expect.anything(),
		);

		await application.shutdown();
		expect(harness.terminalLaunchProfileController.flush).toHaveBeenCalledOnce();
		expect(harness.terminalDefaultSettingsController.flush).toHaveBeenCalledOnce();
		expect(harness.terminalRuntime.dispose).toHaveBeenCalledOnce();
		expect(harness.host.close).toHaveBeenCalledOnce();
	});

	it('flushes terminal defaults even when launch-profile shutdown persistence fails', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		harness.terminalLaunchProfileController.flush.mockRejectedValueOnce(
			new Error('injected launch-profile flush failure'),
		);

		await expect(application.shutdown()).rejects.toThrow('injected launch-profile flush failure');

		expect(harness.terminalDefaultSettingsController.flush).toHaveBeenCalledOnce();
		expect(harness.terminalRuntime.dispose).toHaveBeenCalledOnce();
		expect(harness.host.close).toHaveBeenCalledOnce();
	});

	it('allows renderer reload navigation only within the configured renderer origin', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const allowedPreventDefault = vi.fn<() => void>();
		const blockedPreventDefault = vi.fn<() => void>();

		harness.webContents.emit(
			'will-navigate',
			{ preventDefault: allowedPreventDefault },
			'http://127.0.0.1:3000/terminal/tabs/terminal-context-1-tab-1',
		);
		harness.webContents.emit(
			'will-navigate',
			{ preventDefault: blockedPreventDefault },
			'http://127.0.0.1:3001/',
		);

		expect(allowedPreventDefault).not.toHaveBeenCalled();
		expect(blockedPreventDefault).toHaveBeenCalledOnce();
		await application.shutdown();
	});

	it('loads a packaged Browser Router path from the custom application origin', async () => {
		const harness = createHarness();
		const application = createApplication(
			harness.dependencies,
			'linux',
			createRuntime(),
			false,
			'orchbee://app/projects/project-1',
		);

		await application.start();

		expect(harness.browserWindow.loadURL).toHaveBeenCalledWith('orchbee://app/projects/project-1');
		await application.shutdown();
	});

	it('rolls back all renderer controls when bootstrap postMessage fails', async () => {
		const harness = createHarness();
		harness.webContents.postMessage.mockImplementationOnce(() => {
			throw new Error('renderer bootstrap post failed');
		});
		const application = createApplication(harness.dependencies);

		await expect(application.start()).rejects.toThrow('renderer bootstrap post failed');
		expect(harness.lifetimeBrokers[0]?.dispose).toHaveBeenCalledOnce();
		expect(harness.clientSessionPorts[0]?.close).toHaveBeenCalled();
		expect(harness.projectionBrokers[0]?.dispose).toHaveBeenCalledOnce();
		for (const channel of harness.messageChannels) {
			expect(channel.port1.close).toHaveBeenCalled();
			expect(channel.port2.close).toHaveBeenCalled();
		}
		expect(harness.terminalRuntime.dispose).toHaveBeenCalledOnce();
		expect(harness.host.close).toHaveBeenCalledOnce();
	});

	it('rolls back channels when a renderer broker factory fails', async () => {
		const harness = createHarness();
		harness.createProjectionControlBroker.mockImplementationOnce(() => {
			throw new Error('renderer projection broker factory failed');
		});
		const application = createApplication(harness.dependencies);

		await expect(application.start()).rejects.toThrow('renderer projection broker factory failed');
		expect(harness.messageChannels).toHaveLength(13);
		for (const channel of harness.messageChannels) {
			expect(channel.port1.close).toHaveBeenCalledOnce();
			expect(channel.port2.close).toHaveBeenCalledOnce();
		}
	});

	it('rolls back started renderer brokers when a broker start fails', async () => {
		const harness = createHarness();
		harness.createProjectionControlBroker.mockImplementationOnce(() => {
			const broker = {
				dispose: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
				start: vi.fn<() => void>(() => {
					throw new Error('renderer projection broker start failed');
				}),
			};
			harness.projectionBrokers.push(broker);
			return broker;
		});
		const application = createApplication(harness.dependencies);

		await expect(application.start()).rejects.toThrow('renderer projection broker start failed');
		expect(harness.lifetimeBrokers[0]?.dispose).toHaveBeenCalledOnce();
		expect(harness.clientSessionPorts[0]?.close).toHaveBeenCalled();
		expect(harness.projectionBrokers[0]?.dispose).toHaveBeenCalledOnce();
	});

	it('owns stopped Browser access resources and creates fresh page-scoped controls on demand', async () => {
		const harness = createHarness();
		const application = createApplication(
			harness.dependencies,
			'linux',
			createRuntime({
				ORCHBEE_DESKTOP_GATEWAY: '1',
				ORCHBEE_GATEWAY_ENTRY_PATH: '/repo/apps/gateway/dist/main/gateway.cjs',
			}),
		);

		await application.start();
		await proposeInitializationDocument(harness, 'gateway-local-projection');

		expect(harness.createBrowserAccessRuntime).toHaveBeenCalledWith(
			{
				gatewayEntryPath: '/repo/apps/gateway/dist/main/gateway.cjs',
				personalAccessTokenStorePath:
					'/users/orchbee/.config/orchbee/personal-access-token-v1.json',
				rendererRoot: '/repo/apps/web/dist',
				serverEntryPath: '/repo/apps/server/dist/main/server.cjs',
			},
			expect.any(Function),
		);
		expect(harness.browserAccessRuntime.start).not.toHaveBeenCalled();
		expect(harness.browserAccessBrokers[0]?.runtime).toBe(harness.browserAccessRuntime);

		const openClient = harness.createBrowserAccessRuntime.mock.calls[0]?.[1];
		if (openClient === undefined) throw new Error('Browser access client factory was not created.');
		const gatewayClient = await openClient();
		expect(harness.clientSessionPorts).toHaveLength(2);
		expect(harness.projectionBrokers).toHaveLength(2);
		expect(harness.lifetimeBrokers).toHaveLength(1);
		const desktopBootstrap = harness.webContents.postMessage.mock.calls[0]?.[1] as
			| { readonly clientSession?: { readonly clientSessionId?: string } }
			| undefined;
		expect(gatewayClient.bootstrap.clientSessionId).not.toBe(
			desktopBootstrap?.clientSession?.clientSessionId,
		);
		harness.terminalRuntime.open.mockResolvedValueOnce({
			dispose: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
		} as unknown as Awaited<ReturnType<DesktopTerminalRuntime['open']>>);
		await harness.projectionServices[1]?.open(presentationId, mountedGeometry);
		expect(harness.terminalRuntime.open).toHaveBeenCalledWith(presentationId, mountedGeometry, {
			brokerEffectTarget: 'client',
		});

		await gatewayClient.dispose();
		await application.shutdown();
		expect(harness.browserAccessRuntime.dispose).toHaveBeenCalledOnce();
	});

	it('retains Browser cleanup ownership and continues teardown when runtime disposal rejects', async () => {
		const harness = createHarness();
		const application = createApplication(
			harness.dependencies,
			'linux',
			createRuntime({
				ORCHBEE_DESKTOP_GATEWAY: '1',
				ORCHBEE_GATEWAY_ENTRY_PATH: '/repo/apps/gateway/dist/main/gateway.cjs',
			}),
		);
		await application.start();
		const openClient = harness.createBrowserAccessRuntime.mock.calls[0]?.[1];
		if (openClient === undefined) throw new Error('Browser access client factory was not created.');
		await openClient();
		harness.browserAccessRuntime.dispose.mockRejectedValueOnce(
			new Error('Browser access disposal failed'),
		);

		await expect(application.shutdown()).rejects.toThrow(
			'Desktop Browser access disposal did not converge',
		);
		expect(harness.projectionBrokers[1]?.dispose).toHaveBeenCalledOnce();
		expect(harness.terminalRuntime.dispose).toHaveBeenCalledOnce();
		expect(harness.host.close).toHaveBeenCalledOnce();

		await expect(application.shutdown()).resolves.toBeUndefined();
		expect(harness.browserAccessRuntime.dispose).toHaveBeenCalledTimes(2);
	});

	it('compensates Browser client channels when a broker factory or start fails', async () => {
		const gatewayRuntimeOptions = createRuntime({
			ORCHBEE_DESKTOP_GATEWAY: '1',
			ORCHBEE_GATEWAY_ENTRY_PATH: '/repo/apps/gateway/dist/main/gateway.cjs',
		});
		const factoryHarness = createHarness();
		const factoryApplication = createApplication(
			factoryHarness.dependencies,
			'linux',
			gatewayRuntimeOptions,
		);
		await factoryApplication.start();
		const factoryOpenClient = factoryHarness.createBrowserAccessRuntime.mock.calls[0]?.[1];
		if (factoryOpenClient === undefined)
			throw new Error('Browser access client factory was not created.');
		factoryHarness.createProjectionControlBroker.mockImplementationOnce(() => {
			throw new Error('gateway projection factory failed');
		});
		await expect(factoryOpenClient()).rejects.toThrow('gateway projection factory failed');
		expect(factoryHarness.messageChannels).toHaveLength(15);
		for (const channel of factoryHarness.messageChannels.slice(13)) {
			expect(channel.port1.close).toHaveBeenCalledOnce();
			expect(channel.port2.close).toHaveBeenCalledOnce();
		}
		await factoryApplication.shutdown();

		const startHarness = createHarness();
		const startApplication = createApplication(
			startHarness.dependencies,
			'linux',
			gatewayRuntimeOptions,
		);
		await startApplication.start();
		const startOpenClient = startHarness.createBrowserAccessRuntime.mock.calls[0]?.[1];
		if (startOpenClient === undefined)
			throw new Error('Browser access client factory was not created.');
		startHarness.createProjectionControlBroker.mockImplementationOnce(() => {
			const broker = {
				dispose: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
				start: vi.fn<() => void>(() => {
					throw new Error('gateway projection start failed');
				}),
			};
			startHarness.projectionBrokers.push(broker);
			return broker;
		});
		await expect(startOpenClient()).rejects.toThrow('gateway projection start failed');
		expect(startHarness.projectionBrokers[1]?.dispose).toHaveBeenCalledOnce();
		for (const channel of startHarness.messageChannels.slice(13)) {
			expect(channel.port1.close).toHaveBeenCalledOnce();
			expect(channel.port2.close).toHaveBeenCalled();
		}
		await startApplication.shutdown();
	});

	it('awaits the old client session attachment retirement before publishing reload controls', async () => {
		let finishRetirement: (() => void) | undefined;
		const harness = createHarness(
			() =>
				new Promise((resolve) => {
					finishRetirement = resolve;
				}),
		);
		const application = createApplication(harness.dependencies);
		await application.start();
		expect(harness.webContents.postMessage).toHaveBeenCalledOnce();

		emitRendererDocumentLoad(harness.webContents);
		await Promise.resolve();
		expect(harness.clientSessionPorts[0]?.close).toHaveBeenCalledOnce();
		expect(harness.projectionBrokers[0]?.dispose).toHaveBeenCalledOnce();
		expect(harness.webContents.postMessage).toHaveBeenCalledOnce();

		finishRetirement?.();
		await vi.waitFor(() => expect(harness.webContents.postMessage).toHaveBeenCalledTimes(2));
		expect(harness.clientSessionPorts).toHaveLength(2);
		expect(harness.projectionBrokers).toHaveLength(2);
		await application.shutdown();
	});

	it('preserves renderer controls across same-document and subframe navigation', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();

		harness.webContents.emit('did-start-loading');
		harness.webContents.emit('did-start-navigation', {
			isMainFrame: true,
			isSameDocument: true,
			url: 'http://127.0.0.1:3000/terminal/tabs/terminal-context-1-tab-1',
		});
		harness.webContents.emit('did-start-loading');
		harness.webContents.emit('did-start-navigation', {
			isMainFrame: false,
			isSameDocument: false,
			url: 'http://127.0.0.1:3000/frame',
		});
		await new Promise((resolve) => setImmediate(resolve));

		expect(harness.clientSessionPorts[0]?.close).not.toHaveBeenCalled();
		expect(harness.projectionBrokers[0]?.dispose).not.toHaveBeenCalled();
		expect(harness.messageChannels).toHaveLength(13);
		expect(harness.webContents.postMessage).toHaveBeenCalledOnce();
		await application.shutdown();
	});

	it('fails closed across a rejected renderer retirement and bootstraps once after retry', async () => {
		const projectionDispose = vi
			.fn<() => Promise<void>>()
			.mockRejectedValueOnce(new Error('renderer projection retirement failed'))
			.mockResolvedValue(undefined);
		const harness = createHarness(projectionDispose);
		const application = createApplication(harness.dependencies);
		await application.start();
		expect(harness.webContents.postMessage).toHaveBeenCalledOnce();

		emitRendererDocumentLoad(harness.webContents);
		await vi.waitFor(() => expect(projectionDispose).toHaveBeenCalledTimes(2));
		await Promise.resolve();
		expect(harness.webContents.postMessage).toHaveBeenCalledOnce();
		expect(harness.messageChannels).toHaveLength(13);
		expect(harness.projectionBrokers).toHaveLength(1);

		emitRendererDocumentLoad(harness.webContents);
		await vi.waitFor(() => expect(harness.webContents.postMessage).toHaveBeenCalledTimes(2));
		expect(harness.messageChannels).toHaveLength(26);
		expect(harness.projectionBrokers).toHaveLength(2);
		await application.shutdown();
	});

	it('places recovered terminals in the shared initialization document without eager projection', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();

		const bootstrap = harness.webContents.postMessage.mock.calls.find(
			(call) => call[0] === 'orchbee.desktop.renderer-bootstrap',
		)?.[1] as { readonly clientSession?: unknown } | undefined;
		expect(bootstrap?.clientSession).toMatchObject({
			initializationDocument: {
				terminalContexts: [
					expect.objectContaining({
						panelWorkspace: expect.objectContaining({
							tabs: [
								expect.objectContaining({
									root: expect.objectContaining({
										primaryPresentation: expect.objectContaining({
											intent: { kind: 'retain' },
											presentationId,
										}),
									}),
								}),
							],
						}),
					}),
				],
			},
		});
		expect(harness.terminalRuntime.open).not.toHaveBeenCalled();
		await application.shutdown();
	});

	it('routes a Host exit through the authoritative presentation registry', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const bootstrap = harness.webContents.postMessage.mock.calls.find(
			(call) => call[0] === 'orchbee.desktop.renderer-bootstrap',
		)?.[1] as
			| { readonly clientSession?: { readonly initializationDocument?: unknown } }
			| undefined;
		harness.workbenchDocumentStore.commit.mockResolvedValueOnce({
			kind: 'committed',
			revision: workbenchSharedDocumentRevision('0'),
		});
		harness.clientSessionPorts[0]?.emit('message', {
			data: {
				document: bootstrap?.clientSession?.initializationDocument,
				expectedRevision: null,
				requestId: 'proposal-initial',
				type: 'shared-document-proposed',
			},
			ports: [],
		});
		await vi.waitFor(() => {
			expect(harness.clientSessionPorts[0]?.postMessage).toHaveBeenCalledWith(
				expect.objectContaining({ type: 'shared-document-published' }),
			);
		});
		const terminalExit: TerminalExited = {
			exit: { kind: 'signal', signal: 15 },
			finalRevision: CanonicalUint64TextSchema.parse('12'),
			identity,
		};
		harness.terminalRuntime.recordExitOutcome
			.mockResolvedValueOnce({
				kind: 'published',
				presentationId,
			})
			.mockResolvedValue({ kind: 'ignored' });

		harness.hostEnvelopeListener?.(terminalExitedEnvelope(terminalExit));

		await vi.waitFor(() => {
			expect(harness.clientSessionPorts[0]?.postMessage).toHaveBeenCalledWith(
				expect.objectContaining({
					event: {
						exitCode: null,
						presentationId,
						signal: 'SIGTERM',
						type: 'terminal-exited',
					},
					type: 'terminal-presentation-event-published',
				}),
			);
		});
		harness.hostEnvelopeListener?.(terminalExitedEnvelope(terminalExit));
		await vi.waitFor(() =>
			expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledOnce(),
		);
		expect(
			harness.clientSessionPorts[0]?.postMessage.mock.calls.filter(([value]) => {
				const publication = value as
					| {
							readonly event?: {
								readonly presentationId?: unknown;
								readonly type?: unknown;
							};
							readonly type?: unknown;
					  }
					| undefined;
				return (
					publication?.type === 'terminal-presentation-event-published' &&
					publication.event?.type === 'terminal-exited' &&
					publication.event.presentationId === presentationId
				);
			}).length,
		).toBe(1);
		await application.shutdown();
	});

	it.each(['false', 'throw'] as const)(
		'keeps an exit out of bootstrap until the publication ledger mark succeeds (%s)',
		async (failureMode) => {
			const harness = createHarness(undefined, true);
			const application = createApplication(harness.dependencies);
			await application.start();
			await proposeInitializationDocument(harness, `publication-ledger-${failureMode}`);
			const event = terminalExitFor(101);
			harness.terminalRuntime.recordExitOutcome.mockResolvedValue({
				finalRevision: event.finalRevision,
				kind: 'durable',
				presentationId,
			});
			let allowPublication = false;
			harness.terminalExitPublicationLedger.markPublished.mockImplementation((publishedEvent) => {
				if (!allowPublication) {
					if (failureMode === 'throw') throw new Error('publication ledger write failed');
					return false;
				}
				harness.publishedExitLedgerEvents.push(publishedEvent);
				return true;
			});

			harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
			await vi.waitFor(() =>
				expect(harness.terminalExitPublicationLedger.markPublished).toHaveBeenCalled(),
			);
			expect(harness.publishedExitLedgerEvents).toEqual([]);
			expect(terminalEventPublications(harness)).toHaveLength(0);
			expect(harness.terminalExitSpillStore.list()).toContainEqual(
				expect.objectContaining({ event, reason: 'host-admission', state: 'pending' }),
			);

			emitRendererDocumentLoad(harness.webContents);
			await vi.waitFor(() => expect(harness.webContents.postMessage).toHaveBeenCalledTimes(2));
			const reloadBootstrap = latestRendererBootstrap(harness);
			expect(reloadBootstrap?.clientSession?.terminalEvents ?? []).not.toContainEqual(
				expect.objectContaining({ type: 'terminal-exited', presentationId }),
			);

			allowPublication = true;
			await vi.waitFor(() => expect(terminalEventPublications(harness)).toHaveLength(1));
			expect(harness.publishedExitLedgerEvents).toEqual([event]);
			expect(harness.terminalExitSpillStore.list()).toEqual([]);
			const attemptsAfterSuccess = harness.terminalRuntime.recordExitOutcome.mock.calls.length;
			harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
			await new Promise((resolve) => setImmediate(resolve));
			expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledTimes(attemptsAfterSuccess);
			await application.shutdown();
		},
	);

	it('retains the Main owner when the live publication callback throws', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		await proposeInitializationDocument(harness, 'publication-callback-throws');
		const event = terminalExitFor(102);
		harness.terminalRuntime.recordExitOutcome.mockResolvedValue({
			finalRevision: event.finalRevision,
			kind: 'durable',
			presentationId,
		});
		let failLivePublication = true;
		harness.clientSessionPorts[0]?.postMessage.mockImplementation((value) => {
			const publication = value as { readonly event?: { readonly type?: string } };
			if (failLivePublication && publication.event?.type === 'terminal-exited') {
				failLivePublication = false;
				throw new Error('renderer publication failed');
			}
		});

		harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
		await vi.waitFor(() =>
			expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledOnce(),
		);
		await vi.waitFor(() => expect(harness.clientSessionPorts[0]?.close).toHaveBeenCalled());
		emitRendererDocumentLoad(harness.webContents);
		await vi.waitFor(() => expect(harness.webContents.postMessage).toHaveBeenCalledTimes(2));
		expect(terminalEventPublications(harness)).toHaveLength(1);
		expect(latestRendererBootstrap(harness)?.clientSession?.terminalEvents).toContainEqual(
			expect.objectContaining({ presentationId, type: 'terminal-exited' }),
		);
		expect(harness.terminalExitPublicationLedger.markPublished).toHaveBeenCalledTimes(1);
		expect(harness.terminalExitPublicationLedger.list()).toEqual([{ event }]);
		await application.shutdown();
	});

	it('keeps a real composed exit through association spill and restart without replay', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-desktop-composition-'));
		try {
			const runtime = createRuntime({
				ORCHBEE_DESKTOP_RECOVERY_DIRECTORY: directory,
				SHELL: '/bin/sh',
			});
			const firstHarness = createHarness(
				undefined,
				false,
				() => new TerminalExitSpillStore(runtime.terminalExitSpillPath),
			);
			installRealTerminalRuntime(firstHarness);
			const createResponse = createDeferred<TerminalIdentity>();
			configureRealHost(firstHarness, () => createResponse.promise, []);
			const firstApplication = createApplication(firstHarness.dependencies, 'linux', runtime);
			await firstApplication.start();
			const firstBootstrap = rendererBootstrap(firstHarness);
			const document = firstBootstrap?.clientSession?.initializationDocument as
				| WorkbenchSharedDocument
				| undefined;
			if (document === undefined) throw new Error('Real composition did not produce a document.');
			firstHarness.workbenchDocumentStore.commit.mockResolvedValueOnce({
				kind: 'committed',
				revision: workbenchSharedDocumentRevision('0'),
			});
			firstHarness.clientSessionPorts[0]?.emit('message', {
				data: {
					document,
					expectedRevision: null,
					requestId: 'real-composition-proposal',
					type: 'shared-document-proposed',
				},
				ports: [],
			});
			await vi.waitFor(() =>
				expect(
					firstHarness.requestControl.mock.calls.some(
						([kind]) => kind === MessageKind.createTerminal,
					),
				).toBe(true),
			);
			const createRequest = firstHarness.requestControl.mock.calls.find(
				([kind]) => kind === MessageKind.createTerminal,
			)?.[1] as { readonly creationId: string } | undefined;
			if (createRequest === undefined)
				throw new Error('Real composition create request was not observed.');
			const event: TerminalExited = {
				exit: { code: 17, kind: 'exit-code' },
				finalRevision: CanonicalUint64TextSchema.parse('2'),
				identity: {
					...identity,
					creationId: CreationIdSchema.parse(createRequest.creationId),
				},
			};
			firstHarness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
			await vi.waitFor(async () => {
				const spill = new TerminalExitSpillStore(runtime.terminalExitSpillPath);
				await spill.load();
				expect(spill.list()).toEqual(
					expect.arrayContaining([
						expect.objectContaining({ event, reason: 'association-pending', state: 'pending' }),
					]),
				);
			});
			createResponse.resolve(event.identity);
			await vi.waitFor(() =>
				expect(
					firstHarness.clientSessionPorts[0]?.postMessage.mock.calls.filter(
						([value]) =>
							(value as { readonly event?: { readonly type?: string } } | undefined)?.event
								?.type === 'terminal-exited',
					).length,
				).toBe(1),
			);
			await firstApplication.shutdown();

			const secondHarness = createHarness(
				undefined,
				false,
				() => new TerminalExitSpillStore(runtime.terminalExitSpillPath),
			);
			installRealTerminalRuntime(secondHarness);
			configureRealHost(secondHarness, async () => identity, [terminalSummary(event)]);
			secondHarness.workbenchDocumentStore.load.mockResolvedValue({
				document,
				kind: 'available',
			});
			const secondApplication = createApplication(secondHarness.dependencies, 'linux', runtime);
			await secondApplication.start();
			expect(secondHarness.terminalExitPublicationLedger.list()).toEqual([{ event }]);
			secondHarness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
			await new Promise((resolve) => setImmediate(resolve));
			const secondBootstrap = rendererBootstrap(secondHarness);
			const secondClientSession = secondBootstrap?.clientSession;
			if (secondClientSession === undefined) throw new Error('Missing recovered client session.');
			const secondExitEvents = (secondClientSession.terminalEvents ?? []).filter(
				(candidate) => candidate.type === 'terminal-exited',
			);
			expect(secondExitEvents).toHaveLength(1);
			await secondApplication.shutdown();
		} finally {
			await rm(directory, { recursive: true, force: true });
		}
	});

	it('journals a Host exit before the synchronous listener returns', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-desktop-composition-'));
		try {
			const runtime = createRuntime({
				ORCHBEE_DESKTOP_RECOVERY_DIRECTORY: directory,
				SHELL: '/bin/sh',
			});
			const harness = createHarness(
				undefined,
				false,
				() => new TerminalExitSpillStore(runtime.terminalExitSpillPath),
			);
			const attempt =
				createDeferred<Awaited<ReturnType<DesktopTerminalRuntime['recordExitOutcome']>>>();
			harness.terminalRuntime.recordExitOutcome.mockReturnValue(attempt.promise);
			const application = createApplication(harness.dependencies, 'linux', runtime);
			await application.start();
			const event = terminalExitFor(103);

			harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
			const recovered = new TerminalExitSpillStore(runtime.terminalExitSpillPath);
			expect(await recovered.load()).toEqual({ kind: 'ready' });
			expect(recovered.list()).toEqual([
				expect.objectContaining({ event, reason: 'host-admission', state: 'pending' }),
			]);

			attempt.resolve({ kind: 'ignored' });
			await application.shutdown();
		} finally {
			await rm(directory, { recursive: true, force: true });
		}
	});

	it('records a durable rejection when the pending journal cannot admit an exit', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const event = terminalExitFor(104);
		harness.terminalExitSpillAdmit.mockImplementationOnce(() => {
			throw new Error('pending journal write failed');
		});

		harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
		expect(harness.terminalExitSpillStore.reject).toHaveBeenCalledWith(event, 'write-failed');
		expect(harness.terminalRuntime.recordExitOutcome).not.toHaveBeenCalled();
		await application.shutdown();
	});

	it('records a durable rejection when the pending journal is full', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const event = terminalExitFor(105);
		harness.terminalExitSpillAdmit.mockReturnValueOnce({ kind: 'full' });

		harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
		expect(harness.terminalExitSpillStore.reject).toHaveBeenCalledWith(
			event,
			'pending-journal-full',
		);
		expect(harness.terminalRuntime.recordExitOutcome).not.toHaveBeenCalled();
		await application.shutdown();
	});

	it('keeps a real composed exit owned while binding persistence is temporarily unavailable', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-desktop-composition-'));
		try {
			const runtime = createRuntime({
				ORCHBEE_DESKTOP_RECOVERY_DIRECTORY: directory,
				SHELL: '/bin/sh',
			});
			const harness = createHarness(
				undefined,
				false,
				() => new TerminalExitSpillStore(runtime.terminalExitSpillPath),
			);
			installRealTerminalRuntime(harness);
			configureRealHost(harness, async () => identity, []);
			const application = createApplication(harness.dependencies, 'linux', runtime, true);
			await application.start();
			const bootstrap = rendererBootstrap(harness);
			const document = bootstrap?.clientSession?.initializationDocument;
			if (document === undefined) throw new Error('Real composition did not produce a document.');
			harness.workbenchDocumentStore.commit.mockResolvedValueOnce({
				kind: 'committed',
				revision: workbenchSharedDocumentRevision('0'),
			});
			harness.clientSessionPorts[0]?.emit('message', {
				data: {
					document,
					expectedRevision: null,
					requestId: 'real-composition-persistence-proposal',
					type: 'shared-document-proposed',
				},
				ports: [],
			});
			await vi.waitFor(() =>
				expect(
					harness.requestControl.mock.calls.some(([kind]) => kind === MessageKind.createTerminal),
				).toBe(true),
			);
			const createRequest = harness.requestControl.mock.calls.find(
				([kind]) => kind === MessageKind.createTerminal,
			)?.[1] as { readonly creationId: string } | undefined;
			if (createRequest === undefined)
				throw new Error('Real composition create request was not observed.');
			const event: TerminalExited = {
				exit: { code: 18, kind: 'exit-code' },
				finalRevision: CanonicalUint64TextSchema.parse('2'),
				identity: {
					...identity,
					creationId: CreationIdSchema.parse(createRequest.creationId),
				},
			};
			await vi.waitFor(async () => {
				const stored = await readFile(runtime.terminalBindingStorePath, 'utf8');
				expect(stored).toContain(createRequest.creationId);
			});
			const backupPath = `${runtime.terminalBindingStorePath}.backup`;
			await rename(runtime.terminalBindingStorePath, backupPath);
			await mkdir(runtime.terminalBindingStorePath);
			harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
			await vi.waitFor(() =>
				expect(harness.dependencies.writeTrace).toHaveBeenCalledWith(
					expect.objectContaining({ reason: 'terminal-exit-durability' }),
				),
			);
			await rm(runtime.terminalBindingStorePath, { recursive: true });
			await rename(backupPath, runtime.terminalBindingStorePath);
			await vi.waitFor(() =>
				expect(
					harness.clientSessionPorts[0]?.postMessage.mock.calls.filter(
						([value]) =>
							(value as { readonly event?: { readonly type?: string } } | undefined)?.event
								?.type === 'terminal-exited',
					).length,
				).toBe(1),
			);
			harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
			await new Promise((resolve) => setImmediate(resolve));
			expect(
				harness.clientSessionPorts[0]?.postMessage.mock.calls.filter(
					([value]) =>
						(value as { readonly event?: { readonly type?: string } } | undefined)?.event?.type ===
						'terminal-exited',
				).length,
			).toBe(1);
			await application.shutdown();
		} finally {
			await rm(directory, { recursive: true, force: true });
		}
	});

	it('retries a failed Host exit through the Main envelope owner', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		harness.terminalRuntime.recordExitOutcome
			.mockResolvedValueOnce({
				code: 'terminal-exit-persistence-failed',
				kind: 'retryable',
				reason: 'durability',
			})
			.mockResolvedValueOnce({ kind: 'ignored' });

		harness.hostEnvelopeListener?.(
			terminalExitedEnvelope({
				exit: { kind: 'signal', signal: 15 },
				finalRevision: CanonicalUint64TextSchema.parse('12'),
				identity,
			}),
		);
		await vi.waitFor(() =>
			expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledTimes(2),
		);
		await application.shutdown();
	});

	it('does not spill an exit consumed by close retirement', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		harness.terminalRuntime.recordExitOutcome.mockResolvedValue({
			kind: 'ignored',
			reason: 'closed',
		});
		const event = terminalExitFor(90);

		harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
		await vi.waitFor(() =>
			expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledOnce(),
		);
		await new Promise((resolve) => setTimeout(resolve, 75));
		expect(harness.terminalExitSpillStore.admit).not.toHaveBeenCalledWith(
			event,
			'unknown-identity',
		);
		expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledOnce();

		await application.shutdown();
	});

	it('exposes bounded Main exit backpressure without evicting admitted identities', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies, 'linux', createRuntime(), true);
		await application.start();
		harness.terminalRuntime.recordExitOutcome.mockResolvedValue({
			code: 'terminal-exit-persistence-failed',
			kind: 'retryable',
			reason: 'durability',
		});

		for (let index = 0; index < 65; index += 1) {
			harness.hostEnvelopeListener?.(
				terminalExitedEnvelope({
					exit: { code: index, kind: 'exit-code' },
					finalRevision: CanonicalUint64TextSchema.parse('1'),
					identity: {
						creationId: identity.creationId,
						hostEpoch: identity.hostEpoch,
						terminalId: TerminalIdSchema.parse((index + 1).toString(16).padStart(32, '0')),
					},
				}),
			);
		}

		await vi.waitFor(() =>
			expect(harness.dependencies.writeTrace).toHaveBeenCalledWith(
				expect.objectContaining({ reason: 'terminal-exit-spilled' }),
			),
		);
		expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledTimes(64);
		expect(harness.terminalExitSpillStore.admit).toHaveBeenCalledWith(
			expect.objectContaining({ exit: { code: 64, kind: 'exit-code' } }),
			'queue-full',
		);

		harness.terminalRuntime.recordExitOutcome.mockResolvedValue({ kind: 'ignored' });
		await application.shutdown();
	});

	it('durably classifies an identity that cannot be associated without occupying Main retry slots', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		harness.terminalRuntime.recordExitOutcome.mockResolvedValue({
			code: 'terminal-exit-identity-unknown',
			kind: 'unattributed',
			reason: 'unknown-identity',
		});
		const event = terminalExitFor(91);

		harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
		await vi.waitFor(() =>
			expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledOnce(),
		);
		await vi.waitFor(() =>
			expect(harness.terminalExitSpillStore.admit).toHaveBeenCalledWith(event, 'unknown-identity'),
		);
		await new Promise((resolve) => setTimeout(resolve, 75));
		expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledOnce();

		const next = terminalExitFor(92);
		harness.hostEnvelopeListener?.(terminalExitedEnvelope(next));
		await vi.waitFor(() =>
			expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledTimes(2),
		);
		await application.shutdown();
	});

	it('does not timer-retry an unknown result after an exact observation', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const event = terminalExitFor(94);
		harness.terminalRuntime.recordExitOutcome.mockResolvedValue({
			code: 'terminal-exit-identity-unknown',
			kind: 'unattributed',
			reason: 'unknown-identity',
		});
		harness.identityObservedListener?.(event.identity);
		harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
		await vi.waitFor(() =>
			expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledOnce(),
		);
		await new Promise((resolve) => setTimeout(resolve, 75));
		expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledOnce();
		await application.shutdown();
	});

	it('parks association-pending exits until the exact identity is observed', async () => {
		const harness = createHarness(undefined, true);
		const application = createApplication(harness.dependencies);
		await application.start();
		let associated = false;
		harness.terminalRuntime.recordExitOutcome.mockImplementation(async () =>
			associated
				? { kind: 'published', presentationId }
				: {
						code: 'terminal-exit-association-pending',
						kind: 'retryable',
						reason: 'association',
					},
		);
		const event = terminalExitFor(92);

		harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
		await vi.waitFor(() =>
			expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledTimes(3),
		);
		await vi.waitFor(() =>
			expect(harness.terminalExitSpillAdmit).toHaveBeenCalledWith(event, 'association-pending'),
		);
		const attemptsBeforeAssociation = harness.terminalRuntime.recordExitOutcome.mock.calls.length;
		await new Promise((resolve) => setTimeout(resolve, 50));
		expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledTimes(
			attemptsBeforeAssociation,
		);

		associated = true;
		harness.identityObservedListener?.(event.identity);
		await vi.waitFor(() =>
			expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledTimes(
				attemptsBeforeAssociation + 1,
			),
		);
		await application.shutdown();
	});

	it('moves a retryable exit to durable shutdown ownership before tearing down the application', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		harness.terminalRuntime.recordExitOutcome.mockResolvedValue({
			code: 'terminal-exit-persistence-failed',
			kind: 'retryable',
			reason: 'durability',
		});
		const event = terminalExitFor(93);
		harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
		await vi.waitFor(() =>
			expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledOnce(),
		);

		await expect(application.shutdown()).resolves.toBeUndefined();
		expect(harness.terminalExitSpillStore.admit).toHaveBeenCalledWith(event, 'shutdown');
		expect(harness.terminalRuntime.dispose).toHaveBeenCalledOnce();
		expect(harness.host.close).toHaveBeenCalledOnce();
	});

	it('continues teardown when shutdown exit reconciliation cannot admit its durable handoff', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		harness.terminalRuntime.recordExitOutcome.mockResolvedValue({
			code: 'terminal-exit-persistence-failed',
			kind: 'retryable',
			reason: 'durability',
		});
		harness.terminalExitSpillAdmit.mockImplementation((_event, reason) => {
			if (reason === 'shutdown') throw new Error('shutdown spill unavailable');
			return { kind: 'admitted' };
		});
		const event = terminalExitFor(94);
		harness.hostEnvelopeListener?.(terminalExitedEnvelope(event));
		await vi.waitFor(() =>
			expect(harness.terminalRuntime.recordExitOutcome).toHaveBeenCalledOnce(),
		);

		await expect(application.shutdown()).rejects.toThrow('shutdown spill unavailable');
		expect(harness.terminalRuntime.dispose).toHaveBeenCalledOnce();
		expect(harness.host.close).toHaveBeenCalledOnce();
	});

	it('enters coordinator quiescing before acknowledging an application lifetime mode', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const bootstrap = harness.webContents.postMessage.mock.calls.find(
			(call) => call[0] === 'orchbee.desktop.renderer-bootstrap',
		)?.[1] as
			| { readonly clientSession?: { readonly initializationDocument?: unknown } }
			| undefined;

		expect(harness.lifetimeDependencies[0]?.prepareMode?.('preserve')).toBe(true);
		harness.clientSessionPorts[0]?.emit('message', {
			data: {
				document: bootstrap?.clientSession?.initializationDocument,
				expectedRevision: null,
				requestId: 'proposal-after-quiesce',
				type: 'shared-document-proposed',
			},
			ports: [],
		});

		await vi.waitFor(() => {
			expect(harness.clientSessionPorts[0]?.postMessage).toHaveBeenCalledWith(
				expect.objectContaining({
					result: expect.objectContaining({
						kind: 'rejected',
						reason: 'application-quiescing',
					}),
					type: 'shared-document-proposal-completed',
				}),
			);
		});
		await application.shutdown();
	});

	it('publishes the exact staged revision through the development restart plan callback', async () => {
		const harness = createHarness();
		const prepareDevelopmentHostRestart = vi.fn<
			NonNullable<DesktopApplicationOptions['prepareDevelopmentHostRestart']>
		>(async () => true);
		const application = new DesktopApplication(
			{
				platform: 'linux',
				preloadPath: '/repo/apps/desktop/dist/preload/preload.cjs',
				prepareDevelopmentHostRestart,
				rendererUrl: 'http://127.0.0.1:3000/',
				runtime: createRuntime(),
				semanticCommandEndpointWorkerPath:
					'/repo/apps/desktop/dist/main/semantic-command-local-endpoint-worker.cjs',
				sshEnvironment: createSshEnvironmentOptions(),
				singleInstanceLockOwned: true,
				traceCopyPath: false,
			},
			harness.dependencies,
		);
		await application.start();

		expect(
			await harness.lifetimeDependencies[0]?.prepareDevelopmentHostRestart?.(
				workbenchSharedDocumentRevision('8'),
			),
		).toBe(true);
		expect(prepareDevelopmentHostRestart).toHaveBeenCalledWith(
			workbenchSharedDocumentRevision('8'),
		);
		const restartPresentationIds = [terminalPresentationId('presentation-development-restart')];
		expect(
			harness.lifetimeDependencies[0]?.prepareMode?.(
				'development-host-restart',
				restartPresentationIds,
			),
		).toBe(true);
		expect(harness.lifetimeDependencies[0]?.getDevelopmentHostRestartPresentationIds?.()).toEqual(
			restartPresentationIds,
		);
		expect(
			harness.lifetimeDependencies[0]?.prepareMode?.('development-host-restart', [
				terminalPresentationId('different-presentation'),
			]),
		).toBe(false);
		expect(harness.lifetimeDependencies[0]?.getDevelopmentHostRestartPresentationIds?.()).toEqual(
			restartPresentationIds,
		);
		await application.shutdown();
	});

	it('quits only after the scoped lifetime broker authorizes the workflow', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		harness.lifetimeBrokers[0]?.requestExit.mockResolvedValue({
			forced: false,
			kind: 'ready',
			mode: 'preserve',
		});

		await application.requestExitAndQuit('user-quit');

		expect(application.quitPrepared).toBe(true);
		expect(harness.terminalRuntime.dispose).toHaveBeenCalledOnce();
		expect(harness.quitApplication).toHaveBeenCalledOnce();
	});

	it('reloads a stale renderer lifetime channel before preparing a development Host restart', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const restartPresentationIds = [terminalPresentationId('presentation-before-reload')];
		harness.lifetimeBrokers[0]?.requestExit.mockImplementation(async () => {
			expect(
				harness.lifetimeDependencies[0]?.prepareMode?.(
					'development-host-restart',
					restartPresentationIds,
				),
			).toBe(true);
			return { kind: 'unavailable' };
		});
		vi.mocked(harness.dependencies.createApplicationLifetimeBroker).mockImplementationOnce(
			(_port, brokerDependencies) => {
				harness.lifetimeDependencies.push(brokerDependencies);
				const broker = {
					dispose: vi.fn<() => void>(),
					requestExit: vi
						.fn<
							ReturnType<
								DesktopApplicationDependencies['createApplicationLifetimeBroker']
							>['requestExit']
						>()
						.mockResolvedValue({
							forced: false,
							kind: 'ready',
							mode: 'development-host-restart',
						}),
					start: vi.fn<() => void>(),
				};
				harness.lifetimeBrokers.push(broker);
				return broker;
			},
		);
		vi.mocked(harness.browserWindow.reload).mockImplementation(() => {
			emitRendererDocumentLoad(harness.webContents);
		});

		await application.requestExitAndQuit('development-host-restart');

		expect(harness.browserWindow.reload).toHaveBeenCalledOnce();
		expect(harness.lifetimeBrokers).toHaveLength(2);
		expect(harness.lifetimeBrokers[1]?.requestExit).toHaveBeenCalledWith(
			expect.stringMatching(/^desktop:exit:/u),
			'development-host-restart',
		);
		expect(harness.lifetimeDependencies[1]?.getDevelopmentHostRestartPresentationIds?.()).toEqual(
			restartPresentationIds,
		);
		expect(application.quitPrepared).toBe(true);
		expect(harness.quitApplication).toHaveBeenCalledOnce();
	});

	it('keeps the last Linux window alive while the user-quit decision is pending', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies);
		await application.start();
		const preventDefault = vi.fn<() => void>();

		harness.browserWindow.emit('close', { preventDefault });

		expect(preventDefault).toHaveBeenCalledOnce();
		expect(harness.lifetimeBrokers[0]?.requestExit).toHaveBeenCalledWith(
			expect.stringMatching(/^desktop:exit:/u),
			'user-quit',
		);
	});

	it('reopens a closed macOS document before requesting an application-quit decision', async () => {
		const harness = createHarness();
		const application = createApplication(harness.dependencies, 'darwin');
		await application.start();

		harness.browserWindow.emit('closed');
		await application.requestExitAndQuit('user-quit');

		expect(harness.createWindow).toHaveBeenCalledTimes(2);
		expect(harness.lifetimeBrokers).toHaveLength(2);
		expect(harness.lifetimeBrokers[1]?.requestExit).toHaveBeenCalledWith(
			expect.stringMatching(/^desktop:exit:/u),
			'user-quit',
		);
	});

	it('stops startup when the binding registry cannot initialize', async () => {
		const harness = createHarness();
		harness.terminalRuntime.initialize.mockResolvedValue({ kind: 'conflict' });
		const application = createApplication(harness.dependencies);

		await expect(application.start()).rejects.toThrow('initialization stopped with conflict');
		expect(harness.createWindow).not.toHaveBeenCalled();
		expect(harness.terminalRuntime.dispose).toHaveBeenCalledOnce();
		expect(harness.host.close).toHaveBeenCalledOnce();
	});

	it('stops before opening the local Host when the Environment registry is invalid', async () => {
		const harness = createHarness();
		harness.environmentRegistryStore.load.mockResolvedValueOnce({
			kind: 'unavailable',
			reason: 'invalid-record',
		});
		const application = createApplication(harness.dependencies);

		await expect(application.start()).rejects.toThrow(
			'Desktop Environment registry is unavailable: invalid-record.',
		);
		expect(harness.dependencies.connectHost).not.toHaveBeenCalled();
		expect(harness.createWindow).not.toHaveBeenCalled();
	});

	it('stops startup when the singleton Host does not accept the requested defaults', async () => {
		const harness = createHarness();
		harness.requestControl.mockResolvedValueOnce({
			configuration: { scrollbackBytes: 20_000_000 },
			revision: '2',
		});
		const application = createApplication(harness.dependencies);

		await expect(application.start()).rejects.toThrow('did not accept');
		expect(harness.terminalRuntime.initialize).not.toHaveBeenCalled();
		expect(harness.host.close).toHaveBeenCalledOnce();
	});

	it('requires ownership of the single-instance lock', () => {
		const harness = createHarness();
		expect(
			() =>
				new DesktopApplication(
					{
						platform: 'linux',
						preloadPath: '/preload.cjs',
						rendererUrl: 'http://127.0.0.1:3000/',
						runtime: createRuntime(),
						semanticCommandEndpointWorkerPath:
							'/repo/apps/desktop/dist/main/semantic-command-local-endpoint-worker.cjs',
						sshEnvironment: createSshEnvironmentOptions(),
						singleInstanceLockOwned: false,
						traceCopyPath: false,
					},
					harness.dependencies,
				),
		).toThrow('single-instance lock');
	});
});

function createApplication(
	dependencies: DesktopApplicationDependencies,
	platform: NodeJS.Platform = 'linux',
	runtime = createRuntime(),
	traceCopyPath = false,
	rendererUrl = 'http://127.0.0.1:3000/',
): DesktopApplication {
	return new DesktopApplication(
		{
			platform,
			preloadPath: '/repo/apps/desktop/dist/preload/preload.cjs',
			rendererUrl,
			runtime,
			semanticCommandEndpointWorkerPath:
				'/repo/apps/desktop/dist/main/semantic-command-local-endpoint-worker.cjs',
			sshEnvironment: createSshEnvironmentOptions(),
			singleInstanceLockOwned: true,
			traceCopyPath,
		},
		dependencies,
	);
}

function createSshEnvironmentOptions(): DesktopApplicationOptions['sshEnvironment'] {
	return {
		askPassHelperEntryPath: '/repo/apps/desktop/dist/main/ssh-askpass-helper.cjs',
		askPassRuntimeExecutablePath: '/usr/bin/node',
		release: null,
	};
}

function emitRendererDocumentLoad(webContents: EventEmitter): void {
	webContents.emit('did-start-loading');
	webContents.emit('did-start-navigation', {
		isMainFrame: true,
		isSameDocument: false,
		url: 'http://127.0.0.1:3000/',
	});
	webContents.emit('did-finish-load');
}

function createTestNativeWindow(
	bounds: Readonly<{ height: number; width: number; x: number; y: number }> = {
		height: 720,
		width: 1_024,
		x: 100,
		y: 100,
	},
) {
	let destroyed = false;
	const webContents = Object.assign(new EventEmitter(), {
		postMessage: vi.fn<(...arguments_: unknown[]) => void>(),
		setWindowOpenHandler: vi.fn<(...arguments_: unknown[]) => void>(),
	});
	const browserWindow = Object.assign(new EventEmitter(), {
		destroy: vi.fn<BrowserWindow['destroy']>(() => {
			destroyed = true;
			browserWindow.emit('closed');
		}),
		focus: vi.fn<BrowserWindow['focus']>(),
		getNormalBounds: vi.fn<BrowserWindow['getNormalBounds']>(() => ({ ...bounds })),
		isDestroyed: vi.fn<BrowserWindow['isDestroyed']>(() => destroyed),
		isFullScreen: vi.fn<BrowserWindow['isFullScreen']>(() => false),
		isMaximized: vi.fn<BrowserWindow['isMaximized']>(() => false),
		isMinimized: vi.fn<BrowserWindow['isMinimized']>(() => false),
		loadURL: vi.fn<BrowserWindow['loadURL']>(async () => {
			emitRendererDocumentLoad(webContents);
		}),
		maximize: vi.fn<BrowserWindow['maximize']>(),
		reload: vi.fn<BrowserWindow['reload']>(),
		restore: vi.fn<BrowserWindow['restore']>(),
		setFullScreen: vi.fn<BrowserWindow['setFullScreen']>(),
		show: vi.fn<BrowserWindow['show']>(),
		unmaximize: vi.fn<BrowserWindow['unmaximize']>(),
		webContents,
	}) as unknown as BrowserWindow;
	return { browserWindow, webContents };
}

function createHarness(
	firstProjectionDispose?: () => Promise<void>,
	trackSpill = false,
	spillStoreFactory?: () => TerminalExitSpillStore,
) {
	let hostEnvelopeListener: ((envelope: DecodedProtocolEnvelope) => void) | undefined;
	let identityObservedListener: ((identity: TerminalIdentity) => void) | undefined;
	const requestControl = vi.fn<HostConnection['requestControl']>().mockResolvedValue({
		configuration: { scrollbackBytes: 10_000_000 },
		revision: '1',
	});
	const host = {
		close: vi.fn<HostConnection['close']>(),
		hostEpoch: identity.hostEpoch,
		metrics: {
			bytesReceived: 0,
			bytesSent: 0,
			recordsReceived: 0,
			recordsSent: 0,
			terminalFramesReceived: 0,
		},
		negotiate: vi.fn<HostConnection['negotiate']>().mockResolvedValue({
			frameBytes: PROTOCOL_LIMITS.maxFrameBytes,
			hostEpoch: identity.hostEpoch,
			hostVersion: '0.1.0',
			inputBytes: PROTOCOL_LIMITS.maxInputBytes,
			protocol: CURRENT_PROTOCOL_VERSION,
			rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
			supportedOptionalFeatures: [],
		}),
		onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
		requestControl,
		requestTransportEnded: false,
		onEnvelope: vi.fn<HostConnection['onEnvelope']>((listener) => {
			hostEnvelopeListener = listener;
			return vi.fn<() => void>();
		}),
	} as unknown as HostConnection;
	const { browserWindow, webContents } = createTestNativeWindow();
	let windowOptions: BrowserWindowConstructorOptions | undefined;
	const clientSessionPorts: TestMainPort[] = [];
	const browserAccessBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => void>>;
		readonly runtime: Parameters<DesktopApplicationDependencies['createBrowserAccessBroker']>[1];
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const clipboardBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => void>>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const projectionBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => Promise<void>>>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const projectFolderAcquisitionBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => void>>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const projectWorktreeBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => void>>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const remoteTerminalCreationBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => void>>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const sshProjectEnrollmentBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => void>>;
		readonly runtime: Parameters<
			DesktopApplicationDependencies['createSshProjectEnrollmentBroker']
		>[1];
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const terminalLaunchProfileBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => void>>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const terminalDefaultSettingsBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => void>>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const projectionServices: Array<
		Parameters<DesktopApplicationDependencies['createProjectionControlBroker']>[1]
	> = [];
	const semanticCommandLocalEndpoints: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => Promise<void>>>;
		readonly options: Parameters<
			DesktopApplicationDependencies['createSemanticCommandLocalEndpoint']
		>[0];
		readonly start: ReturnType<typeof vi.fn<() => Promise<void>>>;
	}> = [];
	const lifetimeBrokers: Array<{
		readonly dispose: ReturnType<typeof vi.fn<() => void>>;
		readonly requestExit: ReturnType<typeof vi.fn>;
		readonly start: ReturnType<typeof vi.fn<() => void>>;
	}> = [];
	const lifetimeDependencies: DesktopApplicationLifetimeBrokerDependencies[] = [];
	const messageChannels: Array<{ readonly port1: TestMainPort; readonly port2: TestMainPort }> = [];
	const trackedSpills: Array<{
		readonly event: TerminalExited;
		readonly reason: TerminalExitSpillReason;
		readonly state: 'pending';
	}> = [];
	const trackedRejections: TerminalExited[] = [];
	const terminalExitSpillAdmit = vi
		.fn<TerminalExitSpillStore['admit']>()
		.mockImplementation((event, reason) => {
			if (trackSpill) {
				const key = JSON.stringify(event.identity);
				const existingIndex = trackedSpills.findIndex(
					(entry) => JSON.stringify(entry.event.identity) === key,
				);
				if (existingIndex === -1) trackedSpills.push({ event, reason, state: 'pending' });
				else trackedSpills[existingIndex] = { event, reason, state: 'pending' };
			}
			return { kind: 'admitted' };
		});
	const terminalExitSpillList = vi
		.fn<TerminalExitSpillStore['list']>()
		.mockImplementation(() => (trackSpill ? trackedSpills : []));
	const terminalExitSpillListRejected = vi
		.fn<TerminalExitSpillStore['listRejected']>()
		.mockImplementation(() =>
			trackSpill
				? trackedRejections.map((event) => ({ event, reason: 'pending-journal-full' as const }))
				: [],
		);
	const terminalExitSpillRemove = vi
		.fn<TerminalExitSpillStore['remove']>()
		.mockImplementation((eventOrIdentity) => {
			if (!trackSpill) return;
			const key =
				typeof eventOrIdentity === 'string'
					? eventOrIdentity
					: JSON.stringify(eventOrIdentity.identity);
			const index = trackedSpills.findIndex(
				(entry) => JSON.stringify(entry.event.identity) === key,
			);
			if (index !== -1) trackedSpills.splice(index, 1);
		});
	const terminalExitSpillIsRejected = vi
		.fn<TerminalExitSpillStore['isRejected']>()
		.mockImplementation((event) =>
			trackedRejections.some(
				(candidate) =>
					JSON.stringify(candidate.identity) === JSON.stringify(event.identity) &&
					BigInt(candidate.finalRevision) >= BigInt(event.finalRevision),
			),
		);
	const terminalExitSpillReject = vi
		.fn<TerminalExitSpillStore['reject']>()
		.mockImplementation((event) => {
			if (!trackSpill) return { kind: 'recorded' };
			trackedRejections.push(event);
			return { kind: 'recorded' };
		});
	const terminalExitSpillStore = {
		admit: terminalExitSpillAdmit,
		isRejected: terminalExitSpillIsRejected,
		list: terminalExitSpillList,
		listRejected: terminalExitSpillListRejected,
		load: vi.fn<TerminalExitSpillStore['load']>().mockResolvedValue({ kind: 'ready' }),
		reject: terminalExitSpillReject,
		remove: terminalExitSpillRemove,
	} as unknown as TerminalExitSpillStore;
	const publishedExitLedgerEvents: TerminalExited[] = [];
	const terminalExitPublicationLedgerMarkPublished = vi
		.fn<TerminalExitPublicationLedger['markPublished']>()
		.mockImplementation((event) => {
			const existingIndex = publishedExitLedgerEvents.findIndex(
				(candidate) => JSON.stringify(candidate.identity) === JSON.stringify(event.identity),
			);
			if (existingIndex === -1) {
				publishedExitLedgerEvents.push(event);
				return true;
			}
			const existing = publishedExitLedgerEvents[existingIndex];
			if (existing === undefined || BigInt(existing.finalRevision) >= BigInt(event.finalRevision)) {
				return true;
			}
			publishedExitLedgerEvents[existingIndex] = event;
			return true;
		});
	const terminalExitPublicationLedger = {
		has: vi
			.fn<TerminalExitPublicationLedger['has']>()
			.mockImplementation((event) =>
				publishedExitLedgerEvents.some(
					(candidate) =>
						JSON.stringify(candidate.identity) === JSON.stringify(event.identity) &&
						BigInt(candidate.finalRevision) >= BigInt(event.finalRevision),
				),
			),
		hasExact: vi
			.fn<TerminalExitPublicationLedger['hasExact']>()
			.mockImplementation((event) =>
				publishedExitLedgerEvents.some(
					(candidate) =>
						JSON.stringify(candidate.identity) === JSON.stringify(event.identity) &&
						BigInt(candidate.finalRevision) === BigInt(event.finalRevision),
				),
			),
		list: vi
			.fn<TerminalExitPublicationLedger['list']>()
			.mockImplementation(() => publishedExitLedgerEvents.map((event) => ({ event }))),
		load: vi.fn<TerminalExitPublicationLedger['load']>().mockResolvedValue({ kind: 'ready' }),
		markPublished: terminalExitPublicationLedgerMarkPublished,
	} as unknown as TerminalExitPublicationLedger & {
		readonly markPublished: typeof terminalExitPublicationLedgerMarkPublished;
	};
	const createTerminalExitSpillStore = vi.fn<
		NonNullable<DesktopApplicationDependencies['createTerminalExitSpillStore']>
	>(() => spillStoreFactory?.() ?? terminalExitSpillStore);
	const createTerminalExitPublicationLedger = vi.fn<
		NonNullable<DesktopApplicationDependencies['createTerminalExitPublicationLedger']>
	>(() => terminalExitPublicationLedger);
	const workbenchDocumentStore = {
		commit: vi
			.fn<WorkbenchDocumentStore['commit']>()
			.mockResolvedValue({ kind: 'unavailable', reason: 'write-failed' }),
		load: vi.fn<WorkbenchDocumentStore['load']>().mockResolvedValue({ kind: 'missing' }),
	} satisfies WorkbenchDocumentStore;
	const terminalRuntime = {
		closeBindingsNotReferencedBy: vi
			.fn<DesktopTerminalRuntime['closeBindingsNotReferencedBy']>()
			.mockResolvedValue({ closedCount: 0, failures: [] }),
		dispose: vi.fn<DesktopTerminalRuntime['dispose']>().mockResolvedValue(undefined),
		execute: vi.fn<DesktopTerminalRuntime['execute']>(),
		findPresentationByIdentity: vi
			.fn<DesktopTerminalRuntime['findPresentationByIdentity']>()
			.mockReturnValue(presentationId),
		getProjectionTarget: vi
			.fn<DesktopTerminalRuntime['getProjectionTarget']>()
			.mockReturnValue(null),
		getDurableTerminalExit: vi
			.fn<DesktopTerminalRuntime['getDurableTerminalExit']>()
			.mockReturnValue(null),
		getTerminalFailure: vi.fn<DesktopTerminalRuntime['getTerminalFailure']>().mockReturnValue(null),
		commitExitPublication: vi
			.fn<NonNullable<DesktopTerminalRuntime['commitExitPublication']>>()
			.mockReturnValue(true),
		getProjectionMetrics: vi
			.fn<DesktopTerminalRuntime['getProjectionMetrics']>()
			.mockReturnValue([]),
		getOwnedBindingCount: vi
			.fn<DesktopTerminalRuntime['getOwnedBindingCount']>()
			.mockReturnValue(0),
		getRecoveredPresentations: vi
			.fn<DesktopTerminalRuntime['getRecoveredPresentations']>()
			.mockReturnValue([{ launchCwd: '/workspace', presentationId }]),
		initialize: vi.fn<DesktopTerminalRuntime['initialize']>().mockResolvedValue({ kind: 'ready' }),
		inspectUnknownHostTerminalCount: vi
			.fn<DesktopTerminalRuntime['inspectUnknownHostTerminalCount']>()
			.mockResolvedValue(0),
		open: vi.fn<DesktopTerminalRuntime['open']>(),
		openReadOnly: vi.fn<DesktopTerminalRuntime['openReadOnly']>(),
		reconcileArchives: vi
			.fn<DesktopTerminalRuntime['reconcileArchives']>()
			.mockResolvedValue(undefined),
		recordExit: vi.fn<DesktopTerminalRuntime['recordExit']>().mockResolvedValue(null),
		recordExitOutcome: vi
			.fn<DesktopTerminalRuntime['recordExitOutcome']>()
			.mockResolvedValue({ kind: 'ignored' }),
		recordFailure: vi.fn<DesktopTerminalRuntime['recordFailure']>().mockResolvedValue(null),
		setIdentityObservedListener: vi.fn<
			NonNullable<DesktopTerminalRuntime['setIdentityObservedListener']>
		>((listener) => {
			identityObservedListener = listener;
		}),
	};
	const createWindow = vi.fn<DesktopApplicationDependencies['createWindow']>((options) => {
		windowOptions = options;
		return browserWindow;
	});
	const windowStateStore = {
		flush: vi.fn<DesktopWindowStateStore['flush']>().mockResolvedValue(undefined),
		load: vi.fn<DesktopWindowStateStore['load']>().mockResolvedValue({ kind: 'missing' }),
		save: vi.fn<DesktopWindowStateStore['save']>().mockResolvedValue(true),
	} satisfies DesktopWindowStateStore;
	type BrowserAccessRuntime = ReturnType<
		DesktopApplicationDependencies['createBrowserAccessRuntime']
	>;
	const browserAccessSnapshot = {
		access: { phase: 'stopped' },
		personalAccessToken: 'configured',
	} as const;
	const browserAccessRuntime = {
		copyAccessLink: vi
			.fn<BrowserAccessRuntime['copyAccessLink']>()
			.mockResolvedValue(browserAccessSnapshot),
		dispose: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
		getSnapshot: vi
			.fn<BrowserAccessRuntime['getSnapshot']>()
			.mockReturnValue(browserAccessSnapshot),
		initialize: vi
			.fn<BrowserAccessRuntime['initialize']>()
			.mockResolvedValue(browserAccessSnapshot),
		issuePersonalAccessToken: vi
			.fn<BrowserAccessRuntime['issuePersonalAccessToken']>()
			.mockResolvedValue({
				personalAccessToken: PersonalAccessTokenSchema.parse('t'.repeat(43)),
				snapshot: browserAccessSnapshot,
			}),
		openLocalBrowser: vi
			.fn<BrowserAccessRuntime['openLocalBrowser']>()
			.mockResolvedValue(browserAccessSnapshot),
		start: vi.fn<BrowserAccessRuntime['start']>().mockResolvedValue(browserAccessSnapshot),
		stop: vi.fn<BrowserAccessRuntime['stop']>().mockResolvedValue(browserAccessSnapshot),
	} satisfies ReturnType<DesktopApplicationDependencies['createBrowserAccessRuntime']>;
	const createBrowserAccessRuntime = vi
		.fn<DesktopApplicationDependencies['createBrowserAccessRuntime']>()
		.mockReturnValue(browserAccessRuntime);
	const createProjectionControlBroker = vi.fn<
		DesktopApplicationDependencies['createProjectionControlBroker']
	>((_port, service) => {
		projectionServices.push(service);
		const broker = {
			dispose: vi.fn<() => Promise<void>>(
				projectionBrokers.length === 0 && firstProjectionDispose !== undefined
					? firstProjectionDispose
					: () => Promise.resolve(),
			),
			start: vi.fn<() => void>(),
		};
		projectionBrokers.push(broker);
		return broker;
	});
	const createTerminalRuntime = vi
		.fn<DesktopApplicationDependencies['createTerminalRuntime']>()
		.mockReturnValue(terminalRuntime);
	let terminalLaunchProfileBaseLaunch = createRuntime().terminalLaunch;
	const terminalLaunchProfileSnapshot = DesktopTerminalLaunchProfileSnapshotSchema.parse({
		capabilities: { loginShell: true },
		catalog: {
			defaultProfileId: 'system-default',
			profiles: [
				{
					command: { kind: 'system-shell', login: false },
					id: 'system-default',
					name: null,
				},
			],
			version: 1,
		},
		persistenceIssue: null,
		source: 'built-in',
	});
	const terminalLaunchProfileController = {
		flush: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
		getSnapshot: () => terminalLaunchProfileSnapshot,
		initialize: vi
			.fn<() => Promise<typeof terminalLaunchProfileSnapshot>>()
			.mockResolvedValue(terminalLaunchProfileSnapshot),
		mutate: vi.fn<
			ReturnType<DesktopApplicationDependencies['createTerminalLaunchProfileController']>['mutate']
		>(async () => ({ kind: 'completed', snapshot: terminalLaunchProfileSnapshot })),
		resolveDefaultLaunch: vi.fn<
			ReturnType<
				DesktopApplicationDependencies['createTerminalLaunchProfileController']
			>['resolveDefaultLaunch']
		>(() => terminalLaunchProfileBaseLaunch),
	} satisfies ReturnType<DesktopApplicationDependencies['createTerminalLaunchProfileController']>;
	type ProjectWorktreeRuntime = ReturnType<
		DesktopApplicationDependencies['createProjectWorktreeRuntime']
	>;
	const projectWorktreeRuntime: ProjectWorktreeRuntime = {
		adopt: vi
			.fn<ProjectWorktreeRuntime['adopt']>()
			.mockResolvedValue({ code: 'control-unavailable', kind: 'failed' }),
		create: vi
			.fn<ProjectWorktreeRuntime['create']>()
			.mockResolvedValue({ code: 'control-unavailable', kind: 'failed' }),
		discover: vi
			.fn<ProjectWorktreeRuntime['discover']>()
			.mockResolvedValue({ code: 'control-unavailable', kind: 'failed' }),
		dispose: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
		start: vi.fn<() => Promise<'ready'>>().mockResolvedValue('ready'),
	};
	const terminalDefaultSettingsSnapshot = DesktopTerminalDefaultSettingsSnapshotSchema.parse({
		configuration: { scrollbackBytes: 10_000_000 },
		persistenceIssue: null,
		source: 'runtime',
	});
	let setHostDefaults:
		| Parameters<
				DesktopApplicationDependencies['createTerminalDefaultSettingsController']
		  >[0]['setHostDefaults']
		| undefined;
	const terminalDefaultSettingsController = {
		flush: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
		getSnapshot: () => terminalDefaultSettingsSnapshot,
		initialize: vi.fn<
			ReturnType<
				DesktopApplicationDependencies['createTerminalDefaultSettingsController']
			>['initialize']
		>(async () => {
			if (setHostDefaults === undefined) throw new Error('Host defaults callback is unavailable.');
			const accepted = await setHostDefaults(terminalDefaultSettingsSnapshot.configuration);
			if (
				accepted.scrollbackBytes !== terminalDefaultSettingsSnapshot.configuration.scrollbackBytes
			) {
				throw new Error('Terminal Host did not accept the requested terminal defaults.');
			}
			return terminalDefaultSettingsSnapshot;
		}),
		update: vi.fn<
			ReturnType<
				DesktopApplicationDependencies['createTerminalDefaultSettingsController']
			>['update']
		>(async () => ({
			kind: 'completed',
			snapshot: terminalDefaultSettingsSnapshot,
		})),
	} satisfies ReturnType<DesktopApplicationDependencies['createTerminalDefaultSettingsController']>;
	const agentIntegrationManagementSnapshot = DesktopAgentIntegrationManagementSnapshotSchema.parse({
		integrations: DESKTOP_AGENT_INTEGRATION_IDS.map((integrationId) => ({
			availability: 'not-found',
			integrationId,
			issue: null,
			managementMode: 'managed-local',
			state: 'unknown',
		})),
		launchedIntegrationIds: [],
		revision: 0,
	});
	type AgentIntegrationManagementController = ReturnType<
		DesktopApplicationDependencies['createAgentIntegrationManagementController']
	>;
	const agentIntegrationManagementController: AgentIntegrationManagementController = {
		disable: vi.fn<AgentIntegrationManagementController['disable']>(async () => ({
			kind: 'completed' as const,
			snapshot: agentIntegrationManagementSnapshot,
		})),
		enable: vi.fn<AgentIntegrationManagementController['enable']>(async () => ({
			kind: 'completed' as const,
			snapshot: agentIntegrationManagementSnapshot,
		})),
		getSnapshot: () => agentIntegrationManagementSnapshot,
		initialize: vi.fn<AgentIntegrationManagementController['initialize']>(
			async () => agentIntegrationManagementSnapshot,
		),
		observeLaunch: vi.fn<AgentIntegrationManagementController['observeLaunch']>(() => true),
		refresh: vi.fn<AgentIntegrationManagementController['refresh']>(async () => ({
			kind: 'completed' as const,
			snapshot: agentIntegrationManagementSnapshot,
		})),
		subscribeLaunches: vi
			.fn<AgentIntegrationManagementController['subscribeLaunches']>()
			.mockReturnValue(() => undefined),
	};
	const sshEnvironmentTargetResolver = {
		resolve: vi
			.fn<NonNullable<DesktopApplicationDependencies['sshEnvironmentTargetResolver']>['resolve']>()
			.mockResolvedValue({ code: 'openssh-unavailable', kind: 'failed' }),
	};
	type SshEnvironmentConnectionOwnerHandle = ReturnType<
		DesktopApplicationDependencies['createSshEnvironmentConnectionOwner']
	>;
	const sshEnvironmentConnectionOwner = {
		claimInteractionLease: vi
			.fn<SshEnvironmentConnectionOwnerHandle['claimInteractionLease']>()
			.mockReturnValue({ kind: 'busy' }),
		connect: vi
			.fn<SshEnvironmentConnectionOwnerHandle['connect']>()
			.mockResolvedValue({ kind: 'not-found' }),
		disconnect: vi
			.fn<SshEnvironmentConnectionOwnerHandle['disconnect']>()
			.mockResolvedValue({ kind: 'inactive' }),
		dispose: vi.fn<SshEnvironmentConnectionOwnerHandle['dispose']>().mockResolvedValue(undefined),
		inspectProjectRoot: vi
			.fn<SshEnvironmentConnectionOwnerHandle['inspectProjectRoot']>()
			.mockResolvedValue({ kind: 'failed', reason: 'unavailable' }),
		listProjectDirectory: vi
			.fn<SshEnvironmentConnectionOwnerHandle['listProjectDirectory']>()
			.mockResolvedValue({ kind: 'failed', reason: 'unavailable' }),
		openTerminalHost: vi
			.fn<SshEnvironmentConnectionOwnerHandle['openTerminalHost']>()
			.mockResolvedValue({ kind: 'failed', reason: 'unavailable' }),
		observation: vi
			.fn<SshEnvironmentConnectionOwnerHandle['observation']>()
			.mockReturnValue({ kind: 'inactive' }),
		subscribe: vi.fn<SshEnvironmentConnectionOwnerHandle['subscribe']>().mockReturnValue(null),
	} satisfies SshEnvironmentConnectionOwnerHandle;
	let environmentRegistryDocument: EnvironmentRegistryDocument = {
		environments: [],
		revision: 0,
		schemaVersion: 2,
	};
	const environmentRegistryStore = {
		commit: vi.fn<EnvironmentRegistryStore['commit']>(async (expectedRevision, document) => {
			if (environmentRegistryDocument.revision !== expectedRevision) {
				return { document: environmentRegistryDocument, kind: 'conflict' };
			}
			environmentRegistryDocument = document;
			return { kind: 'committed' };
		}),
		load: vi.fn<EnvironmentRegistryStore['load']>(async () => ({
			document: environmentRegistryDocument,
			kind: 'ready',
		})),
	} satisfies EnvironmentRegistryStore;
	const environmentRegistry = new DesktopEnvironmentRegistry(environmentRegistryStore);
	let identifierSequence = 0;
	const dependencies: DesktopApplicationDependencies = {
		connectHost: vi.fn<DesktopApplicationDependencies['connectHost']>().mockResolvedValue(host),
		createAgentIntegrationManagementBroker: vi.fn<
			DesktopApplicationDependencies['createAgentIntegrationManagementBroker']
		>(() => ({
			dispose: vi.fn<() => void>(),
			start: vi.fn<() => void>(),
		})),
		createAgentIntegrationManagementController: vi.fn<
			DesktopApplicationDependencies['createAgentIntegrationManagementController']
		>(() => agentIntegrationManagementController),
		createApplicationLifetimeBroker: vi.fn<
			DesktopApplicationDependencies['createApplicationLifetimeBroker']
		>((_port, brokerDependencies) => {
			lifetimeDependencies.push(brokerDependencies);
			const broker = {
				dispose: vi.fn<() => void>(),
				requestExit: vi
					.fn<
						ReturnType<
							DesktopApplicationDependencies['createApplicationLifetimeBroker']
						>['requestExit']
					>()
					.mockResolvedValue({ kind: 'cancelled' }),
				start: vi.fn<() => void>(),
			};
			lifetimeBrokers.push(broker);
			return broker;
		}),
		createBrowserAccessBroker: vi.fn<DesktopApplicationDependencies['createBrowserAccessBroker']>(
			(_port, runtime) => {
				const broker = {
					dispose: vi.fn<() => void>(),
					runtime,
					start: vi.fn<() => void>(),
				};
				browserAccessBrokers.push(broker);
				return broker;
			},
		),
		createBrowserAccessRuntime,
		createClipboardBroker: vi.fn<DesktopApplicationDependencies['createClipboardBroker']>(() => {
			const broker = {
				dispose: vi.fn<() => void>(),
				start: vi.fn<() => void>(),
			};
			clipboardBrokers.push(broker);
			return broker;
		}),
		createEnvironmentRegistry: vi
			.fn<DesktopApplicationDependencies['createEnvironmentRegistry']>()
			.mockReturnValue(environmentRegistry),
		createSshEnvironmentConnectionOwner: vi
			.fn<DesktopApplicationDependencies['createSshEnvironmentConnectionOwner']>()
			.mockReturnValue(sshEnvironmentConnectionOwner),
		createBroker: vi.fn<DesktopApplicationDependencies['createBroker']>(),
		createClientSessionBroker: vi.fn<DesktopApplicationDependencies['createClientSessionBroker']>(
			(port, coordinator, disposeAttachments) => {
				const mainPort = port as unknown as TestMainPort;
				clientSessionPorts.push(mainPort);
				return new DesktopClientSessionBroker(port, coordinator, { disposeAttachments });
			},
		),
		createClientSessionCoordinator: (coordinatorDependencies) =>
			new DesktopClientSessionCoordinator(coordinatorDependencies),
		createMessageChannel: vi.fn<DesktopApplicationDependencies['createMessageChannel']>(() => {
			const channel = { port1: new TestMainPort(), port2: new TestMainPort() };
			messageChannels.push(channel);
			return channel as unknown as {
				readonly port1: MessagePortMain;
				readonly port2: MessagePortMain;
			};
		}),
		createProjectionControlBroker,
		createProjectFolderAcquisitionBroker: vi.fn<
			DesktopApplicationDependencies['createProjectFolderAcquisitionBroker']
		>(() => {
			const broker = {
				dispose: vi.fn<() => void>(),
				start: vi.fn<() => void>(),
			};
			projectFolderAcquisitionBrokers.push(broker);
			return broker;
		}),
		createProjectWorktreeBroker: vi.fn<
			DesktopApplicationDependencies['createProjectWorktreeBroker']
		>(() => {
			const broker = {
				dispose: vi.fn<() => void>(),
				start: vi.fn<() => void>(),
			};
			projectWorktreeBrokers.push(broker);
			return broker;
		}),
		createRemoteTerminalCreationBroker: vi.fn<
			DesktopApplicationDependencies['createRemoteTerminalCreationBroker']
		>(() => {
			const broker = {
				dispose: vi.fn<() => void>(),
				start: vi.fn<() => void>(),
			};
			remoteTerminalCreationBrokers.push(broker);
			return broker;
		}),
		createSshProjectEnrollmentBroker: vi.fn<
			DesktopApplicationDependencies['createSshProjectEnrollmentBroker']
		>((_port, runtime) => {
			const broker = {
				dispose: vi.fn<() => void>(),
				runtime,
				start: vi.fn<() => void>(),
			};
			sshProjectEnrollmentBrokers.push(broker);
			return broker;
		}),
		createProjectWorktreeRuntime: vi
			.fn<DesktopApplicationDependencies['createProjectWorktreeRuntime']>()
			.mockReturnValue(projectWorktreeRuntime),
		createSemanticCommandLocalEndpoint: vi.fn<
			DesktopApplicationDependencies['createSemanticCommandLocalEndpoint']
		>((options) => {
			const endpoint = {
				dispose: vi.fn<() => Promise<void>>(async () => undefined),
				options,
				start: vi.fn<() => Promise<void>>(async () => undefined),
			};
			semanticCommandLocalEndpoints.push(endpoint);
			return endpoint;
		}),
		createTerminalLaunchProfileBroker: vi.fn<
			DesktopApplicationDependencies['createTerminalLaunchProfileBroker']
		>(() => {
			const broker = {
				dispose: vi.fn<() => void>(),
				start: vi.fn<() => void>(),
			};
			terminalLaunchProfileBrokers.push(broker);
			return broker;
		}),
		createTerminalDefaultSettingsBroker: vi.fn<
			DesktopApplicationDependencies['createTerminalDefaultSettingsBroker']
		>(() => {
			const broker = {
				dispose: vi.fn<() => void>(),
				start: vi.fn<() => void>(),
			};
			terminalDefaultSettingsBrokers.push(broker);
			return broker;
		}),
		createTerminalDefaultSettingsController: vi.fn<
			DesktopApplicationDependencies['createTerminalDefaultSettingsController']
		>((options) => {
			setHostDefaults = options.setHostDefaults;
			return terminalDefaultSettingsController;
		}),
		createTerminalLaunchProfileController: vi.fn<
			DesktopApplicationDependencies['createTerminalLaunchProfileController']
		>((options) => {
			terminalLaunchProfileBaseLaunch = options.baseLaunch;
			return terminalLaunchProfileController;
		}),
		createTerminalRuntime,
		createTerminalExitSpillStore,
		createTerminalExitPublicationLedger,
		createTerminalIntentReconciler: (reconcilerDependencies) =>
			new DesktopTerminalIntentReconciler(reconcilerDependencies),
		createWorkbenchDocumentStore: vi
			.fn<DesktopApplicationDependencies['createWorkbenchDocumentStore']>()
			.mockReturnValue(workbenchDocumentStore),
		createWindow,
		createWindowStateStore: vi
			.fn<DesktopApplicationDependencies['createWindowStateStore']>()
			.mockReturnValue(windowStateStore),
		getPreferredLocales: () => ['en-US'],
		getWindowDisplayTopology: () => ({
			primaryWorkArea: { height: 900, width: 1_440, x: 0, y: 0 },
			workAreas: [{ height: 900, width: 1_440, x: 0, y: 0 }],
		}),
		randomIdentifier: () => (++identifierSequence).toString(16).padStart(32, '0'),
		quitApplication: vi.fn<() => void>(),
		sshEnvironmentTargetResolver,
		writeTrace: vi.fn<DesktopApplicationDependencies['writeTrace']>(),
	};
	return {
		agentIntegrationManagementController,
		browserAccessBrokers,
		browserAccessRuntime,
		browserWindow,
		clipboardBrokers,
		clientSessionPorts,
		createBrowserAccessRuntime,
		createProjectionControlBroker,
		createTerminalRuntime,
		createTerminalExitPublicationLedger,
		createTerminalExitSpillStore,
		createWorkbenchDocumentStore: dependencies.createWorkbenchDocumentStore,
		createWindow,
		dependencies,
		environmentRegistry,
		environmentRegistryStore,
		host,
		lifetimeBrokers,
		lifetimeDependencies,
		messageChannels,
		get hostEnvelopeListener() {
			return hostEnvelopeListener;
		},
		get identityObservedListener() {
			return identityObservedListener;
		},
		projectionBrokers,
		projectFolderAcquisitionBrokers,
		projectWorktreeBrokers,
		projectWorktreeRuntime,
		remoteTerminalCreationBrokers,
		sshProjectEnrollmentBrokers,
		terminalLaunchProfileBrokers,
		terminalLaunchProfileController,
		terminalDefaultSettingsBrokers,
		terminalDefaultSettingsController,
		projectionServices,
		quitApplication: dependencies.quitApplication,
		requestControl,
		semanticCommandLocalEndpoints,
		sshEnvironmentConnectionOwner,
		sshEnvironmentTargetResolver,
		terminalRuntime,
		terminalExitSpillAdmit,
		terminalExitSpillStore,
		terminalExitPublicationLedger,
		publishedExitLedgerEvents,
		webContents,
		workbenchDocumentStore,
		windowStateStore,
		get windowOptions() {
			return windowOptions;
		},
	};
}

function installRealTerminalRuntime(harness: ReturnType<typeof createHarness>): void {
	harness.createTerminalRuntime.mockImplementation(
		(
			host,
			configuration,
			randomIdentifier,
			createMessageChannel,
			createBroker,
			resolveTerminalLaunch,
		) => {
			return composeDesktopTerminalRuntime({
				configuration,
				createBroker,
				createMessageChannel,
				host,
				randomIdentifier,
				resolveTerminalLaunch,
			});
		},
	);
}

function configureRealHost(
	harness: ReturnType<typeof createHarness>,
	createResponse: () => Promise<TerminalIdentity> | TerminalIdentity,
	inventory: readonly TerminalSummary[],
): void {
	harness.requestControl.mockImplementation(async (kind, value) => {
		switch (kind) {
			case MessageKind.createTerminal: {
				const requested = value as { readonly creationId: string };
				return {
					...(await createResponse()),
					creationId: CreationIdSchema.parse(requested.creationId),
				} as never;
			}
			case MessageKind.listTerminals:
				return {
					inventoryGeneration: '1',
					nextCursor: null,
					terminals: inventory,
				} as never;
			case MessageKind.setTerminalDefaults:
				return { configuration: { scrollbackBytes: 10_000_000 }, revision: '1' } as never;
			default:
				throw new Error(`Unexpected real-composition Host request ${kind}.`);
		}
	});
}

function rendererBootstrap(harness: ReturnType<typeof createHarness>):
	| {
			readonly clientSession?: {
				readonly initializationDocument?: WorkbenchSharedDocument;
				readonly terminalMetadata?: readonly WorkbenchTerminalMetadata[];
				readonly terminalEvents?: readonly TerminalPresentationEvent[];
			};
	  }
	| undefined {
	return harness.webContents.postMessage.mock.calls.find(
		(call) => call[0] === 'orchbee.desktop.renderer-bootstrap',
	)?.[1] as
		| {
				readonly clientSession?: {
					readonly initializationDocument?: WorkbenchSharedDocument;
					readonly terminalMetadata?: readonly WorkbenchTerminalMetadata[];
					readonly terminalEvents?: readonly TerminalPresentationEvent[];
				};
		  }
		| undefined;
}

async function proposeInitializationDocument(
	harness: ReturnType<typeof createHarness>,
	requestId: string,
): Promise<void> {
	const bootstrap = rendererBootstrap(harness);
	const document = bootstrap?.clientSession?.initializationDocument;
	const port = harness.clientSessionPorts[0];
	if (document === undefined || port === undefined) {
		throw new Error('Renderer bootstrap is missing the initialization document.');
	}
	harness.workbenchDocumentStore.commit.mockResolvedValueOnce({
		kind: 'committed',
		revision: document.revision,
	});
	port.emit('message', {
		data: {
			document,
			expectedRevision: null,
			requestId,
			type: 'shared-document-proposed',
		},
		ports: [],
	});
	await vi.waitFor(() =>
		expect(port.postMessage).toHaveBeenCalledWith(
			expect.objectContaining({ type: 'shared-document-published' }),
		),
	);
	await vi.waitFor(() =>
		expect(port.postMessage).toHaveBeenCalledWith(
			expect.objectContaining({ requestId, type: 'shared-document-proposal-completed' }),
		),
	);
}

function latestRendererBootstrap(
	harness: ReturnType<typeof createHarness>,
): ReturnType<typeof rendererBootstrap> {
	return harness.webContents.postMessage.mock.calls
		.filter((call) => call[0] === 'orchbee.desktop.renderer-bootstrap')
		.at(-1)?.[1] as ReturnType<typeof rendererBootstrap>;
}

function terminalEventPublications(
	harness: ReturnType<typeof createHarness>,
	eventType = 'terminal-exited',
): readonly unknown[] {
	return harness.clientSessionPorts.flatMap((port) =>
		port.postMessage.mock.calls
			.filter(([value]) => {
				const publication = value as
					| {
							readonly event?: { readonly type?: string };
							readonly type?: string;
					  }
					| undefined;
				return (
					publication?.type === 'terminal-presentation-event-published' &&
					publication.event?.type === eventType
				);
			})
			.map(([value]) => value),
	);
}

function terminalSummary(event: TerminalExited): TerminalSummary {
	return {
		columns: 80,
		configuration: { scrollbackBytes: 10_000_000 },
		configurationRevision: CanonicalUint64TextSchema.parse('1'),
		exit: event.exit,
		failure: null,
		failureContainment: null,
		identity: event.identity,
		phase: 'exited',
		process: null,
		revision: event.finalRevision,
		rows: 24,
		terminateRequested: false,
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

function terminalExitedEnvelope(event: TerminalExited): DecodedProtocolEnvelope {
	return {
		correlationId: null,
		event: true,
		hostEpoch: identity.hostEpoch,
		ignoredOptionalFlags: 0,
		kind: MessageKind.terminalExited,
		knownMessage: true,
		optionalMessage: false,
		payload: new TextEncoder().encode(JSON.stringify(event)),
		payloadEncoding: PayloadEncoding.controlJson,
		protocol: CURRENT_PROTOCOL_VERSION,
		response: false,
	};
}

function terminalMetadataChangedEnvelope(event: TerminalMetadataChanged): DecodedProtocolEnvelope {
	return {
		correlationId: null,
		event: true,
		hostEpoch: identity.hostEpoch,
		ignoredOptionalFlags: 0,
		kind: MessageKind.terminalMetadataChanged,
		knownMessage: true,
		optionalMessage: false,
		payload: new TextEncoder().encode(JSON.stringify(event)),
		payloadEncoding: PayloadEncoding.controlJson,
		protocol: CURRENT_PROTOCOL_VERSION,
		response: false,
	};
}

function terminalApplicationSignalEnvelope(
	payload: string,
	sequence = '1',
): DecodedProtocolEnvelope {
	const event: TerminalApplicationSignal = {
		channel: 'orchbee.agent.v1',
		identity,
		payload,
		sequence: CanonicalUint64TextSchema.parse(sequence),
	};
	return {
		correlationId: null,
		event: true,
		hostEpoch: identity.hostEpoch,
		ignoredOptionalFlags: 0,
		kind: MessageKind.terminalApplicationSignal,
		knownMessage: true,
		optionalMessage: false,
		payload: new TextEncoder().encode(JSON.stringify(event)),
		payloadEncoding: PayloadEncoding.controlJson,
		protocol: CURRENT_PROTOCOL_VERSION,
		response: false,
	};
}

function terminalFailedEnvelope(event: TerminalFailed): DecodedProtocolEnvelope {
	return {
		correlationId: null,
		event: true,
		hostEpoch: identity.hostEpoch,
		ignoredOptionalFlags: 0,
		kind: MessageKind.terminalFailed,
		knownMessage: true,
		optionalMessage: false,
		payload: new TextEncoder().encode(JSON.stringify(event)),
		payloadEncoding: PayloadEncoding.controlJson,
		protocol: CURRENT_PROTOCOL_VERSION,
		response: false,
	};
}

function terminalMetadataChanged(revision: string, title: string | null): TerminalMetadataChanged {
	return {
		currentDirectory: 'file://orchbee-host/workspace/background-agent',
		identity,
		revision: CanonicalUint64TextSchema.parse(revision),
		title,
	};
}

function terminalExitFor(code: number): TerminalExited {
	return {
		exit: { code, kind: 'exit-code' },
		finalRevision: CanonicalUint64TextSchema.parse(String(code + 1)),
		identity: {
			creationId: identity.creationId,
			hostEpoch: identity.hostEpoch,
			terminalId: TerminalIdSchema.parse((code + 100).toString(16).padStart(32, '0')),
		},
	};
}

function createDeferred<Value>(): { promise: Promise<Value>; resolve: (value: Value) => void } {
	let resolvePromise: (value: Value) => void = () => undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return { promise, resolve: resolvePromise };
}

function remoteWorkspaceDocument(environmentId: string): WorkbenchSharedDocument {
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

function createRuntime(
	environment: Parameters<typeof resolveDesktopRuntimeConfiguration>[0]['environment'] = {
		SHELL: '/bin/sh',
	},
) {
	return resolveDesktopRuntimeConfiguration({
		applicationPath: '/repo/apps/desktop',
		environment,
		homeDirectory: '/users/orchbee',
		platform: 'linux',
		posixRuntimeBaseDirectory: '/run/user/501',
		terminalHostName: 'orchbee-host',
		userDataPath: '/users/orchbee/.config/orchbee',
	});
}
